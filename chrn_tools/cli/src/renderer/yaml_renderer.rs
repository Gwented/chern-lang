pub(super) mod escape;
pub(crate) mod yaml_config;

use chrn_utils::{
    arena::Arena,
    id_types::SourceRegionId,
    intern::Intern,
    source_map::{
        source_diagnostic::{SourceDiagnostic, annotations::Annotation, footers::FooterKind},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

use crate::renderer::{
    output_helpers::{annotation_kind_str, level_str, project_absolute_span, resolve_region_path},
    yaml_renderer::{escape::push_yaml_str, yaml_config::YamlRenderConfig},
};

/// Indent unit (in spaces). Matches the JSON renderer's 2-space indent.
const INDENT: &str = "  ";

/// Renders a slice of source diagnostics as a single pretty-printed YAML
/// document. The shape mirrors [`render_json_diags`]: a top-level mapping
/// with two keys, `diagnostics` and `footers`, holding the same fields and
/// values as the JSON form (with the only differences being syntax and
/// quoting rules).
///
/// IDs are expanded the same way the JSON renderer expands them:
/// - `SourceDiagnostic::path_id` is always resolved to its path string via
///   the `Intern`.
/// - `SourceSpan::region_id` is resolved to its region's path string when
///   the `region_arena` is available, otherwise the field is emitted as
///   `null`.
///
/// The output uses 2-space indentation, block style for mappings and
/// sequences, and plain scalars wherever the string is unambiguous to a
/// YAML 1.2 loader. Ambiguous strings are emitted as double-quoted scalars
/// with the same escape set as the JSON renderer.
///
/// When `cfg.minify` is `true`, the document is emitted on a single line
/// in YAML 1.2 flow style: the top-level mapping is wrapped in `{...}`,
/// nested mappings and sequences use `{key: value, ...}` and `[item, ...]`
/// respectively, and no indentation or newlines are emitted. This produces
/// a string that is still valid YAML 1.2.
pub(crate) fn render_yaml_diags(
    diags: &[SourceDiagnostic],
    footers: &[FooterKind],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    cfg: &YamlRenderConfig,
) -> String {
    let mut out = String::with_capacity(128 + diags.len() * 256);

    // The top-level document is a mapping with two keys. In pretty mode the
    // mapping is block style and each writer emits its own `key:` and value.
    // In minify mode the same writers are reused, but the whole document is
    // wrapped in `{ ... }` and the two key/value pairs are joined by `, `.
    if cfg.minify {
        out.push('{');
    }
    write_diagnostics_list(&mut out, diags, region_arena_opt, interner, 0, cfg);
    if cfg.minify {
        out.push_str(", ");
    }
    write_footers_list(&mut out, footers, 0, cfg);
    if cfg.minify {
        out.push('}');
    }

    out
}

fn write_indent(out: &mut String, level: usize, config: &YamlRenderConfig) {
    if config.minify {
        return;
    }
    for _ in 0..level {
        out.push_str(INDENT);
    }
}

/// Writes `diagnostics:` followed by the diagnostics value. In pretty mode
/// the value is a block-style sequence (`\n  - ...\n  - ...`); in minify
/// mode it is a flow-style sequence (`[ ... , ... ]`) with no leading or
/// trailing newlines. `level` is the indent of the `diagnostics:` key in
/// pretty mode and is ignored in minify mode.
fn write_diagnostics_list(
    out: &mut String,
    diags: &[SourceDiagnostic],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    level: usize,
    config: &YamlRenderConfig,
) {
    if config.minify {
        out.push_str("diagnostics: ");
        if diags.is_empty() {
            out.push_str("[]");
            return;
        }
        out.push('[');
        for (i, diag) in diags.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_diagnostic(out, diag, region_arena_opt, interner, 0, config);
        }
        out.push(']');
    } else {
        write_indent(out, level, config);
        out.push_str("diagnostics:");
        if diags.is_empty() {
            out.push_str(" []\n");
            return;
        }
        out.push('\n');
        for diag in diags {
            write_diagnostic(out, diag, region_arena_opt, interner, level + 1, config);
        }
    }
}

/// Writes a single diagnostic mapping. In pretty mode the mapping is a
/// block-style list item (`<indent>- <field>: <value>\n  <field>: <value>...`).
/// In minify mode the mapping is a flow-style value (`{<field>: <value>,
/// <field>: <value>, ...}`) with no leading `-`, indentation, or newlines.
///
/// `level` is the indent of the `-` in pretty mode and is ignored in
/// minify mode.
fn write_diagnostic(
    out: &mut String,
    diag: &SourceDiagnostic,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    level: usize,
    config: &YamlRenderConfig,
) {
    if config.minify {
        out.push('{');
        write_diagnostic_fields_minify(out, diag, region_arena_opt, interner, config);
        out.push('}');
    } else {
        // Sequence marker + first key on the same line.
        write_indent(out, level, config);
        out.push_str("- ");
        push_yaml_str(out, "level");
        out.push_str(": ");
        push_yaml_str(out, level_str(diag.level));
        out.push('\n');

        // Continuation keys sit at `level + 1` (one indent deeper than `-`,
        // which is the same column the first key started on after the `- `).
        let key_level = level + 1;

        write_indent(out, key_level, config);
        push_yaml_str(out, "message");
        out.push_str(": ");
        push_yaml_str(out, &diag.core_msg);
        out.push('\n');

        let path = interner.search_path(diag.path_id);
        write_indent(out, key_level, config);
        push_yaml_str(out, "path");
        out.push_str(": ");
        push_yaml_str(out, &path.display().to_string());
        out.push('\n');

        // `annotations` is a sub-list whose items live at `key_level + 1`.
        write_indent(out, key_level, config);
        push_yaml_str(out, "annotations");
        write_annotations_list(
            out,
            &diag.annotations,
            region_arena_opt,
            interner,
            key_level,
            config,
        );

        // `help`
        write_indent(out, key_level, config);
        push_yaml_str(out, "help");
        write_string_list(out, &diag.help, key_level, config);

        // `notes` (last field, no trailing newline)
        write_indent(out, key_level, config);
        push_yaml_str(out, "notes");
        write_string_list(out, &diag.notes, key_level, config);
    }
}

/// Writes the fields of one diagnostic in flow style for minify mode.
/// Each field is `key: value` and the fields are joined with `, `; no
/// indentation or newlines are emitted. Container values
/// (`annotations`, `help`, `notes`) are themselves emitted in flow style.
///
/// Takes the `config` so that recursive calls into the general
/// `write_annotation` can receive it. The minify decision is already
/// encoded in the call sites of this helper, so the config's `minify`
/// field is not read here.
fn write_diagnostic_fields_minify(
    out: &mut String,
    diag: &SourceDiagnostic,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    config: &YamlRenderConfig,
) {
    // level
    push_yaml_str(out, "level");
    out.push_str(": ");
    push_yaml_str(out, level_str(diag.level));
    out.push_str(", ");

    // message
    push_yaml_str(out, "message");
    out.push_str(": ");
    push_yaml_str(out, &diag.core_msg);
    out.push_str(", ");

    // path
    let path = interner.search_path(diag.path_id);
    push_yaml_str(out, "path");
    out.push_str(": ");
    push_yaml_str(out, &path.display().to_string());
    out.push_str(", ");

    // annotations: flow-style sequence of flow-style mappings
    push_yaml_str(out, "annotations");
    out.push_str(": ");
    if diag.annotations.is_empty() {
        out.push_str("[]");
    } else {
        out.push('[');
        for (i, ann) in diag.annotations.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_annotation(out, ann, region_arena_opt, interner, 0, config);
        }
        out.push(']');
    }
    out.push_str(", ");

    // help: flow-style sequence of strings
    push_yaml_str(out, "help");
    out.push_str(": ");
    write_inline_string_sequence(out, &diag.help);
    out.push_str(", ");

    // notes: flow-style sequence of strings (last field, no trailing ", ")
    push_yaml_str(out, "notes");
    out.push_str(": ");
    write_inline_string_sequence(out, &diag.notes);
}

