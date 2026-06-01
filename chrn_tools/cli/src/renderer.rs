pub(crate) mod render_layout;
pub(crate) mod render_settings;

use std::{ops::RangeInclusive, path::Path};

use chrn_utils::{
    intern::Intern,
    source_map::{
        line_mapping::{self, Line, LineView},
        source_diagnostic::{Annotation, AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegionArena,
        source_span::SourceSpan,
    },
};
use common::color;

use crate::renderer::render_settings::RenderSettings;

// 60 dashes
const DEFAULT_SEPARATORS: &str = "------------------------------------------------------------";

//TEST: These will be moved eventually (Even though this never happened before)

// Same intention as the lower level line grouping done before with reporter
#[derive(Debug)]
pub(crate) struct RenderGroupManager<'a> {
    render_groups: Vec<RenderGroup<'a>>,
}

impl<'a> RenderGroupManager<'a> {
    pub fn new(render_groups: Vec<RenderGroup<'a>>) -> RenderGroupManager<'a> {
        RenderGroupManager { render_groups }
    }

    fn insert(&mut self, ln: &'a Line, annotation_info: &'a AnnotationInfo) {
        // Checking if the line key already exists before making a new one
        if let Some(group) = self
            .render_groups
            .iter_mut()
            .find(|group| group.ln.ln_num == ln.ln_num)
        {
            group.annotations.push(annotation_info);

            // Should do this last. Or at least after curation. So, last.
            // pair.annotations.sort_by_key(|s| s.1.span.start);
        } else {
            let group = RenderGroup::new(ln, vec![annotation_info]);
            self.render_groups.push(group);
        }
    }
}

#[derive(Debug)]
pub(crate) struct RenderGroup<'a> {
    // ln num key
    ln: &'a Line,
    annotations: Vec<&'a AnnotationInfo>,
}

impl RenderGroup<'_> {
    fn new<'a>(ln: &'a Line, annotations: Vec<&'a AnnotationInfo>) -> RenderGroup<'a> {
        RenderGroup { ln, annotations }
    }
}

// Struct so that overlapping annotations can have options attached
#[derive(Debug)]
struct AnnotationInfo {
    annotation_idx: usize,
    // How many lines it spans
    ln_num_span: RangeInclusive<u32>,
}

impl AnnotationInfo {
    pub fn new<'a>(annotation_idx: usize, ln_num_span: RangeInclusive<u32>) -> AnnotationInfo {
        AnnotationInfo {
            annotation_idx,
            ln_num_span,
        }
    }
}

// Intended to hold final line info that is sorted accordingly. So, it would ensure primary spans
// appear above secondary.
#[derive(Debug)]
struct RenderLineLayout<'a> {
    pub(crate) ln: &'a Line,
    pub(crate) render_info: Vec<RenderInfo<'a>>,
}

impl RenderLineLayout<'_> {
    pub(crate) fn new<'a>(ln: &'a Line, rows: Vec<RenderInfo<'a>>) -> RenderLineLayout<'a> {
        RenderLineLayout {
            ln,
            render_info: rows,
        }
    }
}

// Might add more so it stays as a wrapped annotation info
#[derive(Debug)]
struct RenderInfo<'a> {
    layer: u32,
    try_render_left: bool,
    annotation_info: &'a AnnotationInfo,
}

impl RenderInfo<'_> {
    pub(crate) fn new(
        layer: u32,
        try_render_left: bool,
        annotation_info: &AnnotationInfo,
    ) -> RenderInfo<'_> {
        RenderInfo {
            layer,
            try_render_left,
            annotation_info,
        }
    }
}

// (Not sure about creating a system where they know what diagnostic they want from what file)
// First we should group all diagnostics together and filter them
//
// Should maybe just return some UnfinishedResult struct internally

// The average diagnostic is not going to be that intensive, which is why the original design was
// so simple initially. This will not use anything outside of basic arrays unless somehow
// necessary.

