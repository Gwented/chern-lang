use std::ops::{Range, RangeInclusive};

use crate::id_types::{ModuleId, SourceRegionId};

// pub struct SpanArena {
//     spans: Vec<SourceSpan>,
// }
//
// impl SpanArena {
//     pub fn new(spans: Vec<SourceSpan>) -> SpanArena {
//         SpanArena { spans }
//     }
// }

// Could this be u32?
/// Span structure used for source mapping
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
// Should this type enforce inclusive by using RangeInclusive<u32>?
pub struct SourceSpan {
    pub region_id: SourceRegionId,
    pub start: u32,
    pub end: u32,
}

impl SourceSpan {
    pub fn new(region_id: SourceRegionId, start: u32, end: u32) -> SourceSpan {
        SourceSpan {
            region_id,
            start,
            end,
        }
    }

    // Maybe?
    // pub fn curate(&self, other: SourceSpan) -> SourceSpan {}

    /// Creates an (inclusive, exclusive) ranged span
    pub fn range_exclusive_usize(&self) -> Range<usize> {
        (self.start as usize)..(self.end as usize)
    }

    /// Creates an (inclusive, inclusive) ranged span
    pub fn range_inclusive_usize(&self) -> RangeInclusive<usize> {
        (self.start as usize)..=(self.end as usize)
    }

    /// Creates span that contains the min start and max end of two spans
    pub fn merge(&self, other: SourceSpan) -> SourceSpan {
        // Not sure about this entirely, but if this is used wrongly then it should probably be a
        // hard error since it means something internally went wrong, which would be good to know
        //
        // But should this really be everywhere?
        // if self.path_id == other.path_id {
        //     panic!("`self` path_id == `other` path_id in call to `SourceSpan::merge`");
        // }

        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        SourceSpan::new(other.region_id, start, end)
    }

    pub fn contains(&self, other: SourceSpan) -> bool {
        self.start <= other.start && self.end >= other.end
    }
}

// Maybe option maybe not
/// Takes in an array of spans and merges all of them together. Expects that there is at least 1 span
/// present.
//TODO: Should just return option span
pub fn merge_spans(spans: &[SourceSpan]) -> Option<SourceSpan> {
    let mut full_span = *spans.get(0)?;

    for span in spans.iter().skip(1).copied() {
        full_span = full_span.merge(span);
    }

    Some(full_span)
}
