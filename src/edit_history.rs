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
    Columns(usize),
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
struct ReplaceColumnCommand {
    dataset_index: usize,
    column_index: usize,
    previous_values: Vec<f64>,
    source_column: usize,
    operation: ProcessingOperation,
}

#[derive(Debug)]
enum EditCommand {
    Delete(DeleteCommand),
    AddColumn(AddColumnCommand),
    AddColumns(Vec<AddColumnCommand>),
    ReplaceColumns(Vec<ReplaceColumnCommand>),
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
            Self::AddColumns(commands) => commands
                .iter()
                .map(|command| command.column_name.capacity() + size_of::<AddColumnCommand>())
                .sum(),
            Self::ReplaceColumns(commands) => commands
                .iter()
                .map(|command| {
                    command.previous_values.capacity() * size_of::<f64>()
                        + size_of::<ReplaceColumnCommand>()
                })
                .sum(),
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
            Self::AddColumns(commands) => {
                for command in commands.iter().rev() {
                    let dataset = datasets.get(command.dataset_index)?;
                    if dataset
                        .columns
                        .get(command.column_index)
                        .is_none_or(|column| column.name != command.column_name)
                    {
                        return None;
                    }
                }
                for command in commands.iter().rev() {
                    datasets[command.dataset_index]
                        .columns
                        .remove(command.column_index);
                }
                Some(HistoryEffect::Columns(commands.len()))
            }
            Self::ReplaceColumns(commands) => {
                for command in commands {
                    datasets
                        .get(command.dataset_index)?
                        .columns
                        .get(command.column_index)?;
                }
                for command in commands {
                    let values = &mut datasets
                        .get_mut(command.dataset_index)?
                        .columns
                        .get_mut(command.column_index)?
                        .values;
                    *values = command.previous_values.clone();
                }
                Some(HistoryEffect::Columns(commands.len()))
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
            Self::AddColumns(commands) => {
                let mut results = Vec::with_capacity(commands.len());
                for command in commands {
                    let dataset = datasets.get(command.dataset_index)?;
                    if command.column_index > dataset.columns.len() {
                        return None;
                    }
                    let result = processing::apply_to_dataset(
                        dataset,
                        command.source_column,
                        &command.operation,
                    )
                    .ok()?;
                    results.push(result.values);
                }
                for (command, values) in commands.iter().zip(results) {
                    datasets[command.dataset_index].columns.insert(
                        command.column_index,
                        NumericColumn {
                            name: command.column_name.clone(),
                            values,
                        },
                    );
                }
                Some(HistoryEffect::Columns(commands.len()))
            }
            Self::ReplaceColumns(commands) => {
                let mut results = Vec::with_capacity(commands.len());
                for command in commands {
                    let dataset = datasets.get(command.dataset_index)?;
                    results.push(
                        processing::apply_to_dataset(
                            dataset,
                            command.source_column,
                            &command.operation,
                        )
                        .ok()?
                        .values,
                    );
                }
                for (command, values) in commands.iter().zip(results) {
                    datasets[command.dataset_index].columns[command.column_index].values = values;
                }
                Some(HistoryEffect::Columns(commands.len()))
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

    pub fn record_add_columns(
        &mut self,
        columns: Vec<(usize, usize, String, usize, ProcessingOperation)>,
    ) {
        let commands = columns
            .into_iter()
            .map(
                |(dataset_index, column_index, column_name, source_column, operation)| {
                    AddColumnCommand {
                        dataset_index,
                        column_index,
                        column_name,
                        source_column,
                        operation,
                    }
                },
            )
            .collect::<Vec<_>>();
        match commands.len() {
            0 => {}
            1 => self.record(EditCommand::AddColumn(commands.into_iter().next().unwrap())),
            _ => self.record(EditCommand::AddColumns(commands)),
        }
    }

    pub fn record_replace_columns(
        &mut self,
        columns: Vec<(usize, usize, Vec<f64>, usize, ProcessingOperation)>,
    ) {
        let commands = columns
            .into_iter()
            .map(
                |(dataset_index, column_index, previous_values, source_column, operation)| {
                    ReplaceColumnCommand {
                        dataset_index,
                        column_index,
                        previous_values,
                        source_column,
                        operation,
                    }
                },
            )
            .collect::<Vec<_>>();
        if !commands.is_empty() {
            self.record(EditCommand::ReplaceColumns(commands));
        }
    }

    fn record(&mut self, command: EditCommand) {
        for command in self.redo.drain(..) {
            self.retained_bytes = self.retained_bytes.saturating_sub(command.retained_bytes());
        }
        self.retained_bytes += command.retained_bytes();
        self.undo.push_back(command);
        while self.undo.len() > MAX_COMMANDS
            || (self.retained_bytes > MAX_HISTORY_BYTES && self.undo.len() > 1)
        {
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
    use crate::data::{DataSet, DataSetKind, NumericColumn};
    use crate::processing::ProcessingOperation;
    use std::path::PathBuf;

    fn dataset() -> DataSet {
        DataSet {
            source: PathBuf::from("sample.csv"),
            label: None,
            kind: DataSetKind::Source,
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
        history.record_add_columns(vec![(0, 2, "y [对称]".to_owned(), 1, operation)]);

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

    #[test]
    fn formula_column_history_recomputes_with_saved_parameters() {
        let mut datasets = vec![dataset()];
        let operation = ProcessingOperation::Formula {
            x_column: 0,
            expression: "a * y + b".to_owned(),
            a: 2.0,
            b: 3.0,
        };
        let result = crate::processing::apply_to_dataset(&datasets[0], 1, &operation).unwrap();
        datasets[0].columns.push(NumericColumn {
            name: "y [公式]".to_owned(),
            values: result.values,
        });
        let mut history = EditHistory::default();
        history.record_add_columns(vec![(0, 2, "y [公式]".to_owned(), 1, operation)]);

        assert!(history.undo(&mut datasets).is_some());
        assert!(history.redo(&mut datasets).is_some());
        assert_eq!(datasets[0].columns[2].values, [3.0, 5.0, 7.0]);
    }

    #[test]
    fn batch_derived_columns_undo_and_redo_as_one_command() {
        let mut datasets = vec![dataset(), dataset()];
        let operation = ProcessingOperation::Center;
        let mut commands = Vec::new();
        for (dataset_index, dataset) in datasets.iter_mut().enumerate() {
            let result = crate::processing::apply_to_dataset(dataset, 1, &operation).unwrap();
            let column_index = dataset.columns.len();
            dataset.columns.push(NumericColumn {
                name: "y [对称]".to_owned(),
                values: result.values,
            });
            commands.push((
                dataset_index,
                column_index,
                "y [对称]".to_owned(),
                1,
                operation.clone(),
            ));
        }
        let mut history = EditHistory::default();
        history.record_add_columns(commands);

        assert_eq!(history.undo(&mut datasets), Some(HistoryEffect::Columns(2)));
        assert_eq!(datasets[0].columns.len(), 2);
        assert_eq!(datasets[1].columns.len(), 2);
        assert_eq!(history.redo(&mut datasets), Some(HistoryEffect::Columns(2)));
        assert_eq!(datasets[0].columns[2].values, [-1.0, 0.0, 1.0]);
        assert_eq!(datasets[1].columns[2].values, [-1.0, 0.0, 1.0]);
    }

    #[test]
    fn overwritten_columns_restore_the_original_values_on_undo() {
        let mut datasets = vec![dataset()];
        let operation = ProcessingOperation::Center;
        let result = crate::processing::apply_to_dataset(&datasets[0], 1, &operation).unwrap();
        let previous = std::mem::replace(&mut datasets[0].columns[1].values, result.values);
        let mut history = EditHistory::default();
        history.record_replace_columns(vec![(0, 1, previous, 1, operation)]);

        assert_eq!(datasets[0].columns[1].values, [-1.0, 0.0, 1.0]);
        assert_eq!(history.undo(&mut datasets), Some(HistoryEffect::Columns(1)));
        assert_eq!(datasets[0].columns[1].values, [0.0, 1.0, 2.0]);
        assert_eq!(history.redo(&mut datasets), Some(HistoryEffect::Columns(1)));
        assert_eq!(datasets[0].columns[1].values, [-1.0, 0.0, 1.0]);
    }

    #[test]
    fn consecutive_processing_operations_undo_one_step_at_a_time() {
        let mut datasets = vec![dataset()];
        let mut history = EditHistory::default();

        let center = ProcessingOperation::Center;
        let centered = crate::processing::apply_to_dataset(&datasets[0], 1, &center).unwrap();
        let original = std::mem::replace(&mut datasets[0].columns[1].values, centered.values);
        history.record_replace_columns(vec![(0, 1, original, 1, center)]);

        let offset = ProcessingOperation::Formula {
            x_column: 0,
            expression: "y + b".to_owned(),
            a: 1.0,
            b: 10.0,
        };
        let shifted = crate::processing::apply_to_dataset(&datasets[0], 1, &offset).unwrap();
        let centered_values = std::mem::replace(&mut datasets[0].columns[1].values, shifted.values);
        history.record_replace_columns(vec![(0, 1, centered_values, 1, offset)]);

        assert_eq!(datasets[0].columns[1].values, [9.0, 10.0, 11.0]);
        assert_eq!(history.undo(&mut datasets), Some(HistoryEffect::Columns(1)));
        assert_eq!(datasets[0].columns[1].values, [-1.0, 0.0, 1.0]);
        assert_eq!(history.undo(&mut datasets), Some(HistoryEffect::Columns(1)));
        assert_eq!(datasets[0].columns[1].values, [0.0, 1.0, 2.0]);
    }
}