// Possibly the worst code I've produced to date
pub(crate) fn render_cli_diags(
    diags: &[SourceDiagnostic],
    settings: &RenderSettings,
    region_arena_opt: Option<&SourceRegionArena>,
    interner: &Intern,
) -> Vec<String> {
    let region_arena = match region_arena_opt {
        Some(arena) => arena,
        None => {
            for diag in diags {}
            todo!("Hey")
        }
    };

    // Innate key value relationship since SourceSpan holds it's own region id.
    // This is how we determine how much of the source should be mapped in lines
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

    // dbg!(&required_mapping);

    let mut ln_views = Vec::new();
    let mut all_src_strs = Vec::new();

    for span in &required_mapping {
        let region = region_arena.extract_region(span.region_id);
        let ln_view = line_mapping::form_ln_view(&region.src_bytes, &span);
        ln_views.push(ln_view);
        // Lexer would break if it were invalid in the first place though right?
        let src_str = match str::from_utf8(&region.src_bytes) {
            Ok(s) => s,
            // I honestly
            Err(_) => {
                unreachable!("Earlier stages that depend on UTF-8 would've failed before this")
            }
        };

        all_src_strs.push(src_str);
    }

    // Can maybe check if annotation.is_empty() then store plain message. Or maybe do that inside
    // of form diagnostic
    let mut rendered_diags: Vec<String> = Vec::new();
    for diag in diags {
        let rendered_diag = form_diag(diag, &all_src_strs, &ln_views, settings, interner);
        rendered_diags.push(rendered_diag);
    }

    rendered_diags
}

fn form_diag_no_src() {}

