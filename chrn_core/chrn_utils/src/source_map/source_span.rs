// Should maybe not be specifically inside source_map
use std::ops::{Range, RangeInclusive};

use crate::id_types::SourceRegionId;

// pub struct SpanArena {
//     pub spans: Vec<SourceSpan>,
// }
//
// impl SpanArena {
//     pub fn new(spans: Vec<SourceSpan>) -> SpanArena {
//         SpanArena { spans }
//     }
//
//     pub fn push_span(&mut self, span: SourceSpan) -> SpanId {
//         let span_id = SpanId::new(self.spans.len() as u32);
//         self.spans.push(span);
//         span_id
//     }
//
//     /// Returns owned spane
//     pub fn get_span(&self, span_id: SpanId) -> SourceSpan {
//         self.spans[span_id.id as usize]
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

    /// Creates an (inclusive, exclusive) ranged span
    pub fn range_exclusive_u32(&self) -> Range<u32> {
        (self.start)..(self.end)
    }

    /// Creates an (inclusive, inclusive) ranged span
    pub fn range_inclusive_u32(&self) -> RangeInclusive<u32> {
        (self.start)..=(self.end)
    }

    /// Creates span that contains the min start and max end of two spans
    pub fn merge(&self, other: &SourceSpan) -> SourceSpan {
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

    /// Checks if `self` is either a superset or equal to `other`
    pub fn contains(&self, other: SourceSpan) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    /// Checks if `self` contains `other`
    pub fn contains_part(&self, other: u32) -> bool {
        self.start <= other && self.end >= other
    }
}

// Uh
// impl Ord for SourceSpan {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         if self.start < other.start && self.end > other.end {
//             std::cmp::Ordering::Greater
//         } else if self.start > other.start && self.end < other.end {
//             std::cmp::Ordering::Less
//         } else {
//             std::cmp::Ordering::Equal
//         }
//     }
// }
//
// impl PartialOrd for SourceSpan {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         match self.start.partial_cmp(&other.start) {
//             Some(core::cmp::Ordering::Equal) => {}
//             ord => return ord,
//         }
//         self.end.partial_cmp(&other.end)
//     }
// }

// Maybe option maybe not
/// Takes in an array of spans and merges all of them together. Expects that there is at least 1 span
/// present.
//TODO: Should just return option span
pub fn merge_spans(spans: &[SourceSpan]) -> Option<SourceSpan> {
    let mut full_span = *spans.get(0)?;

    for span in spans.iter().skip(1) {
        full_span = full_span.merge(span);
    }

    Some(full_span)
}
