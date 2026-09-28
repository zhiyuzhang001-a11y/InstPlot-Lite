#[derive(Default)]
pub(crate) struct FitResultState {
    pub(crate) overlays: Vec<FitOverlay>,
}

pub(crate) struct FitOverlay {
    pub(crate) points: Vec<[f64; 2]>,
    pub(crate) r2: f64,
    pub(crate) equation: String,
    pub(crate) display_equation: String,
    pub(crate) name: String,
    pub(crate) target: FitTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FitTarget {
    pub(crate) dataset_index: Option<usize>,
    pub(crate) source_dataset_ids: Vec<String>,
    pub(crate) x_column_name: String,
    pub(crate) y_column_name: String,
}