// Uh
fn form_diag(
    diag: &SourceDiagnostic,
    src_strs: &[&str],
    ln_views: &[LineView],
    settings: &RenderSettings,
    interner: &Intern,
) -> String {
    // For width
    let mut highest_ln_num = 1;

    // Making `AnnotationInfo` which couples annotations with info that would be useful for the
    // layering stage
    let mut all_annotation_info: Vec<AnnotationInfo> = Vec::new();

    // Maybe the idx isn't doing what was intended
    for (annotation_idx, annotation) in diag.annotations.iter().enumerate() {
        dbg!(annotation_idx, annotation);
        let current_ln_view = ln_views
            .iter()
            .find(|lv| lv.region_id == annotation.span.region_id)
            .expect("Should already have mapped the given annotation's ln_view");

        // Finding first line containing the current annotation and making it mutable in case the
        // given annottation spans multiple lines.
        let mut current_idx = current_ln_view
            .lines
            .iter()
            .position(|ln| ln.ln_span.contains_part(annotation.span.start))
            .expect("Should already have mapped the given annotation's ln_view");

        // Mutable in case the given annotation spans multiple lines.
        let mut current_ln = &current_ln_view.lines[current_idx];
        let start = current_ln.ln_num as u32;
        let mut end = start;

        highest_ln_num = *current_ln_view.ln_num_range.end().max(&highest_ln_num);

        // Ensuring full line number spanning
        // TODO: Hold line SPANNING over line indices, which COULD be one, rather than individual
        // groups with indices over a line.
        while current_ln.ln_span.end < annotation.span.end {
            current_idx += 1;
            current_ln = &current_ln_view.lines[current_idx];
            end += 1;
        }

        let ln_num_span = start..=end;
        let annotation_info = AnnotationInfo::new(annotation_idx, ln_num_span);
        all_annotation_info.push(annotation_info);
    }

    // Creating grouped lines based off the information found in `AnnotationInfo`
    // Oh wow, another group of lines? They probably want to join our group.
    let mut group_manager = RenderGroupManager::new(Default::default());
    for annotation_info in &all_annotation_info {
        let current_annotation = &diag.annotations[annotation_info.annotation_idx];
        let current_ln_view = ln_views
            .iter()
            .find(|lv| lv.region_id == current_annotation.span.region_id)
            .expect("Should already have mapped the given annotation's ln_view");

        // Finding first line containing the current annotation and making it mutable in case the
        // given annottation spans multiple lines.
        let mut current_idx = current_ln_view
            .lines
            .iter()
            .position(|ln| ln.ln_span.contains_part(current_annotation.span.start))
            .expect("Should already have mapped the given annotation's ln_view");

        let mut current_ln = &current_ln_view.lines[current_idx];
        group_manager.insert(current_ln, annotation_info);

        while current_ln.ln_span.end < current_annotation.span.end {
            current_idx += 1;
            current_ln = &current_ln_view.lines[current_idx];
            group_manager.insert(current_ln, annotation_info);
            dbg!(current_ln);
        }
    }

    // dbg!(group_manager);

    let mut ln_layouts = create_render_line_layout(&mut group_manager);

    // dbg!(render_layout);

    let ln_num_width = line_mapping::get_num_width(highest_ln_num as usize);

    for layout in &mut ln_layouts {
        sort_render_layouts(layout, &diag.annotations);
    }

    // cut_off_render_layouts(&mut ln_layouts, &diag.annotations);

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

fn sort_render_layouts(ln_layout: &mut RenderLineLayout, annotations: &[Annotation]) {
    // Should encode, if two spans overlap, check kind. If both of same kind priority, randomly
    // pick one to increase the layer of.
    //
    // Then need sort by layout AND span
    //

    let mut has_primary = ln_layout.render_info.iter().any(|r_info| {
        let current = &annotations[r_info.annotation_info.annotation_idx];
        current.kind == AnnotationKind::Primary && current.label.is_some()
    });
    // Not using this yet
    let mut used_try_left = false;

    let mut next_layer = 1;
    for render_info in &mut ln_layout.render_info {
        dbg!(&render_info);
        let parent_annotation = &annotations[render_info.annotation_info.annotation_idx];
        // let get_prior_primary = |annotations: &[Annotation]| {
        //     annotations
        //         .iter()
        //         .rev()
        //         .find(|ann| ann.kind == AnnotationKind)
        // };

        // Simple form right now that just considers all non-primary as a different layer
        match parent_annotation.kind {
            AnnotationKind::Primary => has_primary = true,
            // if overlaps
            AnnotationKind::Secondary | AnnotationKind::Note | AnnotationKind::Help => {
                if has_primary {
                    render_info.layer = next_layer;
                    next_layer += 1;
                    has_primary = false;
                }
            }
        }
    }

    dbg!(&ln_layout);
    ln_layout.render_info.sort_by_key(|info| info.layer);
    let mut chunks: Vec<&mut [RenderInfo]> = ln_layout
        .render_info
        .chunk_by_mut(|l_info, r_info| l_info.layer == r_info.layer)
        .collect();

    // Segmenting each layer into chunks then sorting them locally based on their spanning
    // Overlaps not checked yet..
    for chunk in &mut chunks {
        chunk.sort_by_key(|r_info| {
            let parent_annotation = &annotations[r_info.annotation_info.annotation_idx];
            dbg!(parent_annotation);
            parent_annotation.span.start
        });
    }

    // let mut chunk: &[RenderInfo] = ln_layout
    //     .render_info
    //     .chunk_by_mut(|l_info, r_info| l_info.layer == r_info.layer);
}

/// through heuristic style ruling determines whether a line should be rendered or removed.
/// An example would be removing intermediate line spanning if no secondary annotations are present
/// within them, as to avoid scenarios such as hundred line rendered lines.
fn cut_off_render_layouts(ln_layouts: &mut Vec<RenderLineLayout>, annotations: &[Annotation]) {
    if ln_layouts.len() == 1 {
        return;
    }

    todo!();
}

/// Creates a line and row relationship for each member of `RenderGroupManager`
fn create_render_line_layout<'a>(
    group_manager: &'a RenderGroupManager,
) -> Vec<RenderLineLayout<'a>> {
    let mut ln_layouts: Vec<RenderLineLayout> = Vec::new();

    // Using the information obtained grom the group manager regarding line spanning to create
    // lines and rows that can be tuned according to overlaps
    for render_group in &group_manager.render_groups {
        let mut rows: Vec<RenderInfo> = Vec::new();

        // Could just do the priority checking at the time of turning to String directly by
        // attaching it's layering number, which would also avoid the vecs
        for annotation_info in &render_group.annotations {
            let render_info = RenderInfo::new(0, false, annotation_info);
            rows.push(render_info);
        }

        let ln_layout = RenderLineLayout::new(render_group.ln, rows);

        // Does nothing (Permanent)
        ln_layouts.push(ln_layout);
    }

    // Ensuring line numbers are ascending order
    ln_layouts.sort_by_key(|layout| layout.ln.ln_num);

    ln_layouts
}

