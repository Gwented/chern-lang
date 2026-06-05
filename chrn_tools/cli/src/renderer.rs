pub(super) mod layout;
pub(super) mod render_settings;
pub(super) mod style;

use chrn_utils::{
    intern::Intern,
    source_map::{
        line_mapping::{self, Line, LineView},
        source_diagnostic::{Annotation, AnnotationKind, SourceDiagnostic},
        source_region::SourceRegionArena,
        source_span::SourceSpan,
    },
};
use common::color;

use crate::renderer::render_settings::RenderSettings;

/// 60 dashes used as a visual separator between diagnostics
const DEFAULT_VISUAL_SEPARATORS: &str =
    "------------------------------------------------------------";

/// Groups annotations by the line they appear on, so spans that share a line
/// can be reasoned about together during layout.
#[derive(Debug)]
pub(crate) struct RenderGroupManager<'a> {
    render_groups: Vec<RenderGroup<'a>>,
}

impl<'a> RenderGroupManager<'a> {
    /// Creates a new group manager from an existing set of render groups.
    pub fn new(render_groups: Vec<RenderGroup<'a>>) -> RenderGroupManager<'a> {
        RenderGroupManager { render_groups }
    }

    /// Inserts an annotation into a line group if present, creating a new group if needed.
    fn insert(&mut self, ln: &'a Line, annotation: &'a Annotation) {
        if let Some(group) = self
            .render_groups
            .iter_mut()
            .find(|group| group.ln.ln_num == ln.ln_num)
        {
            group.annotations.push(annotation);
        } else {
            let group = RenderGroup::new(ln, vec![annotation]);
            self.render_groups.push(group);
        }
    }
}

/// Groups multiple annotations that belong to the same source line together
/// for layer assignment and rendering.
#[derive(Debug)]
pub(crate) struct RenderGroup<'a> {
    // line number used as key internally
    /// The source line this group corresponds to. (line number used as key internally)
    ln: &'a Line,
    /// Annotations on this line
    annotations: Vec<&'a Annotation>,
}

impl RenderGroup<'_> {
    fn new<'a>(ln: &'a Line, annotations: Vec<&'a Annotation>) -> RenderGroup<'a> {
        RenderGroup { ln, annotations }
    }
}

/// Holds the computed layout for a single source line, including layer assignments
/// that determine the vertical ordering of annotation pointers.
#[derive(Debug)]
struct RenderLineLayout<'a> {
    /// The line structure attached to this layout
    pub(crate) ln: &'a Line,
    /// Annotations associated with this line, which contain their layering details.
    /// Acts as a 1D column and row system.
    pub(crate) render_info: Vec<RenderInfo<'a>>,
}

impl RenderLineLayout<'_> {
    pub(crate) fn new<'a>(ln: &'a Line, render_info: Vec<RenderInfo<'a>>) -> RenderLineLayout<'a> {
        RenderLineLayout { ln, render_info }
    }
}

/// Associates an annotation with its assigned layer number for rendering.
#[derive(Debug)]
struct RenderInfo<'a> {
    layer: u32,
    annotation: &'a Annotation,
}

impl RenderInfo<'_> {
    pub(crate) fn new<'a>(layer: u32, annotation: &'a Annotation) -> RenderInfo<'a> {
        RenderInfo { layer, annotation }
    }
}

/// Renders a slice of source diagnostics into formatted CLI output strings.
/// When no region arena is provided, only the diagnostic header and message are emitted.
/// Otherwise, annotated source code is rendered inline.
pub(crate) fn render_cli_diags(
    diags: &[SourceDiagnostic],
    settings: &RenderSettings,
    region_arena_opt: Option<&SourceRegionArena>,
    interner: &Intern,
) -> Vec<String> {
    let region_arena = match region_arena_opt {
        Some(arena) => arena,
        None => {
            let mut rendered_diags: Vec<String> = Vec::new();
            for diag in diags {
                let path = interner.search_path(diag.path_id);
                let header = style::create_diag_header(
                    diag.level,
                    path,
                    settings.can_color,
                    settings.terminal_type,
                );
                rendered_diags.push(format!("{header} {}", diag.core_msg));
            }

            return rendered_diags;
        }
    };

    // Merge overlapping spans per region. SourceSpan stores its own region_id, so we
    // can merge annotations that refer to the same source region into a single span
    // that covers all of them. This determines how much source text we need to map.
    let mut required_mapping: Vec<SourceSpan> = Vec::with_capacity(region_arena.regions.len());

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
        let region = region_arena.extract_region(span.region_id);
        let ln_view = line_mapping::form_ln_view(&region.src_bytes, &span);
        ln_views.push(ln_view);
        let src_str = match str::from_utf8(&region.src_bytes) {
            Ok(s) => s,
            Err(_) => unreachable!("Should already have UTF-8 validity at this stage"),
        };

        all_src_strs.push(src_str);
    }

    // Final step of rendering and returning the text
    let mut rendered_diags: Vec<String> = Vec::new();
    for diag in diags {
        let rendered_diag = form_diag(diag, &all_src_strs, &ln_views, settings, interner);
        rendered_diags.push(rendered_diag);
    }

    // Might just return a new line joined string of a single diagnostic
    rendered_diags
}

