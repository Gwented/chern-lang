/// Configuration for JSON renderer.
///
/// `minify` strips all insignificant whitespace from the rendered document
/// (newlines, indentation, and the space after `:` and `,`) while leaving
/// whitespace inside string values untouched.
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsonRenderConfig {
    pub(crate) minify: bool,
}

impl JsonRenderConfig {
    pub(crate) fn new(minify: bool) -> JsonRenderConfig {
        JsonRenderConfig { minify }
    }
}