fn render_text(
    diag: &SourceDiagnostic,
    ln_layouts: &[RenderLineLayout],
    src_strs: &[&str],
    ln_views: &[LineView],
    settings: &RenderSettings,
    interner: &Intern,
    ln_num_width: usize,
) -> String {
    let header = create_diag_header(diag.level, settings.can_color);
    let (bold, nc) = color::get_bold(settings.can_color);

    let path = interner.search_path(diag.path_id);
    let full_header = format!("{bold}PATH{nc} => \"{}\"\n{header}:", path.display());

    let annotations = &diag.annotations;
    let mut layout_text: Vec<String> = Vec::new();

    for (i, layout) in ln_layouts.iter().enumerate() {
        dbg!(layout.ln);
        let current_idx = ln_views
            .iter()
            .position(|lv| lv.region_id == layout.ln.ln_span.region_id)
            .expect("Infailable existence");

        // If we are not at the first layout, and the next layout isn't the last, skip it (and
        // append the special delimiting?)

        let text = render_line_layout_text(
            layout,
            src_strs[current_idx],
            annotations,
            settings,
            ln_num_width,
        );

        layout_text.push(text);
    }

    let num_alignment = " ".repeat(ln_num_width);
    let dashes = "-".repeat(3);
    let layout_text = layout_text.join(&format!("\n{num_alignment}{dashes}"));

    let mut help = String::new();
    if !diag.help.is_empty() {
        help.push('\n');
        let (orange, _) = color::get_orange(settings.can_color);

        for (i, inner_help) in diag.help.iter().enumerate() {
            help.push_str(&format!("{orange}help{nc}: "));
            help.push_str(inner_help);

            if i + 1 != diag.help.len() {
                help.push('\n');
            }
        }
    }

    let mut notes = String::new();
    if !diag.notes.is_empty() {
        notes.push('\n');
        let (cyan, _) = color::get_bold_cyan(settings.can_color);

        for (i, inner_note) in diag.notes.iter().enumerate() {
            notes.push_str(&format!("{cyan}note{nc}: "));
            notes.push_str(inner_note);

            if i + 1 != diag.notes.len() {
                notes.push('\n');
            }
        }
    }

    // println!(
    //     "{full_header} {}{layout_text}{help}{notes}\n{DEFAULT_SEPARATORS}",
    //     diag.core_msg
    // );
    //
    format!(
        "{full_header} {}{layout_text}{help}{notes}\n{DEFAULT_SEPARATORS}",
        diag.core_msg
    )
}