/// Writes `items` as a flow-style YAML sequence: `[a, b, c]` or `[]` when
/// empty. Each item is pushed through `push_yaml_str` so the same
/// quoting rules as the rest of the renderer apply.
fn write_inline_string_sequence(out: &mut String, items: &[String]) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        push_yaml_str(out, item);
    }
    out.push(']');
}

/// Writes `annotations:` followed by the annotations value. The caller has
/// already written the `annotations` key; this function emits the `:`
/// separator and the value. In pretty mode the value is a block-style
/// sequence (`\n    - ...\n    - ...`); in minify mode it is a flow-style
/// sequence (`[ ... , ... ]`) with no leading or trailing newlines.
fn write_annotations_list(
    out: &mut String,
    annotations: &[Annotation],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    level: usize,
    config: &YamlRenderConfig,
) {
    if annotations.is_empty() {
        out.push_str(if config.minify { ": []" } else { ": []\n" });
        return;
    }
    if config.minify {
        out.push_str(": [");
        for (i, ann) in annotations.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_annotation(out, ann, region_arena_opt, interner, 0, config);
        }
        out.push(']');
    } else {
        out.push_str(":\n");
        for ann in annotations {
            write_annotation(out, ann, region_arena_opt, interner, level + 1, config);
        }
    }
}