/// Given an annotation, finds all source lines it touches and the highest line number
/// in that view for number width alignment
fn find_annotation_lines<'a>(
    annotation: &Annotation,
    ln_views: &'a [LineView],
) -> (Vec<&'a Line>, u32) {
    let current_ln_view = ln_views
        .iter()
        .find(|lv| lv.region_id == annotation.span.region_id)
        .expect("Should already have mapped the given annotation's ln_view");

    let mut current_idx = current_ln_view
        .lines
        .iter()
        .position(|ln| ln.ln_span.contains_part(annotation.span.start))
        .expect("Should already have mapped the given annotation's ln_view");

    let mut current_ln = &current_ln_view.lines[current_idx];
    let max_ln_num = *current_ln_view.ln_num_range.end();

    let mut spanned_lines = vec![current_ln];
    while current_ln.ln_span.end < annotation.span.end {
        current_idx += 1;
        current_ln = &current_ln_view.lines[current_idx];
        spanned_lines.push(current_ln);
    }

    (spanned_lines, max_ln_num)
}

/// Build a rendered diagnostic string from a single `SourceDiagnostic`.
/// This function: Collects annotation info, groups by line, assigns layers to resolve overlap,
/// then renders result to text.
fn form_diag(
    diag: &SourceDiagnostic,
    src_strs: &[&str],
    ln_views: &[LineView],
    settings: &RenderSettings,
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
        let (spanned_lines, max_ln_num) = find_annotation_lines(annotation, ln_views);
        highest_ln_num = highest_ln_num.max(max_ln_num);
        annotation_and_lines.push((annotation, spanned_lines));
    }

    // Using render groups to directly pair an annotation with it's associated line
    let mut group_manager = RenderGroupManager::new(Vec::new());
    for (annotation, spanned_lines) in &annotation_and_lines {
        for ln in spanned_lines {
            group_manager.insert(ln, annotation);
        }
    }

    let mut ln_layouts = create_render_line_layout(&group_manager);
    let ln_num_width = line_mapping::get_num_width(highest_ln_num as usize);

    for layout in &mut ln_layouts {
        //FIX: EOF is not quite accounted for by line mapping due to how it works
        let current_idx = ln_views
            .iter()
            .position(|lv| lv.region_id == layout.ln.ln_span.region_id)
            .expect("Should already have mapped the given annotation's ln_view");

        assign_layers_in_layout(layout, src_strs[current_idx]);
    }

    // Remove layouts that ended up with no annotations after layer assignment
    // (intermediate lines of multi-line spans)
    ln_layouts.retain(|layout| !layout.render_info.is_empty());

    render_text(
        diag,
        &ln_layouts,
        src_strs,
        ln_views,
        settings,
        interner,
        ln_num_width,
    )
}

/// Assigns layers to each `RenderInfo` so overlapping annotations don't collide on the
/// same printed row.
fn assign_layers_in_layout(ln_layout: &mut RenderLineLayout, src_str: &str) {
    // Remove annotations from intermediate lines of multi-line spans. An annotation
    // only draws its pointer on the line where it starts and the line where it ends;
    // lines in between show the source but no pointer repetition.
    let ln_span = ln_layout.ln.ln_span;
    ln_layout.render_info.retain(|render_info| {
        let ann = render_info.annotation;
        ln_span.contains_part(ann.span.start) || ln_span.contains_part(ann.span.end)
    });

    // Track occupied visual spacing for layers
    let mut layer_occupied: Vec<usize> = Vec::new();

    // Sort primaries first so they reserve layer 0 before overlapping secondaries
    // are placed. Within the same kind, sort by span start.
    ln_layout.render_info.sort_by_key(|r_info| {
        (
            r_info.annotation.kind != AnnotationKind::Primary,
            r_info.annotation.span.start,
        )
    });

    for render_info in &mut ln_layout.render_info {
        let annotation = render_info.annotation;

        match annotation.kind {
            // Place primaries on the base layer 0 and extend its occupied width
            // so overlapping secondaries are forced to a higher layer.
            AnnotationKind::Primary => {
                if layer_occupied.is_empty() {
                    layer_occupied.push(0);
                }

                let span = annotation.span.range_exclusive_usize();
                let ln_start = ln_layout.ln.ln_span.start as usize;
                let ln_end = ln_layout.ln.ln_span.end as usize;
                let clamped_start = span.start.max(ln_start);
                let clamped_end = span.end.min(ln_end);
                let start = line_mapping::get_chars_width(src_str, ln_start, clamped_start);
                let len = line_mapping::get_chars_width(src_str, clamped_start, clamped_end + 1);
                layer_occupied[0] = layer_occupied[0].max(start + len);
            }
            AnnotationKind::Secondary | AnnotationKind::Note | AnnotationKind::Help => {
                let span = annotation.span.range_exclusive_usize();
                let ln_start = ln_layout.ln.ln_span.start as usize;
                let ln_end = ln_layout.ln.ln_span.end as usize;
                let clamped_start = span.start.max(ln_start);
                let clamped_end = span.end.min(ln_end);
                let start = line_mapping::get_chars_width(src_str, ln_start, clamped_start);
                let len = line_mapping::get_chars_width(src_str, clamped_start, clamped_end + 1);
                let end = start + len;

                if layer_occupied.is_empty() {
                    layer_occupied.push(end);
                } else {
                    let mut placed = false;
                    for (layer_idx, occ_end) in layer_occupied.iter_mut().enumerate().skip(1) {
                        if start >= *occ_end {
                            render_info.layer = layer_idx as u32;
                            *occ_end = end;
                            placed = true;
                            break;
                        }
                    }

                    if !placed {
                        render_info.layer = layer_occupied.len() as u32;
                        layer_occupied.push(end);
                    }
                }
            }
        }
    }

    // Sorting by layer and by span start for correct positioning
    ln_layout
        .render_info
        .sort_by_key(|info| (info.layer, info.annotation.span.start));
}