// Still need to curate
fn render_line_layout_text(
    ln_layout: &RenderLineLayout,
    src_str: &str,
    annotations: &[Annotation],
    settings: &RenderSettings,
    ln_num_width: usize,
) -> String {
    let mut plain_ln = String::new();

    let ln = ln_layout.ln;
    // Ok might just use exclusive, but then make inclusive. But the type safety would be lost.
    let ln_span = ln.ln_span.range_exclusive_usize();

    let mut all_ptrs: Vec<String> = Vec::new();

    let eof_byte_pos = src_str.as_bytes().len() - 1;

    let nc = color::NC;

    // if !ln_layout.rows.is_empty() {
    //     plain_ln.push('\n');
    // }

    // GENERATIONAL INDICES
    let mut current_layer = 0;
    let mut last_span_start = ln.ln_span.start as usize;

    if src_str.as_bytes()[ln_span.start] != b'\n' {
        // Is an inclusive range since spans are inclusive, inclusive in the lexer
        // dbg!(&src_str[ln_span.start..=ln_span.end]);
        plain_ln.push_str(&src_str[ln_span.start..=ln_span.end]);
    }

    // Iterating through each render_info, which should already be sorted by layer and spanning.
    for (i, render_info) in ln_layout.render_info.iter().enumerate() {
        dbg!(last_span_start);
        let annotation_parent = &annotations[render_info.annotation_info.annotation_idx];
        let span = annotation_parent.span.range_exclusive_usize();

        dbg!(render_info);
        // If the current layer is different then that means we need to add a new line for a new
        // line of pointers and recent last span start.
        let mut ptrs = String::new();
        let mut needs_new_line = false;

        if render_info.layer != current_layer {
            // If the while loop was passed then
            current_layer = render_info.layer;
            last_span_start = ln_span.start;
            needs_new_line = true;
        }

        // let annotation_header = get_annotation_kind_text(annotation_parent.kind);
        // let annotation_color = get_annotation_kind_color(annotation_parent.kind, settings);
        let ptr_str = get_annotation_kind_ptr(annotation_parent.kind);
        let ptr_color = get_annotation_kind_ptr_color(annotation_parent.kind, settings.can_color);

        // Fallback to break since eof bytes are not accounted for right now
        if last_span_start == eof_byte_pos {
            break;
        }

        dbg!(last_span_start, annotation_parent.span);
        let space_count = line_mapping::get_chars_width(
            src_str,
            last_span_start,
            annotation_parent.span.start as usize,
        );

        ptrs.push_str(&" ".repeat(space_count));

        // Space count added makes last_span_start one before the actual span. The difference of
        // span end + 1 and span start is the actual span that needs to be skipped.
        last_span_start += space_count + ((span.end + 1) - span.start);
        dbg!(last_span_start);

        // Since the function is inclusive, exclusive a + 1 is needed
        let ptr_count = line_mapping::get_chars_width(src_str, span.start, span.end + 1);
        let repeated_ptrs = ptr_str.repeat(ptr_count);

        let fmtted_ptrs = format!("{ptr_color}{repeated_ptrs}{nc}");
        ptrs.push_str(&fmtted_ptrs);

        if let Some(label) = &annotation_parent.label {
            dbg!(label);
            ptrs.push_str(&format!(" {label}"));

            if needs_new_line {
                ptrs.push('\n');
            }
        }

        all_ptrs.push(ptrs);
    }

    let current_ln_num_size = line_mapping::get_num_width(ln_layout.ln.ln_num as usize);
    let num_alignment = " ".repeat(ln_num_width - current_ln_num_size + 1);

    let fmtted_ln_num = format!("{}{num_alignment}", ln.ln_num);
    let bar_spaces = " ".repeat(ln_num_width + 1);

    let mut all_ptrs_str = String::new();
    for (i, ptr) in all_ptrs.iter().enumerate() {
        let associated_parent = &annotations[i];

        if associated_parent.label.is_some() {
            all_ptrs_str.push_str(&format!("{bar_spaces}|\t{ptr}"));
        } else {
            all_ptrs_str.push_str(&format!("{ptr}"));
        }
    }

    // need bars
    // println!("{bar_spaces}|\n{fmtted_ln_num}|\t{plain_ln}\n{all_ptrs_str}");

    // plain_ln.push(ch);
    format!("\n{bar_spaces}|\n{fmtted_ln_num}|\t{plain_ln}\n{all_ptrs_str}")
}