/// Writes a single annotation. In pretty mode the annotation is a
/// block-style list item (`<indent>- <field>: <value>\n  <field>: <value>...`).
/// In minify mode the annotation is a flow-style value (`{<field>:
/// <value>, ...}`). `level` is the indent of the `-` in pretty mode and is
/// ignored in minify mode.
fn write_annotation(
    out: &mut String,
    ann: &Annotation,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    level: usize,
    config: &YamlRenderConfig,
) {
    if config.minify {
        out.push('{');
        // kind
        push_yaml_str(out, "kind");
        out.push_str(": ");
        push_yaml_str(out, annotation_kind_str(ann.kind));
        out.push_str(", ");
        // label
        push_yaml_str(out, "label");
        out.push_str(": ");
        match &ann.label {
            Some(label) => push_yaml_str(out, label),
            None => out.push_str("null"),
        }
        out.push_str(", ");
        // span (sub-mapping in flow style)
        push_yaml_str(out, "span");
        out.push_str(": ");
        write_span_inline(out, &ann.span, region_arena_opt, interner);
        out.push('}');
    } else {
        write_indent(out, level, config);
        out.push_str("- ");
        push_yaml_str(out, "kind");
        out.push_str(": ");
        push_yaml_str(out, annotation_kind_str(ann.kind));
        out.push('\n');

        let key_level = level + 1;

        write_indent(out, key_level, config);
        push_yaml_str(out, "label");
        out.push_str(": ");
        match &ann.label {
            Some(label) => push_yaml_str(out, label),
            None => out.push_str("null"),
        }
        out.push('\n');

        // `span` is a sub-mapping whose keys live at `key_level + 1`.
        write_indent(out, key_level, config);
        push_yaml_str(out, "span");
        write_span(
            out,
            &ann.span,
            region_arena_opt,
            interner,
            key_level,
            config,
        );
    }
}

/// Writes `span:` followed by the span value. The caller has already
/// written the `span` key; this function emits the `:` separator and the
/// value. In pretty mode the value is a block-style mapping
/// (`\n    region_path: ...\n    start: ...\n    end: ...`); in minify
/// mode it is a flow-style mapping (`{region_path: ..., start: ...,
/// end: ...}`).
fn write_span(
    out: &mut String,
    span: &SourceSpan,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    level: usize,
    config: &YamlRenderConfig,
) {
    if config.minify {
        // `write_span` is only called from `write_annotation`, which has
        // already written the `span` key. The minify path is delegated to
        // a dedicated inline writer so the wrapping `: { ... }` is
        // emitted consistently.
        out.push_str(": ");
        write_span_inline(out, span, region_arena_opt, interner);
    } else {
        out.push_str(":\n");

        let key_level = level + 1;

        write_indent(out, key_level, config);
        push_yaml_str(out, "region_path");
        out.push_str(": ");
        match resolve_region_path(region_arena_opt, interner, span.region_id) {
            Some(path) => push_yaml_str(out, &path),
            None => out.push_str("null"),
        }
        out.push('\n');

        // The pipeline emits relative offsets into the owning region, so
        // the renderer projects them to absolute file positions using
        // the region's `script_start` before emitting them. External
        // tooling receives absolute offsets and does not need the
        // region metadata to interpret them.
        let (abs_start, abs_end) = project_absolute_span(region_arena_opt, span);
        write_indent(out, key_level, config);
        push_yaml_str(out, "start");
        out.push_str(": ");
        out.push_str(&abs_start.to_string());
        out.push('\n');

        write_indent(out, key_level, config);
        push_yaml_str(out, "end");
        out.push_str(": ");
        out.push_str(&abs_end.to_string());
        out.push('\n');
    }
}

