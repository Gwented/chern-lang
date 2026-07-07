pub(super) mod escape;
pub(crate) mod json_config;

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
    json_renderer::{escape::push_json_str, json_config::JsonRenderConfig},
    output_helpers::{annotation_kind_str, level_str, resolve_region_path},
};

/// Renders a slice of source diagnostics as a single pretty-printed JSON document.
///
/// IDs are expanded where it adds value for a JSON consumer:
/// - `SourceDiagnostic::path_id` is always resolved to its path string via the `Intern`.
/// - `SourceSpan::region_id` is resolved to its region's path string when the
///   `region_arena` is available, otherwise the field is emitted as `null`.
///
/// Internal arena indices are intentionally not emitted. The span keeps its raw
/// byte offsets (`start`, `end`).
///
/// When `cfg.minify` is `true`, the pretty output is post-processed to strip
/// all insignificant whitespace (newlines, indentation, and the space after `:`
/// and `,`). Whitespace inside JSON string values is preserved verbatim by
/// tracking the string state while scanning.
pub(crate) fn render_json_diags(
    diags: &[SourceDiagnostic],
    footers: &[FooterKind],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    cfg: &JsonRenderConfig,
) -> String {
    let pretty = render_json_diags_pretty(diags, footers, region_arena_opt, interner);
    if cfg.minify {
        minify_json(&pretty)
    } else {
        pretty
    }
}

/// Renders the pretty (indented) form of a JSON document. All minified
/// output goes through this first, so any change to the canonical shape
/// automatically applies to both modes.
fn render_json_diags_pretty(
    diags: &[SourceDiagnostic],
    footers: &[FooterKind],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
) -> String {
    // Rough pre-allocation: header overhead + a small amount per diagnostic.
    let mut out = String::with_capacity(128 + diags.len() * 256);
    out.push_str("{\n");

    write_diagnostics_array(&mut out, diags, region_arena_opt, interner, 1);
    out.push_str(",\n");
    write_footers_array(&mut out, footers, 1);

    out.push_str("\n}");
    out
}

