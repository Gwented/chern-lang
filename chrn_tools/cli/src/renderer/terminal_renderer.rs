pub(super) mod layout;
pub(crate) mod render_settings;
pub(super) mod style;

use chrn_utils::{
    arena::Arena,
    id_types::SourceRegionId,
    intern::Intern,
    source_map::{
        line_mapping::{self, Line, LineView},
        source_diagnostic::{SourceDiagnostic, annotations::Annotation, footers::FooterKind},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};
use common::color;

use crate::{
    renderer::terminal_renderer::{
        layout::{RenderInfo, RenderLineLayout},
        render_settings::TerminalRenderConfig,
    },
    s_ifier,
};

/// 60 dashes used as a visual separator between diagnostics
const DEFAULT_VISUAL_SEPARATORS: &str =
    "------------------------------------------------------------";

/// Renders a slice of source diagnostics into this renderer's heuristic styling, output as strings.
/// When no region arena is provided, only the diagnostic header and message are emitted.
// Why was "inline" relevant here?
pub(crate) fn render_terminal_diags(
    diags: &[SourceDiagnostic],
    footers: &[FooterKind],
    settings: &TerminalRenderConfig,
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
) -> Vec<String> {
    let region_arena = match region_arena_opt {
        Some(arena) => arena,
        None => {
            let mut rendered_diags: Vec<String> = Vec::new();
            for diag in diags {
                let path = interner.search_path(diag.path_id);
                let path_header = style::create_path_header(path, settings);
                // Not really a header when it's using the message too
                let level_header = style::create_level_header(diag.level, &diag.core_msg, settings);

                let header = format!("{path_header}\n{level_header}");
                rendered_diags.push(header);
            }

            return rendered_diags;
        }
    };

    // Merge overlapping spans per region. SourceSpan stores its own region_id, so we
    // can merge annotations that refer to the same source region into a single span
    // that covers all of them. This determines how much source text we need to map.
    let mut required_mapping: Vec<SourceSpan> = Vec::with_capacity(region_arena.len());

    for diag in diags {
        for annotation in &diag.annotations {
            let span_opt = required_mapping
                .iter()
                .position(|other| annotation.span.region_id == other.region_id);

            if let Some(span_idx) = span_opt {
                let other = required_mapping[span_idx];
                required_mapping[span_idx] = annotation.span.merge(&other);
            } else {
                required_mapping.push(annotation.span);
            }
        }
    }

    let mut ln_views = Vec::new();
    let mut all_src_strs = Vec::new();

    for span in &required_mapping {
        let region = &region_arena[span.region_id];
        let ln_view = line_mapping::form_ln_view(&region.src_bytes, &span);
        ln_views.push(ln_view);
        let src_str = match str::from_utf8(&region.src_bytes) {
            Ok(s) => s,
            Err(_) => unreachable!("Should already have UTF-8 validity at this stage from lexer"),
        };

        all_src_strs.push(src_str);
    }

    // Final step of rendering and returning the text
    let mut rendered_diags: Vec<String> = Vec::with_capacity(diags.len());
    for diag in diags {
        let rendered_diag = form_diag(
            diag,
            &all_src_strs,
            &ln_views,
            settings,
            region_arena,
            interner,
        );
        rendered_diags.push(rendered_diag);
    }

    for footer in footers {
        rendered_diags.push(render_footer(footer, settings));
    }

    // Might just return a new line joined string of a single diagnostic
    rendered_diags
}

/// Build a rendered diagnostic string from a single `SourceDiagnostic`.
/// This function: Collects annotation info, groups by line, assigns layers to resolve overlap,
/// then renders result to text.
fn form_diag(
    diag: &SourceDiagnostic,
    src_strs: &[&str],
    ln_views: &[LineView],
    settings: &TerminalRenderConfig,
    region_arena: &Arena<SourceRegion, SourceRegionId>,
    interner: &Intern,
) -> String {
    // For tracking max line width needed for rendering
    let mut highest_ln_num: u32 = 1;
    // Tuple of an annotation and the lines associated with it as KV relationship
    // This is because an annotation can span multiple lines, and may be used to account for
    // possibly removing intermediate lines.
    let mut annotation_and_lines: Vec<(&Annotation, Vec<&Line>)> = Vec::new();

    // Pairs annotation with it's lines then stores it
    for annotation in &diag.annotations {
        let (spanned_lines, max_ln_num) = layout::find_annotation_lines(annotation, ln_views);
        highest_ln_num = highest_ln_num.max(max_ln_num);
        annotation_and_lines.push((annotation, spanned_lines));
    }

    // Using render groups to directly pair an annotation with it's associated line
    let mut group_manager = layout::RenderGroupManager::new(Vec::new());
    for (annotation, spanned_lines) in &annotation_and_lines {
        for ln in spanned_lines {
            group_manager.insert(ln, annotation);
        }
    }

    let mut ln_layouts = layout::create_render_line_layout(&group_manager);
    let ln_num_width = line_mapping::get_num_width(highest_ln_num as usize);

    for layout in &mut ln_layouts {
        let current_idx = ln_views
            .iter()
            .position(|lv| lv.region_id == layout.ln.ln_span.region_id)
            .expect("Should already have mapped the given annotation's ln_view");

        layout::assign_layers_in_layout(layout, src_strs[current_idx]);
    }

    // Remove layouts that ended up with no annotations after layer assignment
    // (intermediate lines of multi-line spans)
    ln_layouts.retain(|lay| !lay.render_info.is_empty());
    layout::sort_layouts_by_region_priority(&mut ln_layouts);

    render_text(
        diag,
        &ln_layouts,
        src_strs,
        ln_views,
        settings,
        ln_num_width,
        region_arena,
        interner,
    )
}

/// Assembles the full diagnostic string by combining the header, all rendered line layouts,
/// help messages, notes, and the trailing separator.
fn render_text(
    diag: &SourceDiagnostic,
    ln_layouts: &[RenderLineLayout],
    src_strs: &[&str],
    ln_views: &[LineView],
    settings: &TerminalRenderConfig,
    ln_num_width: usize,
    region_arena: &Arena<SourceRegion, SourceRegionId>,
    interner: &Intern,
) -> String {
    // Spaces prefixing the `---` gap separator (line-number column width). The bar lines use
    // one additional space so the `|` visually sits just after the line-number column.
    let num_alignment = " ".repeat(ln_num_width);
    // Spacing intented to align right where the bars would be for the given line context
    let bar_spaces = " ".repeat(ln_num_width + 1);

    let mut layout_text = String::new();

    // Is Option since there could be something going through render_text that does not actually have
    // any line layouts and only has a header and basic error message
    let mut prev_region_id_opt: Option<SourceRegionId> =
        ln_layouts.first().map(|layout| layout.ln.ln_span.region_id);
    let mut placed_path = false;

    for (i, layout) in ln_layouts.iter().enumerate() {
        // Searching by key with the region id into ln_views, which corresponds with it's src_str
        let current_ln_view_idx = ln_views
            .iter()
            .position(|lv| lv.region_id == layout.ln.ln_span.region_id)
            .expect("Infallable existence");
        let current_region_id = layout.ln.ln_span.region_id;

        // Checking if the region is different so files from different annotations are visually
        // distinct and labeled.
        if let Some(prev_id) = prev_region_id_opt
            && prev_id != current_region_id
        {
            placed_path = false;
        }

        if !placed_path {
            let new_region = &region_arena[current_region_id];
            let path = interner.search_path(new_region.path_id);
            let path_header_sep = style::create_path_header(path, settings);

            // Since this boolean controls the first path placed and any intermediate paths placed,
            // this condition is so that it doesn't push dashes for the first
            if i > 0 {
                layout_text.push_str(&format!("\n{num_alignment}---"));
            }

            layout_text.push_str(&format!("\n{path_header_sep}"));
            layout_text.push_str(&format!("\n{bar_spaces}|"));

            prev_region_id_opt = Some(current_region_id);
            placed_path = true;
        } else if i > 0 {
            // Giving visual dashes if the distance between the previous and current line is > 1
            let prev_ln = ln_layouts[i - 1].ln.ln_num;
            if prev_ln + 1 != layout.ln.ln_num {
                layout_text.push_str(&format!("\n{num_alignment}---"));
            }
        } else {
            layout_text.push_str(&format!("\n{bar_spaces}|"));
        }

        layout_text.push_str(&render_line_layout_text(
            layout,
            src_strs[current_ln_view_idx],
            settings,
            ln_num_width,
        ));
    }

    // Meaning there were no line layouts which skips the loop, but this still needs it's pat
    // shown so this is done
    if prev_region_id_opt.is_none() {
        let path = interner.search_path(diag.path_id);
        let path_header_sep = style::create_path_header(path, settings);
        layout_text.push_str(&format!("\n{path_header_sep}"));
    }

    let mut help = String::new();
    if !diag.help.is_empty() {
        help.push('\n');
        for (i, inner_help) in diag.help.iter().enumerate() {
            let fmtted_help =
                style::standardize_help(inner_help, settings.can_color, settings.terminal_type);
            help.push_str(&fmtted_help);

            if i + 1 != diag.help.len() {
                help.push('\n');
            }
        }
    }

    let mut notes = String::new();
    if !diag.notes.is_empty() {
        notes.push('\n');
        for (i, inner_note) in diag.notes.iter().enumerate() {
            let fmtted_note =
                style::standardize_note(inner_note, settings.can_color, settings.terminal_type);
            notes.push_str(&fmtted_note);

            if i + 1 != diag.notes.len() {
                notes.push('\n');
            }
        }
    }

    let level_header = style::create_level_header(diag.level, &diag.core_msg, settings);

    format!("{level_header} {layout_text}{help}{notes}\n{DEFAULT_VISUAL_SEPARATORS}")
}

//FIX: Eof byte handling might DESTROY this. Also cross-module.
/// Renders the annotated source line and all its pointer rows according to the layer
/// assignments from [`assign_layers_in_layout`].
fn render_line_layout_text(
    ln_layout: &RenderLineLayout,
    src_str: &str,
    settings: &TerminalRenderConfig,
    ln_num_width: usize,
) -> String {
    let ln = ln_layout.ln;
    let ln_span = ln.ln_span.range_exclusive_usize();
    let mut all_ptr_rows: Vec<String> = Vec::new();

    let nc = color::get_nc(settings.can_color);

    let mut plain_ln = String::new();

    // -- FIRST --
    // If the current line is the last line then it may or may not contain a new line as it's eof
    // byte, which needs to be removed if present
    //WARN: SPANNING IS (INCLUSIVE, EXCLUSIVE) SO THIS NEEDS - 1 TO NOT GO OUT OF BOUNDS
    let ln_end = if src_str.as_bytes()[ln_span.end - 1] == b'\n' {
        ln_span.end - 1
    } else {
        ln_span.end
    };

    // The line mapping functions used within `chrn_core` ONLY keeps a new line if the line is a
    // single empty line, so this just skips any empty lines.
    if src_str.as_bytes()[ln_span.start] != b'\n' {
        plain_ln.push_str(&src_str[ln_span.start..ln_end]);

        // Not sure if this eof specific character is really needed. It's already pretty obvious
        // looking.
        // if ln_end != ln_span.end {
        //     let (bold, _) = color::get_bold(settings.can_color);
        //     let (grey, _) = color::get_grey(settings.can_color, settings.terminal_type);
        //     plain_ln.push_str(&format!("{bold}{grey}<eof>{nc}"));
        // }
    }

    // -- SECOND --
    // Partition render info by layer into a Vec indexed by layer number.
    let mut layer_vec: Vec<Vec<&RenderInfo>> = Vec::new();
    for render_info in &ln_layout.render_info {
        let idx = render_info.layer as usize;
        if idx >= layer_vec.len() {
            layer_vec.resize(idx + 1, Vec::new());
        }

        layer_vec[idx].push(render_info);
    }

    // -- THIRD --
    // For each layer, annotations are placed on the earlier row visually possible without
    // overlapping with an already placed annotation.

    // Re-used vectors for every row that are cleared
    //
    // row_ends[i] tracks the rightmost visual column that has been placed on row i.
    // A new pointer can share row i if its visual_start >= row_ends[i]
    let mut row_ends: Vec<usize> = Vec::new();
    let mut row_strs: Vec<String> = Vec::new();

    for infos in &layer_vec {
        for render_info in infos {
            let annotation = render_info.annotation;
            let span = annotation.span.range_exclusive_usize();

            let ptr_str = style::get_annotation_kind_ptr(annotation.kind);
            let ptr_color = style::get_annotation_kind_ptr_color(
                annotation.kind,
                settings.can_color,
                settings.terminal_type,
            );

            // Process the visual column range of this pointer, accounting for unicode width
            let clamped_start = span.start.max(ln_span.start);
            let clamped_end = span.end.min(ln_end);
            let visual_start = line_mapping::get_chars_width(src_str, ln_span.start, clamped_start);
            //WARN: CHANGED
            // let visual_len = line_mapping::get_chars_width(src_str, clamped_start, clamped_end + 1);
            let visual_len = line_mapping::get_chars_width(src_str, clamped_start, clamped_end);
            let visual_end = visual_start + visual_len;

            let fmtted_ptrs = format!("{ptr_color}{}", ptr_str.repeat(visual_len));

            // Try first-fit by placing it on the earliest existing row where this pointer
            // does not visually overlap with already-placed spanning
            let mut placed = false;
            for (row_idx, end) in row_ends.iter_mut().enumerate() {
                if visual_start >= *end {
                    let spaces = visual_start.saturating_sub(*end);
                    row_strs[row_idx].push_str(&" ".repeat(spaces));
                    row_strs[row_idx].push_str(&fmtted_ptrs);
                    if let Some(label) = &annotation.label {
                        row_strs[row_idx].push_str(&format!(" {label}"));
                    }
                    *end = visual_end;
                    placed = true;
                    break;
                }
            }

            if !placed {
                // No existing row has room for this pointer so it starts a new row
                let mut single = String::new();
                single.push_str(&" ".repeat(visual_start));
                single.push_str(&fmtted_ptrs);
                if let Some(label) = &annotation.label {
                    single.push_str(&format!(" {label}"));
                }
                row_strs.push(single);
                row_ends.push(visual_end);
            }
        }

        // Allowing re-usage of both vectors
        row_ends.clear();

        for row in row_strs.drain(..) {
            all_ptr_rows.push(row);
        }
    }

    // -- FOURTH --
    // Padding using the max line number width as well as the current line number so that the
    // vertical bars are aligned even with line numbers
    let current_ln_num_size = line_mapping::get_num_width(ln_layout.ln.ln_num as usize);
    let num_alignment = " ".repeat(ln_num_width - current_ln_num_size + 1);

    let fmtted_ln_num = format!("{}{num_alignment}", ln.ln_num);
    let bar_spaces = " ".repeat(ln_num_width + 1);

    // Joining all pointers which requires consistent bar allignment for all annotations to be
    // aligned
    let mut all_ptrs_str = String::new();
    for (i, row) in all_ptr_rows.iter().enumerate() {
        all_ptrs_str.push_str(&format!("{bar_spaces}| {row}{nc}"));
        if i + 1 < all_ptr_rows.len() {
            all_ptrs_str.push('\n');
        }
    }

    // I don't like how this space is here but can't remember how this even became a requirement.
    // After moving to pointers and having several bugs related wrong source mapping this just stuck
    // afterwards, which seems like a bad thing.
    format!("\n{fmtted_ln_num}| {plain_ln}\n{all_ptrs_str}")
}

/// Renders given footer into a string
fn render_footer(footer: &FooterKind, settings: &TerminalRenderConfig) -> String {
    let footer = match footer {
        FooterKind::DiagnosticsExceeded(amt_exceeded) => {
            let s_suffix = s_ifier!(*amt_exceeded);
            format!("{amt_exceeded} diagnostic{s_suffix} suppressed")
        }
    };

    style::standardize_warn(&footer, settings.can_color, settings.terminal_type)
}