// let header = format!("From path => \"{}\"\n{red}error{nc}:", path.display());
// let help = help.unwrap_or_default();
//
// Probably stays the same other than the help and notes being printed as multiple if possible
// format!(
//     "{header} {base_msg}\n[{}:{}]\n{}\n{help}{note}{}",
//     line_data.ln,
//     line_data.col,
//     line_data.diag,
//     "-".repeat(TOTAL_SEPARATORS)
// )

fn create_diag_header(level: DiagnosticLevel, can_color: bool) -> String {
    let header_text = get_diag_level_text(level);

    let nc = color::NC;
    let header_color = if can_color {
        get_diag_level_color(level)
    } else {
        "".into()
    };

    format!("{header_color}{header_text}{nc}")
}

fn get_diag_level_text(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warn => "warn",
        DiagnosticLevel::Help => "help",
        DiagnosticLevel::Note => "note",
    }
}

fn get_diag_level_color(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => {
            let (red, _) = color::get_red(true);
            red
        }
        DiagnosticLevel::Warn => {
            let (orange, _) = color::get_orange(true);
            orange
        }
        DiagnosticLevel::Help => {
            let (orange, _) = color::get_orange(true);
            orange
        }
        DiagnosticLevel::Note => {
            let (cyan, _) = color::get_cyan(true);
            cyan
        }
    }
}

fn get_annotation_kind_text(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary | AnnotationKind::Secondary => "",
        AnnotationKind::Note => "note",
        AnnotationKind::Help => "help",
    }
}

fn get_annotation_kind_color(kind: AnnotationKind, can_color: bool) -> &'static str {
    match kind {
        AnnotationKind::Primary | AnnotationKind::Secondary => "",
        AnnotationKind::Note => {
            let (cyan, _) = color::get_cyan(can_color);
            cyan
        }
        AnnotationKind::Help => {
            let (orange, _) = color::get_orange(can_color);
            orange
        }
    }
}

// Nice name bud
fn get_annotation_kind_ptr_color(kind: AnnotationKind, can_color: bool) -> &'static str {
    match kind {
        AnnotationKind::Primary => {
            let (red, _) = color::get_red(can_color);
            red
        }
        AnnotationKind::Secondary | AnnotationKind::Note => {
            // This is kind of hard to see without bold
            let (cyan, _) = color::get_bold_cyan(can_color);
            cyan
        }
        AnnotationKind::Help => {
            let (orange, _) = color::get_orange(can_color);
            orange
        }
    }
}

fn get_annotation_kind_ptr(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary => "^",
        AnnotationKind::Secondary | AnnotationKind::Note | AnnotationKind::Help => "-",
    }
}

pub fn standardize_help(msg: &str, can_color: bool) -> String {
    let (orange, nc) = color::get_orange(can_color);

    if can_color {
        format!("{orange}help{nc}: {msg}\n")
    } else {
        format!("help: {msg}\n")
    }
}

pub fn standardize_note(msg: &str, can_color: bool) -> String {
    let (cyan, nc) = color::get_cyan(can_color);

    if can_color {
        format!("{cyan}help{nc}: {msg}\n")
    } else {
        format!("note: {msg}\n")
    }
}

// Probably needs to standardize, given a layout instead.
// Might not use this beyond ensuring!@#!#!
pub fn standardize_err(path: &Path, can_color: bool) -> String {
    todo!()
    // let (red, nc) = color::get_red(can_color);
    // let header = format!("From path => \"{}\"\n{red}error{nc}:", path.display());
    // let help = help.unwrap_or_default();
    //
    // Probably stays the same other than the help and notes being printed as multiple if possible
    // format!(
    //     "{header} {base_msg}\n[{}:{}]\n{}\n{help}{note}{}",
    //     line_data.ln,
    //     line_data.col,
    //     line_data.diag,
    //     "-".repeat(TOTAL_SEPARATORS)
    // )
}