/// Strips all insignificant whitespace from a valid JSON document. The input
/// is assumed to be well-formed JSON (the pretty output produced by
/// [`render_json_diags_pretty`]); whitespace inside string values is left
/// alone by tracking the string state and escape sequences while scanning.
fn minify_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut escape_next = false;

    for c in s.chars() {
        if in_string {
            out.push(c);
            if escape_next {
                // The character after a backslash is part of the escape
                // sequence; it can be `"` or `\` and must be passed through
                // without flipping `in_string` back to false.
                escape_next = false;
            } else if c == '\\' {
                escape_next = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }

    out
}

fn write_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_diagnostics_array(
    out: &mut String,
    diags: &[SourceDiagnostic],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    depth: usize,
) {
    write_indent(out, depth);
    push_json_str(out, "diagnostics");
    out.push_str(": ");

    if diags.is_empty() {
        out.push_str("[]");
        return;
    }

    out.push('[');
    out.push('\n');
    for (i, diag) in diags.iter().enumerate() {
        if i > 0 {
            out.push(',');
            out.push('\n');
        }
        write_diagnostic(out, diag, region_arena_opt, interner, depth + 1);
    }
    out.push('\n');
    write_indent(out, depth);
    out.push(']');
}

fn write_diagnostic(
    out: &mut String,
    diag: &SourceDiagnostic,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    depth: usize,
) {
    write_indent(out, depth);
    out.push('{');
    out.push('\n');

    // "level"
    write_indent(out, depth + 1);
    push_json_str(out, "level");
    out.push_str(": ");
    push_json_str(out, level_str(diag.level));
    out.push(',');
    out.push('\n');

    // "message"
    write_indent(out, depth + 1);
    push_json_str(out, "message");
    out.push_str(": ");
    push_json_str(out, &diag.core_msg);
    out.push(',');
    out.push('\n');

    // "path" (always expanded)
    let path = interner.search_path(diag.path_id);
    write_indent(out, depth + 1);
    push_json_str(out, "path");
    out.push_str(": ");
    push_json_str(out, &path.display().to_string());
    out.push(',');
    out.push('\n');

    // "annotations"
    write_indent(out, depth + 1);
    push_json_str(out, "annotations");
    out.push_str(": ");
    write_annotations_array(
        out,
        &diag.annotations,
        region_arena_opt,
        interner,
        depth + 1,
    );
    out.push(',');
    out.push('\n');

    // "help"
    write_indent(out, depth + 1);
    push_json_str(out, "help");
    out.push_str(": ");
    write_string_array(out, &diag.help, depth + 1);
    out.push(',');
    out.push('\n');

    // "notes" (last field, no trailing comma)
    write_indent(out, depth + 1);
    push_json_str(out, "notes");
    out.push_str(": ");
    write_string_array(out, &diag.notes, depth + 1);

    out.push('\n');
    write_indent(out, depth);
    out.push('}');
}

fn write_annotations_array(
    out: &mut String,
    annotations: &[Annotation],
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    depth: usize,
) {
    if annotations.is_empty() {
        out.push_str("[]");
        return;
    }

    out.push('[');
    out.push('\n');
    for (i, ann) in annotations.iter().enumerate() {
        if i > 0 {
            out.push(',');
            out.push('\n');
        }
        write_annotation(out, ann, region_arena_opt, interner, depth + 1);
    }
    out.push('\n');
    write_indent(out, depth);
    out.push(']');
}

fn write_annotation(
    out: &mut String,
    ann: &Annotation,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    depth: usize,
) {
    write_indent(out, depth);
    out.push('{');
    out.push('\n');

    // "kind"
    write_indent(out, depth + 1);
    push_json_str(out, "kind");
    out.push_str(": ");
    push_json_str(out, annotation_kind_str(ann.kind));
    out.push(',');
    out.push('\n');

    // "label" (null when absent)
    write_indent(out, depth + 1);
    push_json_str(out, "label");
    out.push_str(": ");
    match &ann.label {
        Some(label) => push_json_str(out, label),
        None => out.push_str("null"),
    }
    out.push(',');
    out.push('\n');

    // "span" (last field, no trailing comma)
    write_indent(out, depth + 1);
    push_json_str(out, "span");
    out.push_str(": ");
    write_span(out, &ann.span, region_arena_opt, interner, depth + 1);

    out.push('\n');
    write_indent(out, depth);
    out.push('}');
}

fn write_span(
    out: &mut String,
    span: &SourceSpan,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    depth: usize,
) {
    out.push('{');
    out.push('\n');

    // "region_path" (expanded when arena is available, else null)
    write_indent(out, depth + 1);
    push_json_str(out, "region_path");
    out.push_str(": ");
    match resolve_region_path(region_arena_opt, interner, span.region_id) {
        Some(path) => push_json_str(out, &path),
        None => out.push_str("null"),
    }
    out.push(',');
    out.push('\n');

    // "start" / "end" (last fields, no trailing comma)
    write_indent(out, depth + 1);
    push_json_str(out, "start");
    out.push_str(": ");
    out.push_str(&span.start.to_string());
    out.push(',');
    out.push('\n');

    write_indent(out, depth + 1);
    push_json_str(out, "end");
    out.push_str(": ");
    out.push_str(&span.end.to_string());

    out.push('\n');
    write_indent(out, depth);
    out.push('}');
}

fn write_string_array(out: &mut String, items: &[String], depth: usize) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }

    out.push('[');
    out.push('\n');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
            out.push('\n');
        }
        write_indent(out, depth + 1);
        push_json_str(out, item);
    }
    out.push('\n');
    write_indent(out, depth);
    out.push(']');
}

fn write_footers_array(out: &mut String, footers: &[FooterKind], depth: usize) {
    write_indent(out, depth);
    push_json_str(out, "footers");
    out.push_str(": ");

    if footers.is_empty() {
        out.push_str("[]");
        return;
    }

    out.push('[');
    out.push('\n');
    for (i, footer) in footers.iter().enumerate() {
        if i > 0 {
            out.push(',');
            out.push('\n');
        }
        write_footer(out, footer, depth + 1);
    }
    out.push('\n');
    write_indent(out, depth);
    out.push(']');
}

