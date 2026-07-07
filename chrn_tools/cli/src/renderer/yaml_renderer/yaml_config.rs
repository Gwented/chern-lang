/// Configuration for YAML renderer
///
/// `minify` switches the document from block style to flow style: the
/// top-level mapping is wrapped in `{...}`, nested mappings use
/// `{key: value, ...}`, and sequences use `[item, ...]`, all on a single
/// line. The output remains valid YAML 1.2.
#[derive(Debug, Clone, Copy)]
pub(crate) struct YamlRenderConfig {
    pub(crate) minify: bool,
}

impl YamlRenderConfig {
    pub(crate) fn new(minify: bool) -> YamlRenderConfig {
        YamlRenderConfig { minify }
    }
}
