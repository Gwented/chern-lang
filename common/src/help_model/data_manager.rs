use std::path::Path;

use crate::help_model::quote_model::QuoteGraph;

// TEST:
pub(crate) fn load_quote_embedding(path: &Path) -> Result<Vec<Vec<f32>>, ()> {
    todo!();
}

pub(crate) fn save_quote_tensors(path: &Path, q_graph: &QuoteGraph) -> Result<(), ()> {
    todo!();
}
