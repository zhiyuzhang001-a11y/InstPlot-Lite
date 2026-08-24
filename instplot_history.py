"""Pure differential undo/redo primitives for tabular data operations."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
from typing import Any, Iterable, Sequence

import numpy as np
import pandas as pd


class HistoryError(RuntimeError):
    """A deterministic, caller-visible history failure."""

    def __init__(self, code: str, reason: str):
        self.code = code
        self.reason = reason
        super().__init__(f"{code}: {reason}")


@dataclass(frozen=True)
class _Target:
    token: object
    position: int
    path: Any


def _digest(frame: pd.DataFrame) -> bytes:
    """Return a compact fingerprint without retaining a full-frame snapshot."""
    try:
        hashed = pd.util.hash_pandas_object(frame, index=True).to_numpy(
            dtype="uint64", copy=False
        )
        metadata = repr(
            (
                frame.shape,
                frame.columns.tolist(),
                [str(dtype) for dtype in frame.dtypes],
                frame.index.names,
                type(frame.index).__name__,
            )
        ).encode("utf-8", "backslashreplace")
    except Exception as exc:
        raise HistoryError("unsupported_state", str(exc)) from exc
    digest = hashlib.sha256(metadata)
    digest.update(hashed.tobytes())
    return digest.digest()


def _positions(values: Iterable[int], size: int, kind: str) -> tuple[int, ...]:
    try:
        positions = tuple(sorted({int(value) for value in values}))
    except (TypeError, ValueError) as exc:
        raise HistoryError(f"invalid_{kind}", "positions must be integers") from exc
    if any(value < 0 or value >= size for value in positions):
        raise HistoryError(f"invalid_{kind}", "position is out of range")
    return positions


def _column_position(frame: pd.DataFrame, column: Any) -> int:
    matches = [position for position, label in enumerate(frame.columns) if label == column]
    if len(matches) != 1:
        reason = "column does not exist" if not matches else "column label is ambiguous"
        raise HistoryError("invalid_column", reason)
    return matches[0]


def _memory_bytes(value: Any) -> int:
    if isinstance(value, (pd.DataFrame, pd.Series)):
        usage = value.memory_usage(index=True, deep=True)
        return int(usage.sum() if hasattr(usage, "sum") else usage)
    if isinstance(value, np.ndarray):
        return int(value.nbytes)
    return 0


class HistoryManager:
    """Own command stacks and stable per-list-entry identities."""

    def __init__(self, loaded_files: list[tuple[Any, pd.DataFrame]], max_steps: int = 10):
        if max_steps < 1:
            raise HistoryError("invalid_limit", "max_steps must be positive")
        self.max_steps = int(max_steps)
        self._undo: list[Any] = []
        self._redo: list[Any] = []
        self.reset(loaded_files)

    @property
    def undo_count(self) -> int:
        return len(self._undo)

    @property
    def redo_count(self) -> int:
        return len(self._redo)

    @property
    def payload_bytes(self) -> int:
        return sum(command.payload_bytes for command in self._undo + self._redo)

    def reset(self, loaded_files: list[tuple[Any, pd.DataFrame]]) -> None:
        if not isinstance(loaded_files, list):
            raise HistoryError("invalid_state", "loaded_files must be a list")
        self.loaded_files = loaded_files
        self._tokens = [object() for _ in loaded_files]
        self._frames = []
        for entry in loaded_files:
            if not isinstance(entry, tuple) or len(entry) != 2 or not isinstance(entry[1], pd.DataFrame):
                raise HistoryError("invalid_state", "each entry must be a (path, DataFrame) tuple")
            self._frames.append(entry[1])
        self._undo.clear()
        self._redo.clear()

    def _validate_structure(self) -> None:
        if len(self.loaded_files) != len(self._tokens):
            raise HistoryError("state_mismatch", "loaded file count changed outside history")
        for position, entry in enumerate(self.loaded_files):
            if (
                not isinstance(entry, tuple)
                or len(entry) != 2
                or entry[1] is not self._frames[position]
            ):
                raise HistoryError("state_mismatch", "loaded file structure changed outside history")

    def target(self, position: int) -> _Target:
        self._validate_structure()
        if not isinstance(position, int) or position < 0 or position >= len(self.loaded_files):
            raise HistoryError("invalid_file", "file position is out of range")
        return _Target(self._tokens[position], position, self.loaded_files[position][0])

    def _resolve(self, target: _Target) -> int:
        self._validate_structure()
        if target.position >= len(self._tokens) or self._tokens[target.position] is not target.token:
            raise HistoryError("state_mismatch", "target entry identity changed")
        if self.loaded_files[target.position][0] != target.path:
            raise HistoryError("state_mismatch", "target path changed")
        return target.position

    def _replace_frame(self, position: int, frame: pd.DataFrame) -> None:
        path = self.loaded_files[position][0]
        self.loaded_files[position] = (path, frame)
        self._frames[position] = frame

    def execute(self, command: Any) -> bool:
        if bool(command.is_noop):
            return False
        command.redo(self)
        self._undo.append(command)
        if len(self._undo) > self.max_steps:
            del self._undo[0 : len(self._undo) - self.max_steps]
        self._redo.clear()
        return True

    def undo(self) -> bool:
        if not self._undo:
            return False
        command = self._undo[-1]
        command.undo(self)
        self._undo.pop()
        self._redo.append(command)
        return True

    def redo(self) -> bool:
        if not self._redo:
            return False
        command = self._redo[-1]
        command.redo(self)
        self._redo.pop()
        self._undo.append(command)
        return True


class ColumnPatchCommand:
    def __init__(
        self,
        target: _Target,
        column_position: int,
        positions: Sequence[int],
        before: pd.Series,
        after: pd.Series,
        before_dtype: Any,
        after_dtype: Any,
        before_digest: bytes,
        after_digest: bytes,
    ):
        self.target = target
        self.column_position = column_position
        self.positions = positions
        self.before = before
        self.after = after
        self.before_dtype = before_dtype
        self.after_dtype = after_dtype
        self.before_digest = before_digest
        self.after_digest = after_digest
        self.is_noop = before_digest == after_digest
        self.payload_bytes = (
            _memory_bytes(before) + _memory_bytes(after) + _memory_bytes(positions)
        )

    @classmethod
    def create(
        cls,
        history: HistoryManager,
        file_position: int,
        column: Any,
        row_positions: Iterable[int],
        after_values: Sequence[Any],
    ) -> "ColumnPatchCommand":
        target = history.target(file_position)
        frame = history.loaded_files[file_position][1]
        column_position = _column_position(frame, column)
        normalized_positions = _positions(row_positions, len(frame), "row")
        positions: Sequence[int]
        if normalized_positions == tuple(range(len(frame))):
            positions = range(len(frame))
        else:
            positions = np.asarray(normalized_positions, dtype=np.intp)
        try:
            values = list(after_values)
        except TypeError as exc:
            raise HistoryError("invalid_values", "after values must be iterable") from exc
        if len(values) != len(positions):
            raise HistoryError("invalid_values", "value count does not match row count")
        before_column = frame.iloc[:, column_position]
        before = before_column.iloc[list(positions)].reset_index(drop=True).copy(deep=True)
        candidate = frame.copy(deep=True)
        after_column = candidate.iloc[:, column_position].copy(deep=True)
        try:
            after_column.iloc[list(positions)] = values
            candidate.isetitem(column_position, after_column)
        except Exception as exc:
            raise HistoryError("invalid_values", str(exc)) from exc
        after = (
            candidate.iloc[:, column_position]
            .iloc[list(positions)]
            .reset_index(drop=True)
            .copy(deep=True)
        )
        return cls(
            target,
            column_position,
            positions,
            before,
            after,
            before_column.dtype,
            candidate.dtypes.iloc[column_position],
            _digest(frame),
            _digest(candidate),
        )

    def _apply(
        self,
        history: HistoryManager,
        expected: bytes,
        values: pd.Series,
        dtype: Any,
        result: bytes,
    ) -> None:
        position = history._resolve(self.target)
        current = history.loaded_files[position][1]
        if _digest(current) != expected:
            raise HistoryError("state_mismatch", "target frame changed")
        candidate = current.copy(deep=True)
        column = candidate.iloc[:, self.column_position].copy(deep=True)
        try:
            column.iloc[list(self.positions)] = values.to_numpy(copy=True)
            column = column.astype(dtype)
            candidate.isetitem(self.column_position, column)
        except Exception as exc:
            raise HistoryError("state_mismatch", str(exc)) from exc
        if _digest(candidate) != result:
            raise HistoryError("state_mismatch", "column patch produced an unexpected state")
        history._replace_frame(position, candidate)

    def redo(self, history: HistoryManager) -> None:
        self._apply(history, self.before_digest, self.after, self.after_dtype, self.after_digest)

    def undo(self, history: HistoryManager) -> None:
        self._apply(history, self.after_digest, self.before, self.before_dtype, self.before_digest)


class DeleteRowsCommand:
    def __init__(
        self,
        target: _Target,
        positions: tuple[int, ...],
        keep_positions: tuple[int, ...],
        removed: pd.DataFrame,
        dtypes: tuple[Any, ...],
        before_digest: bytes,
        after_digest: bytes,
    ):
        self.target = target
        self.positions = positions
        self.keep_positions = keep_positions
        self.removed = removed
        self.dtypes = dtypes
        self.before_digest = before_digest
        self.after_digest = after_digest
        self.is_noop = not positions
        self.payload_bytes = _memory_bytes(removed) + len(positions) * 8

    @classmethod
    def create(
        cls, history: HistoryManager, file_position: int, row_positions: Iterable[int]
    ) -> "DeleteRowsCommand":
        target = history.target(file_position)
        frame = history.loaded_files[file_position][1]
        positions = _positions(row_positions, len(frame), "row")
        removed_set = set(positions)
        keep = tuple(position for position in range(len(frame)) if position not in removed_set)
        removed = frame.iloc[list(positions)].copy(deep=True)
        candidate = frame.iloc[list(keep)].copy(deep=True)
        return cls(
            target,
            positions,
            keep,
            removed,
            tuple(frame.dtypes),
            _digest(frame),
            _digest(candidate),
        )

    def redo(self, history: HistoryManager) -> None:
        position = history._resolve(self.target)
        current = history.loaded_files[position][1]
        if _digest(current) != self.before_digest:
            raise HistoryError("state_mismatch", "target frame changed")
        candidate = current.iloc[list(self.keep_positions)].copy(deep=True)
        if _digest(candidate) != self.after_digest:
            raise HistoryError("state_mismatch", "row deletion produced an unexpected state")
        history._replace_frame(position, candidate)

    def undo(self, history: HistoryManager) -> None:
        position = history._resolve(self.target)
        current = history.loaded_files[position][1]
        if _digest(current) != self.after_digest:
            raise HistoryError("state_mismatch", "target frame changed")
        kept = iter(range(len(current)))
        removed = iter(range(len(self.removed)))
        deleted = set(self.positions)
        pieces = []
        for original_position in range(len(current) + len(self.removed)):
            source = self.removed if original_position in deleted else current
            source_position = next(removed) if original_position in deleted else next(kept)
            pieces.append(source.iloc[[source_position]])
        candidate = pd.concat(pieces, axis=0) if pieces else current.copy(deep=True)
        try:
            for column_position, dtype in enumerate(self.dtypes):
                candidate.isetitem(column_position, candidate.iloc[:, column_position].astype(dtype))
        except Exception as exc:
            raise HistoryError("state_mismatch", str(exc)) from exc
        if _digest(candidate) != self.before_digest:
            raise HistoryError("state_mismatch", "row restoration produced an unexpected state")
        history._replace_frame(position, candidate)


@dataclass(frozen=True)
class _DeletedEntry:
    position: int
    path: Any
    frame: pd.DataFrame
    token: object


class DeleteFilesCommand:
    def __init__(self, entries: tuple[_DeletedEntry, ...]):
        self.entries = entries
        self.is_noop = not entries
        self.payload_bytes = sum(8 + len(str(entry.path).encode("utf-8")) for entry in entries)

    @classmethod
    def create(
        cls, history: HistoryManager, file_positions: Iterable[int]
    ) -> "DeleteFilesCommand":
        history._validate_structure()
        positions = _positions(file_positions, len(history.loaded_files), "file")
        return cls(
            tuple(
                _DeletedEntry(
                    position,
                    history.loaded_files[position][0],
                    history.loaded_files[position][1],
                    history._tokens[position],
                )
                for position in positions
            )
        )

    def redo(self, history: HistoryManager) -> None:
        history._validate_structure()
        for entry in self.entries:
            if (
                entry.position >= len(history._tokens)
                or history._tokens[entry.position] is not entry.token
                or history.loaded_files[entry.position][0] != entry.path
                or history.loaded_files[entry.position][1] is not entry.frame
            ):
                raise HistoryError("state_mismatch", "file target changed")
        for entry in reversed(self.entries):
            del history.loaded_files[entry.position]
            del history._tokens[entry.position]
            del history._frames[entry.position]

    def undo(self, history: HistoryManager) -> None:
        history._validate_structure()
        if any(entry.token in history._tokens for entry in self.entries):
            raise HistoryError("state_mismatch", "deleted entry is already present")
        for entry in self.entries:
            history.loaded_files.insert(entry.position, (entry.path, entry.frame))
            history._tokens.insert(entry.position, entry.token)
            history._frames.insert(entry.position, entry.frame)


class CompositeCommand:
    def __init__(self, commands: Sequence[Any]):
        self.commands = tuple(commands)
        self.is_noop = all(bool(command.is_noop) for command in self.commands)
        self.payload_bytes = sum(command.payload_bytes for command in self.commands)

    def redo(self, history: HistoryManager) -> None:
        completed = []
        try:
            for command in self.commands:
                if not command.is_noop:
                    command.redo(history)
                    completed.append(command)
        except Exception:
            for command in reversed(completed):
                command.undo(history)
            raise

    def undo(self, history: HistoryManager) -> None:
        completed = []
        try:
            for command in reversed(self.commands):
                if not command.is_noop:
                    command.undo(history)
                    completed.append(command)
        except Exception:
            for command in reversed(completed):
                command.redo(history)
            raise
