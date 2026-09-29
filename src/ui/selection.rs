pub(crate) fn sole_selected_index(selected: &[bool]) -> Option<usize> {
    let mut indices = selected
        .iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(index));
    let first = indices.next()?;
    indices.next().is_none().then_some(first)
}

pub(crate) fn synchronize_selection(selected: &mut Vec<bool>, item_count: usize, new_value: bool) {
    selected.resize(item_count, new_value);
}

#[cfg(test)]
mod tests {
    use super::{sole_selected_index, synchronize_selection};

    #[test]
    fn sole_selection_handles_none_one_and_many_without_indexing_empty_state() {
        assert_eq!(sole_selected_index(&[]), None);
        assert_eq!(sole_selected_index(&[false, false]), None);
        assert_eq!(sole_selected_index(&[false, true, false]), Some(1));
        assert_eq!(sole_selected_index(&[true, true]), None);
    }

    #[test]
    fn selection_state_tracks_items_added_or_removed_while_a_window_is_open() {
        let mut selected = vec![true, false, true];
        synchronize_selection(&mut selected, 1, false);
        assert_eq!(selected, [true]);
        synchronize_selection(&mut selected, 3, false);
        assert_eq!(selected, [true, false, false]);

        let mut columns = vec![false];
        synchronize_selection(&mut columns, 3, true);
        assert_eq!(columns, [false, true, true]);
    }
}