/// Converts grouped annotations into a sorted list of line layouts, one per source line
/// with annotations
fn create_render_line_layout<'a>(
    group_manager: &'a RenderGroupManager,
) -> Vec<RenderLineLayout<'a>> {
    let mut ln_layouts = Vec::new();

    for render_group in &group_manager.render_groups {
        let mut rows = Vec::new();

        for annotation in &render_group.annotations {
            rows.push(RenderInfo::new(0, annotation));
        }

        ln_layouts.push(RenderLineLayout::new(render_group.ln, rows));
    }

    // Sorting line numbers by ascending order
    ln_layouts.sort_by_key(|layout| layout.ln.ln_num);
    ln_layouts
}

/// Assembles the full diagnostic string by combining the header, all rendered line layouts,
/// help messages, notes, and the trailing separator.
fn render_text(
    diag: &SourceDiagnostic,
    ln_layouts: &[RenderLineLayout],
    src_strs: &[&str],
    ln_views: &[LineView],
    settings: &RenderSettings,
    interner: &Intern,
    ln_num_width: usize,
) -> String {
    let path = interner.search_path(diag.path_id);
    let header =
        style::create_diag_header(diag.level, path, settings.can_color, settings.terminal_type);

    // Spaces prefixing the `---` gap separator (line-number column width). The bar lines use
    // one additional space so the `|` visually sits just after the line-number column.
    let num_alignment = " ".repeat(ln_num_width);
    let bar_spaces = " ".repeat(ln_num_width + 1);

    let mut layout_text = String::new();
    for (i, layout) in ln_layouts.iter().enumerate() {
        // Giving visual dashes if the distance between the previous and current line is > 1
        if i > 0 {
            let prev_ln = ln_layouts[i - 1].ln.ln_num;
            if layout.ln.ln_num - prev_ln > 1 {
                layout_text.push_str(&format!("\n{num_alignment}---"));
                layout_text.push_str(&format!("\n{bar_spaces}|"));
            }
        } else {
            layout_text.push_str(&format!("\n{bar_spaces}|"));
        }

        let current_idx = ln_views
            .iter()
            .position(|lv| lv.region_id == layout.ln.ln_span.region_id)
            .expect("Infailable existence");

        layout_text.push_str(&render_line_layout_text(
            layout,
            src_strs[current_idx],
            settings,
            ln_num_width,
        ));
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

    format!(
        "{header} {}{layout_text}{help}{notes}\n{DEFAULT_VISUAL_SEPARATORS}",
        diag.core_msg
    )
}

//FIX: Eof byte handling might DESTROY this. Also cross-module.
/// Renders the annotated source line and all its pointer rows according to the layer
/// assignments from [`assign_layers_in_layout`].
fn render_line_layout_text(
    ln_layout: &RenderLineLayout,
    src_str: &str,
    settings: &RenderSettings,
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
    let ln_end = if src_str.as_bytes()[ln_span.end] == b'\n' {
        ln_span.end - 1
    } else {
        ln_span.end
    };

    // The line mapping functions used within `chrn_core` ONLY keeps a new line if the line is a
    // single empty line, so this just skips any empty lines.
    if src_str.as_bytes()[ln_span.start] != b'\n' {
        plain_ln.push_str(&src_str[ln_span.start..=ln_end]);

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
            let visual_len = line_mapping::get_chars_width(src_str, clamped_start, clamped_end + 1);
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

    format!("\n{fmtted_ln_num}| {plain_ln}\n{all_ptrs_str}")
}