fn write_footer(out: &mut String, footer: &FooterKind, depth: usize) {
    write_indent(out, depth);
    out.push('{');
    out.push('\n');

    match footer {
        FooterKind::DiagnosticsExceeded(count) => {
            write_indent(out, depth + 1);
            push_json_str(out, "kind");
            out.push_str(": ");
            push_json_str(out, "diagnostics_exceeded");
            out.push(',');
            out.push('\n');

            write_indent(out, depth + 1);
            push_json_str(out, "count");
            out.push_str(": ");
            out.push_str(&count.to_string());
        }
    }

    out.push('\n');
    write_indent(out, depth);
    out.push('}');
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrn_utils::{
        id_types::{PathId, SourceRegionId},
        intern::Intern,
        source_map::{
            source_diagnostic::{
                DiagnosticLevel, SourceDiagnostic,
                annotations::{Annotation, AnnotationKind},
            },
            source_span::SourceSpan,
        },
    };

    #[test]
    fn empty_diags_and_footers_produces_valid_shape() {
        let interner = Intern::init();
        let out = render_json_diags(&[], &[], None, &interner, &JsonRenderConfig::new(false));
        assert_eq!(out, "{\n  \"diagnostics\": [],\n  \"footers\": []\n}");
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

        let out = render_json_diags(&[diag], &[], None, &interner, &JsonRenderConfig::new(false));
        assert!(out.contains("\"level\": \"error\""));
        assert!(out.contains("\"message\": \"boom\""));
        assert!(out.contains("\"path\": \"/tmp/a.chrn\""));
        assert!(out.contains("\"kind\": \"primary\""));
        assert!(out.contains("\"label\": \"here\""));
        assert!(out.contains("\"region_path\": null"));
        assert!(out.contains("\"start\": 4"));
        assert!(out.contains("\"end\": 9"));
        assert!(out.contains("\"help\": [\n        \"try this\"\n      ]"));
        assert!(out.contains("\"notes\": [\n        \"note one\"\n      ]"));
        // Should not have a trailing comma in either the inner object or the top-level.
        assert!(!out.contains(",\n}"));
        // Should end with a single closing brace, not with a trailing newline.
        assert!(out.ends_with('}'));
        assert!(!out.ends_with('\n'));
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

        let out = render_json_diags(&[diag], &[], None, &interner, &JsonRenderConfig::new(false));
        assert!(out.contains("\"label\": null"));
    }

    #[test]
    fn diagnostics_exceeded_footer_is_emitted() {
        let interner = Intern::init();
        let out = render_json_diags(
            &[],
            &[FooterKind::DiagnosticsExceeded(7)],
            None,
            &interner,
            &JsonRenderConfig::new(false),
        );
        assert!(out.contains("\"kind\": \"diagnostics_exceeded\""));
        assert!(out.contains("\"count\": 7"));
    }

    #[test]
    fn minified_output_strips_insignificant_whitespace() {
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

        let out = render_json_diags(&[diag], &[], None, &interner, &JsonRenderConfig::new(true));
        // No newlines or indentation should remain.
        assert!(!out.contains('\n'), "minified output had a newline: {out}");
        assert!(
            !out.contains("  "),
            "minified output had indentation: {out}"
        );
        // Tokens should sit directly next to the colon and comma (no `": "` or `, `).
        assert!(out.contains(r#""level":"error""#), "got: {out}");
        assert!(out.contains(r#""message":"boom""#), "got: {out}");
        assert!(out.contains(r#""path":"/tmp/a.chrn""#), "got: {out}");
        // Sequence and nested object should still parse.
        assert!(out.contains(r#""help":["try this"]"#), "got: {out}");
        assert!(out.contains(r#""notes":["note one"]"#), "got: {out}");
        // Starts with `{` and ends with `}` (no trailing newline).
        assert!(out.starts_with('{'));
        assert!(out.ends_with('}'));
    }

    #[test]
    fn minified_output_preserves_whitespace_inside_string_values() {
        // A message with internal whitespace must survive minification intact,
        // because that whitespace is part of the JSON string literal.
        let mut interner = Intern::init();
        let path_id = interner.intern_path(std::path::Path::new("/tmp/a.chrn"));
        let diag = SourceDiagnostic::new(
            DiagnosticLevel::Error,
            "two  spaces  here".to_string(),
            path_id,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        let out = render_json_diags(&[diag], &[], None, &interner, &JsonRenderConfig::new(true));
        assert!(
            out.contains(r#""message":"two  spaces  here""#),
            "internal whitespace in string was mangled: {out}"
        );
    }

    #[test]
    fn minified_empty_diags_and_footers_is_compact() {
        let interner = Intern::init();
        let out = render_json_diags(&[], &[], None, &interner, &JsonRenderConfig::new(true));
        assert_eq!(out, r#"{"diagnostics":[],"footers":[]}"#);
    }

    #[test]
    fn minify_json_preserves_escape_sequences() {
        // A string containing an escaped quote must not flip the scanner out
        // of "inside a string" state mid-escape.
        let pretty = r#"{"a":"he said \"hi\"","b":1}"#;
        let minified = minify_json(pretty);
        assert_eq!(minified, r#"{"a":"he said \"hi\"","b":1}"#);
    }
}