/// Writes a span mapping in flow style. Always wraps the value in `{...}`
/// with fields separated by `, `; intended for the minify code path where
/// the caller has already emitted the `span:` key and `: ` separator.
fn write_span_inline(
    out: &mut String,
    span: &SourceSpan,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
) {
    out.push('{');
    push_yaml_str(out, "region_path");
    out.push_str(": ");
    match resolve_region_path(region_arena_opt, interner, span.region_id) {
        Some(path) => push_yaml_str(out, &path),
        None => out.push_str("null"),
    }
    out.push_str(", ");
    // The pipeline emits relative offsets into the owning region, so
    // the renderer projects them to absolute file positions using the
    // region's `script_start` before emitting them. External tooling
    // receives absolute offsets and does not need the region metadata
    // to interpret them.
    let (abs_start, abs_end) = project_absolute_span(region_arena_opt, span);
    push_yaml_str(out, "start");
    out.push_str(": ");
    out.push_str(&abs_start.to_string());
    out.push_str(", ");
    push_yaml_str(out, "end");
    out.push_str(": ");
    out.push_str(&abs_end.to_string());
    out.push('}');
}

/// Writes `key:` followed by the list value. The caller has already
/// written the key. In pretty mode the value is a block-style sequence
/// (`\n  - item\n  - item`); in minify mode it is a flow-style sequence
/// (`[item, item]`) with no leading or trailing newlines.
fn write_string_list(out: &mut String, items: &[String], level: usize, config: &YamlRenderConfig) {
    if items.is_empty() {
        out.push_str(if config.minify { ": []" } else { ": []\n" });
        return;
    }
    if config.minify {
        out.push_str(": ");
        write_inline_string_sequence(out, items);
    } else {
        out.push_str(":\n");
        for item in items {
            write_indent(out, level + 1, config);
            out.push_str("- ");
            push_yaml_str(out, item);
            out.push('\n');
        }
    }
}

/// Writes `footers:` followed by the footers value. In pretty mode the
/// value is a block-style sequence (`\n  - ...\n  - ...`); in minify mode
/// it is a flow-style sequence (`[ ... , ... ]`). `level` is the indent
/// of the `footers:` key in pretty mode and is ignored in minify mode.
fn write_footers_list(
    out: &mut String,
    footers: &[FooterKind],
    level: usize,
    config: &YamlRenderConfig,
) {
    if config.minify {
        out.push_str("footers: ");
        if footers.is_empty() {
            out.push_str("[]");
            return;
        }
        out.push('[');
        for (i, footer) in footers.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_footer(out, footer, 0, config);
        }
        out.push(']');
    } else {
        write_indent(out, level, config);
        out.push_str("footers:");
        if footers.is_empty() {
            out.push_str(" []\n");
            return;
        }
        out.push('\n');
        for (i, footer) in footers.iter().enumerate() {
            write_footer(out, footer, level + 1, config);
            if i + 1 < footers.len() {
                out.push('\n');
            }
        }
    }
}

