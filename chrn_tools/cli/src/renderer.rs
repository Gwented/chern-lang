use crate::renderer::render_kind::{RenderKind, RenderKindFlat};
use chrn_utils::{
    arena::Arena,
    id_types::SourceRegionId,
    intern::Intern,
    source_map::{
        source_diagnostic::{SourceDiagnostic, footers::FooterKind},
        source_region::SourceRegion,
    },
};

//TODO: Diagnostic unit tests?
//
//NOTE: Json and yaml output use relative spans which probably isn't the best so may turn into
//absolute since why would a tool even know what the absolute position is.
pub(crate) mod json_renderer;
mod output_helpers;
pub(crate) mod render_kind;
pub(crate) mod terminal_renderer;
pub(crate) mod yaml_renderer;

pub fn render(
    diags: &[SourceDiagnostic],
    footers: &[FooterKind],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    render_kind: &RenderKind,
) -> String {
    match render_kind {
        // API returns Vec<String> to keep granularity.
        RenderKind::Terminal(cfg) => terminal_renderer::render_terminal_diags(
            diags,
            footers,
            region_arena_opt,
            interner,
            &cfg,
        )
        .join("\n"),
        RenderKind::Json(cfg) => {
            json_renderer::render_json_diags(diags, footers, region_arena_opt, interner, &cfg)
        }
        RenderKind::Yaml(cfg) => {
            yaml_renderer::render_yaml_diags(diags, footers, region_arena_opt, interner, &cfg)
        }
    }
}
