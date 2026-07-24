//TODO: Diagnostic unit tests?
//
//NOTE: Json and yaml output use relative spans which probably isn't the best so may turn into
//absolute since why would a tool even know what the absolute position is.
pub(crate) mod json_renderer;
mod output_helpers;
pub(crate) mod render_kind;
pub(crate) mod terminal_renderer;
pub(crate) mod yaml_renderer;