/// Writes a single footer. In pretty mode the footer is a block-style
/// list item (`<indent>- <field>: <value>\n  <field>: <value>...`). In
/// minify mode the footer is a flow-style value (`{<field>: <value>,
/// ...}`). `level` is the indent of the `-` in pretty mode and is
/// ignored in minify mode. No trailing newline.
fn write_footer(out: &mut String, footer: &FooterKind, level: usize, config: &YamlRenderConfig) {
    if config.minify {
        out.push('{');
        match footer {
            FooterKind::DiagnosticsExceeded(count) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "diagnostics_exceeded");
                out.push_str(", ");
                push_yaml_str(out, "count");
                out.push_str(": ");
                out.push_str(&count.to_string());
            }
            FooterKind::MaxModulesExceeded(max) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "max_modules_exceeded");
                out.push_str(", ");
                push_yaml_str(out, "max");
                out.push_str(": ");
                out.push_str(&max.to_string());
            }
            FooterKind::ErrorsEmitted(count) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "errors_emitted");
                out.push_str(", ");
                push_yaml_str(out, "count");
                out.push_str(": ");
                out.push_str(&count.to_string());
            }
            FooterKind::WarnsEmitted(count) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "warns_emitted");
                out.push_str(", ");
                push_yaml_str(out, "count");
                out.push_str(": ");
                out.push_str(&count.to_string());
            }
        }
        out.push('}');
    } else {
        write_indent(out, level, config);
        out.push_str("- ");
        let key_level = level + 1;

        match footer {
            FooterKind::DiagnosticsExceeded(count) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "diagnostics_exceeded");
                out.push('\n');

                write_indent(out, key_level, config);
                push_yaml_str(out, "count");
                out.push_str(": ");
                out.push_str(&count.to_string());
            }
            FooterKind::MaxModulesExceeded(max) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "max_modules_exceeded");
                out.push('\n');

                write_indent(out, key_level, config);
                push_yaml_str(out, "max");
                out.push_str(": ");
                out.push_str(&max.to_string());
            }
            FooterKind::ErrorsEmitted(count) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "errors_emitted");
                out.push('\n');

                write_indent(out, key_level, config);
                push_yaml_str(out, "count");
                out.push_str(": ");
                out.push_str(&count.to_string());
            }
            FooterKind::WarnsEmitted(count) => {
                push_yaml_str(out, "kind");
                out.push_str(": ");
                push_yaml_str(out, "warns_emitted");
                out.push_str(", ");
                push_yaml_str(out, "count");
                out.push_str(": ");
                out.push_str(&count.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrn_utils::{
        arena::Arena,
        id_types::SourceRegionId,
        intern::Intern,
        source_map::{
            source_diagnostic::{
                DiagnosticLevel, SourceDiagnostic,
                annotations::{Annotation, AnnotationKind},
            },
            source_region::SourceRegion,
            source_span::SourceSpan,
        },
    };

    #[test]
    fn empty_diags_and_footers_produces_valid_shape() {
        let interner = Intern::init();
        let out = render_yaml_diags(&[], &[], None, &interner, &YamlRenderConfig::new(false));
        assert_eq!(out, "diagnostics: []\nfooters: []\n");
    }

    #[test]
    fn diagnostic_fields_and_annotation_are_emitted() {
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/a.chrn"));

        let span = SourceSpan::new(SourceRegionId::new(0), 4, 9);
        let ann = Annotation::new(span, AnnotationKind::Primary, Some("here".to_string()));
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Error,
            "boom".to_string(),
            path_id,
            vec![ann],
            vec!["try this".to_string()],
            vec!["note one".to_string()],
        );

        let out = render_yaml_diags(&[diag], &[], None, &interner, &YamlRenderConfig::new(false));
        // First key of the diagnostic sits on the same line as `-`.
        assert!(
            out.contains("- level: error\n"),
            "missing '- level: error' in:\n{out}"
        );
        // Continuation keys are aligned with `level` (column 4).
        assert!(out.contains("    message: boom\n"), "got:\n{out}");
        assert!(out.contains("    path: /tmp/a.chrn\n"), "got:\n{out}");
        // annotations sub-list lives at column 4, with items at column 6.
        assert!(out.contains("    annotations:\n"), "got:\n{out}");
        assert!(out.contains("      - kind: primary\n"), "got:\n{out}");
        assert!(out.contains("        label: here\n"), "got:\n{out}");
        // span sub-mapping lives at column 8, with keys at column 10.
        assert!(out.contains("        span:\n"), "got:\n{out}");
        assert!(out.contains("          region_path: null\n"), "got:\n{out}");
        assert!(out.contains("          start: 4\n"), "got:\n{out}");
        assert!(out.contains("          end: 9\n"), "got:\n{out}");
        // help / notes: each item at column 6.
        assert!(out.contains("    help:\n"), "got:\n{out}");
        assert!(out.contains("      - try this\n"), "got:\n{out}");
        assert!(out.contains("    notes:\n"), "got:\n{out}");
        assert!(out.contains("      - note one\n"), "got:\n{out}");
    }

    #[test]
    fn null_label_is_emitted() {
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/b.chrn"));
        let span = SourceSpan::new(SourceRegionId::new(0), 0, 1);
        let ann = Annotation::new(span, AnnotationKind::Secondary, None);
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Warn,
            String::new(),
            path_id,
            vec![ann],
            Vec::new(),
            Vec::new(),
        );

        let out = render_yaml_diags(&[diag], &[], None, &interner, &YamlRenderConfig::new(false));
        assert!(out.contains("        label: null\n"), "got:\n{out}");
    }

    #[test]
    fn diagnostics_exceeded_footer_is_emitted() {
        let interner = Intern::init();
        let out = render_yaml_diags(
            &[],
            &[FooterKind::DiagnosticsExceeded(7)],
            None,
            &interner,
            &YamlRenderConfig::new(false),
        );
        assert!(
            out.contains("- kind: diagnostics_exceeded\n"),
            "got:\n{out}"
        );
        // `count: 7` is the last thing written; the document ends without
        // a trailing newline (matches the JSON renderer's convention).
        assert!(out.contains("    count: 7"), "got:\n{out}");
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn minified_output_is_single_line_flow_style() {
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/a.chrn"));
        let span = SourceSpan::new(SourceRegionId::new(0), 4, 9);
        let ann = Annotation::new(span, AnnotationKind::Primary, Some("here".to_string()));
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Error,
            "boom".to_string(),
            path_id,
            vec![ann],
            vec!["try this".to_string()],
            vec!["note one".to_string()],
        );

        let out = render_yaml_diags(&[diag], &[], None, &interner, &YamlRenderConfig::new(true));
        // Minified output is a single line, no indentation, no newlines.
        assert!(!out.contains('\n'), "minified output had a newline: {out}");
        assert!(
            !out.contains("  "),
            "minified output had indentation: {out}"
        );
        // Top-level is wrapped in `{...}` and joins the two keys with `, `.
        assert!(out.starts_with('{'));
        assert!(out.ends_with('}'));
        assert!(out.contains("diagnostics: ["), "got: {out}");
        assert!(out.contains(", footers: []"), "got: {out}");
        // Diagnostics are flow-style mappings joined by `, ` inside `[...]`.
        assert!(out.contains("{level: error,"), "got: {out}");
        assert!(out.contains("message: boom,"), "got: {out}");
        // Annotations is a flow-style sequence of flow-style mappings.
        assert!(out.contains("annotations: [{kind: primary,"), "got: {out}");
        // Span is a flow-style mapping with `null` for the missing region.
        assert!(out.contains("span: {region_path: null,"), "got: {out}");
        // help / notes are flow-style sequences of strings.
        assert!(out.contains("help: [try this]"), "got: {out}");
        assert!(out.contains("notes: [note one]"), "got: {out}");
    }

    #[test]
    fn minified_empty_diags_and_footers_is_compact() {
        let interner = Intern::init();
        let out = render_yaml_diags(&[], &[], None, &interner, &YamlRenderConfig::new(true));
        assert_eq!(out, "{diagnostics: [], footers: []}");
    }

    #[test]
    fn minified_footer_is_emitted_in_flow_style() {
        let interner = Intern::init();
        let out = render_yaml_diags(
            &[],
            &[FooterKind::DiagnosticsExceeded(7)],
            None,
            &interner,
            &YamlRenderConfig::new(true),
        );
        assert_eq!(
            out,
            r#"{diagnostics: [], footers: [{kind: diagnostics_exceeded, count: 7}]}"#
        );
    }

    #[test]
    fn max_modules_exceeded_footer_is_emitted() {
        let interner = Intern::init();
        let out = render_yaml_diags(
            &[],
            &[FooterKind::MaxModulesExceeded(256)],
            None,
            &interner,
            &YamlRenderConfig::new(false),
        );
        assert!(
            out.contains("- kind: max_modules_exceeded\n"),
            "got:\n{out}"
        );
        assert!(out.contains("    max: 256"), "got:\n{out}");
        assert!(!out.ends_with('\n'));
    }

    #[test]
    fn minified_max_modules_exceeded_footer_is_emitted_in_flow_style() {
        let interner = Intern::init();
        let out = render_yaml_diags(
            &[],
            &[FooterKind::MaxModulesExceeded(256)],
            None,
            &interner,
            &YamlRenderConfig::new(true),
        );
        assert_eq!(
            out,
            r#"{diagnostics: [], footers: [{kind: max_modules_exceeded, max: 256}]}"#
        );
    }

    #[test]
    fn span_offsets_are_projected_to_absolute_when_region_is_known() {
        // A region that begins at absolute byte 128 within the file. The
        // loader strips everything before `@def` from `src_bytes`, so
        // spans emitted by the pipeline are relative to byte 0 of the
        // region. The renderer must add the region's `script_start` to
        // those relative offsets so external tooling sees an absolute
        // file position.
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/a.chrn"));
        let region_id = SourceRegionId::new(0);

        let region = SourceRegion::new(
            1,
            1,
            b"@def\nhello\n@end\n".to_vec(),
            region_id,
            path_id,
            128,
            Some(140),
        );
        let arena: Arena<SourceRegion, SourceRegionId> = Arena::from(vec![region]);

        let span = SourceSpan::new(region_id, 4, 9);
        let ann = Annotation::new(span, AnnotationKind::Primary, Some("here".to_string()));
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Error,
            "boom".to_string(),
            path_id,
            vec![ann],
            Vec::new(),
            Vec::new(),
        );

        let out = render_yaml_diags(
            &[diag],
            &[],
            Some(&arena),
            &interner,
            &YamlRenderConfig::new(false),
        );
        assert!(
            out.contains("          start: 132\n"),
            "expected absolute start in pretty output, got:\n{out}"
        );
        assert!(
            out.contains("          end: 137\n"),
            "expected absolute end in pretty output, got:\n{out}"
        );
    }

    #[test]
    fn span_offsets_are_projected_to_absolute_in_minified_output() {
        // Same projection must hold for the minify path so flow-style
        // consumers also receive absolute byte offsets.
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/a.chrn"));
        let region_id = SourceRegionId::new(0);

        let region = SourceRegion::new(
            1,
            1,
            b"@def\nhello\n@end\n".to_vec(),
            region_id,
            path_id,
            200,
            Some(212),
        );
        let arena: Arena<SourceRegion, SourceRegionId> = Arena::from(vec![region]);

        let span = SourceSpan::new(region_id, 4, 9);
        let ann = Annotation::new(span, AnnotationKind::Primary, Some("here".to_string()));
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Error,
            "boom".to_string(),
            path_id,
            vec![ann],
            Vec::new(),
            Vec::new(),
        );

        let out = render_yaml_diags(
            &[diag],
            &[],
            Some(&arena),
            &interner,
            &YamlRenderConfig::new(true),
        );
        // 4 + 200 == 204, 9 + 200 == 209
        assert!(
            out.contains("start: 204,"),
            "expected absolute start in minified output, got: {out}"
        );
        assert!(
            out.contains("end: 209}"),
            "expected absolute end in minified output, got: {out}"
        );
    }

    #[test]
    fn span_offsets_pass_through_when_region_is_missing_from_arena() {
        // If the region arena is provided but the span's region is not
        // in it, the renderer cannot recover an absolute position. It
        // must still emit a well-formed document with the raw relative
        // values, rather than crashing or silently shifting the offset
        // by an unknown amount.
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/a.chrn"));

        let arena: Arena<SourceRegion, SourceRegionId> = Arena::new();

        let span = SourceSpan::new(SourceRegionId::new(0), 4, 9);
        let ann = Annotation::new(span, AnnotationKind::Primary, Some("here".to_string()));
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Error,
            "boom".to_string(),
            path_id,
            vec![ann],
            Vec::new(),
            Vec::new(),
        );

        let out = render_yaml_diags(
            &[diag],
            &[],
            Some(&arena),
            &interner,
            &YamlRenderConfig::new(false),
        );
        assert!(out.contains("          start: 4\n"), "got:\n{out}");
        assert!(out.contains("          end: 9\n"), "got:\n{out}");
    }
}
