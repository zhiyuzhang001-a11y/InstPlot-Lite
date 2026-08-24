use std::collections::VecDeque;
use std::mem::size_of;

use crate::data::{DataSet, NumericColumn};
use crate::processing::{self, ProcessingOperation};

const MAX_COMMANDS: usize = 256;
const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum HistoryEffect {
    Rows(usize),
    Column(String),
}

#[derive(Debug)]
struct DeleteCommand {
    dataset_index: usize,
    rows: Vec<usize>,
}

#[derive(Debug)]
struct AddColumnCommand {
    dataset_index: usize,
    column_index: usize,
    column_name: String,
    source_column: usize,
    operation: ProcessingOperation,
}

#[derive(Debug)]
enum EditCommand {
    Delete(DeleteCommand),
    AddColumn(AddColumnCommand),
}

impl EditCommand {
    fn retained_bytes(&self) -> usize {
        match self {
            Self::Delete(command) => {
                command.rows.capacity() * size_of::<usize>() + size_of::<DeleteCommand>()
            }
            Self::AddColumn(command) => {
                command.column_name.capacity() + size_of::<AddColumnCommand>()
            }
        }
    }

    fn undo(&self, datasets: &mut [DataSet]) -> Option<HistoryEffect> {
        match self {
            Self::Delete(command) => {
                datasets
                    .get_mut(command.dataset_index)?
                    .restore_rows(&command.rows);
                Some(HistoryEffect::Rows(command.rows.len()))
            }
            Self::AddColumn(command) => {
                let dataset = datasets.get_mut(command.dataset_index)?;
                if dataset
                    .columns
                    .get(command.column_index)
                    .is_none_or(|column| column.name != command.column_name)
                {
                    return None;
                }
                dataset.columns.remove(command.column_index);
                Some(HistoryEffect::Column(command.column_name.clone()))
            }
        }
    }

    fn redo(&self, datasets: &mut [DataSet]) -> Option<HistoryEffect> {
        match self {
            Self::Delete(command) => {
                let count = datasets
                    .get_mut(command.dataset_index)?
                    .delete_rows(&command.rows)
                    .len();
                Some(HistoryEffect::Rows(count))
            }
            Self::AddColumn(command) => {
                let dataset = datasets.get_mut(command.dataset_index)?;
                if command.column_index > dataset.columns.len() {
                    return None;
                }
                let result = processing::apply_to_dataset(
                    dataset,
                    command.source_column,
                    &command.operation,
                )
                .ok()?;
                dataset.columns.insert(
                    command.column_index,
                    NumericColumn {
                        name: command.column_name.clone(),
                        values: result.values,
                    },
                );
                Some(HistoryEffect::Column(command.column_name.clone()))
            }
        }
    }
}

#[derive(Default)]
pub struct EditHistory {
    undo: VecDeque<EditCommand>,
    redo: Vec<EditCommand>,
    retained_bytes: usize,
}

impl EditHistory {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.retained_bytes = 0;
    }

    pub fn record_delete(&mut self, dataset_index: usize, rows: Vec<usize>) {
        if rows.is_empty() {
            return;
        }
        self.record(EditCommand::Delete(DeleteCommand {
            dataset_index,
            rows,
        }));
    }

    pub fn record_add_column(
        &mut self,
        dataset_index: usize,
        column_index: usize,
        column_name: String,
        source_column: usize,
        operation: ProcessingOperation,
    ) {
        self.record(EditCommand::AddColumn(AddColumnCommand {
            dataset_index,
            column_index,
            column_name,
            source_column,
            operation,
        }));
    }

    fn record(&mut self, command: EditCommand) {
        for command in self.redo.drain(..) {
            self.retained_bytes = self.retained_bytes.saturating_sub(command.retained_bytes());
        }
        self.retained_bytes += command.retained_bytes();
        self.undo.push_back(command);
        while self.undo.len() > MAX_COMMANDS || self.retained_bytes > MAX_HISTORY_BYTES {
            let Some(discarded) = self.undo.pop_front() else {
                break;
            };
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(discarded.retained_bytes());
        }
    }

    pub fn undo(&mut self, datasets: &mut [DataSet]) -> Option<HistoryEffect> {
        let command = self.undo.pop_back()?;
        let Some(effect) = command.undo(datasets) else {
            self.undo.push_back(command);
            return None;
        };
        self.redo.push(command);
        Some(effect)
    }

    pub fn redo(&mut self, datasets: &mut [DataSet]) -> Option<HistoryEffect> {
        let command = self.redo.pop()?;
        let Some(effect) = command.redo(datasets) else {
            self.redo.push(command);
            return None;
        };
        self.undo.push_back(command);
        Some(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::{EditHistory, HistoryEffect};
    use crate::data::{DataSet, NumericColumn};
    use crate::processing::ProcessingOperation;
    use std::path::PathBuf;

    fn dataset() -> DataSet {
        DataSet {
            source: PathBuf::from("sample.csv"),
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "x".to_owned(),
                    values: vec![0.0, 1.0, 2.0],
                },
                NumericColumn {
                    name: "y".to_owned(),
                    values: vec![0.0, 1.0, 2.0],
                },
            ],
            row_count: 3,
            alive: vec![true; 3],
        }
    }

    #[test]
    fn deletion_history_round_trips_without_copying_the_dataset() {
        let mut datasets = vec![dataset()];
        let mut history = EditHistory::default();
        let changed = datasets[0].delete_rows(&[1, 2]);
        history.record_delete(0, changed);
        assert_eq!(history.undo(&mut datasets), Some(HistoryEffect::Rows(2)));
        assert_eq!(datasets[0].alive, [true, true, true]);
        assert_eq!(history.redo(&mut datasets), Some(HistoryEffect::Rows(2)));
        assert_eq!(datasets[0].alive, [true, false, false]);
    }

    #[test]
    fn derived_column_history_recomputes_instead_of_retaining_values() {
        let mut datasets = vec![dataset()];
        let operation = ProcessingOperation::Center;
        let result = crate::processing::apply_to_dataset(&datasets[0], 1, &operation).unwrap();
        datasets[0].columns.push(NumericColumn {
            name: "y [对称]".to_owned(),
            values: result.values,
        });
        let mut history = EditHistory::default();
        history.record_add_column(0, 2, "y [对称]".to_owned(), 1, operation);

        assert_eq!(
            history.undo(&mut datasets),
            Some(HistoryEffect::Column("y [对称]".to_owned()))
        );
        assert_eq!(datasets[0].columns.len(), 2);
        assert_eq!(
            history.redo(&mut datasets),
            Some(HistoryEffect::Column("y [对称]".to_owned()))
        );
        assert_eq!(datasets[0].columns[2].values, [-1.0, 0.0, 1.0]);
    }
}
