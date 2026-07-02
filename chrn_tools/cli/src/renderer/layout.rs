use chrn_utils::{
    id_types::SourceRegionId,
    source_map::{
        line_mapping::{self, Line, LineView},
        source_diagnostic::{
            SourceDiagnostic,
            annotations::{Annotation, AnnotationKind},
        },
    },
};

/// Groups annotations by the line they appear on, so spans that share a line
/// can be reasoned about together during layout.
#[derive(Debug)]
pub(crate) struct RenderGroupManager<'a> {
    render_groups: Vec<RenderGroup<'a>>,
}

impl<'a> RenderGroupManager<'a> {
    pub(crate) fn new(render_groups: Vec<RenderGroup<'a>>) -> RenderGroupManager<'a> {
        RenderGroupManager { render_groups }
    }

    pub(crate) fn insert(&mut self, ln: &'a Line, annotation: &'a Annotation) {
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
    ln: &'a Line,
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
pub(super) struct RenderLineLayout<'a> {
    pub(crate) ln: &'a Line,
    pub(crate) render_info: Vec<RenderInfo<'a>>,
}

impl RenderLineLayout<'_> {
    pub(crate) fn new<'a>(ln: &'a Line, render_info: Vec<RenderInfo<'a>>) -> RenderLineLayout<'a> {
        RenderLineLayout { ln, render_info }
    }
}

/// Associates an annotation with its assigned layer number for rendering.
#[derive(Debug)]
pub(super) struct RenderInfo<'a> {
    pub(crate) layer: u32,
    pub(crate) annotation: &'a Annotation,
}

impl RenderInfo<'_> {
    pub(crate) fn new<'a>(layer: u32, annotation: &'a Annotation) -> RenderInfo<'a> {
        RenderInfo { layer, annotation }
    }
}

/// Given an annotation, finds all source lines it touches and the highest line number
/// in that view for number width alignment
pub(super) fn find_annotation_lines<'a>(
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

/// Assigns layers to each `RenderInfo` so overlapping annotations don't collide on the
/// same printed row.
pub(super) fn assign_layers_in_layout(ln_layout: &mut RenderLineLayout, src_str: &str) {
    let ln_span = ln_layout.ln.ln_span;
    ln_layout.render_info.retain(|render_info| {
        let ann = render_info.annotation;
        ln_span.contains_part(ann.span.start) || ln_span.contains_part(ann.span.end)
    });

    let mut layer_occupied: Vec<usize> = Vec::new();

    ln_layout.render_info.sort_by_key(|r_info| {
        (
            r_info.annotation.kind != AnnotationKind::Primary,
            r_info.annotation.span.start,
        )
    });

    for render_info in &mut ln_layout.render_info {
        let annotation = render_info.annotation;

        match annotation.kind {
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

                render_info.layer = 0;
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

    ln_layout
        .render_info
        .sort_by_key(|info| (info.layer, info.annotation.span.start));
}

/// Converts grouped annotations into a sorted list of line layouts, one per source line
/// with annotations
pub(super) fn create_render_line_layout<'a>(
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

    ln_layouts.sort_by_key(|lay| lay.ln.ln_num);
    ln_layouts
}

/// Greedily sorts layouts so that any region containing an `AnnotationKind::Primary`
/// is rendered before regions that don't. For example, if we have 3 regions where 2 have primaries
/// and one only has a secondary annotation, the 2 primary containing regions along with any other
/// annotation associated with them on the same line layout will be prioritized depending on
/// whichever comes first, and the region with only a secondary will be rendered last.
/// and within a region layouts are ordered by line number.
pub(super) fn sort_layouts_by_region_priority<'a>(ln_layouts: &mut [RenderLineLayout<'a>]) {
    // Leaves early to avoid allocations if no elements are present
    if ln_layouts.is_empty() {
        return;
    }

    // Tracks the first-seen encounter order of each unique region encountered while
    // walking the layouts. The position of a region in this Vec is its priority index where
    // smaller = encountered earlier in the input.
    let mut region_order: Vec<SourceRegionId> = Vec::new();
    // Vec which greedily stores any `RegionId` that has a primary annotation
    let mut region_has_primary: Vec<SourceRegionId> = Vec::new();

    // Single pass over the layouts to record region order and which regions are primary.
    for layout in ln_layouts.iter() {
        let region_id = layout.ln.ln_span.region_id;
        // First time we see this region, record its position in the encounter order.
        if !region_order.contains(&region_id) {
            region_order.push(region_id);
        }
        // Check if this layout's annotations contain a primary, if so, store the `RegionId` if not
        // already present
        let contains_primary = layout
            .render_info
            .iter()
            .any(|info| info.annotation.kind == AnnotationKind::Primary);
        if contains_primary && !region_has_primary.contains(&region_id) {
            region_has_primary.push(region_id);
        }
    }

    //  Sorts by primary regions first.
    //  Sorts by region_idx next to preserve the original encounter order which separates
    //  regions that have a primary
    //  Sorts by line number to ensure order is still retained on a region basis in line number
    ln_layouts.sort_by_key(|layout| {
        let region_id = layout.ln.ln_span.region_id;
        // Look up this region's priority index from the encounter order; should always
        // be found since we built `region_order` from the same layouts.
        let region_idx = region_order
            .iter()
            .position(|id| *id == region_id)
            .unwrap_or(0);
        let is_primary = region_has_primary.contains(&region_id);
        (!is_primary, region_idx, layout.ln.ln_num)
    });
}
