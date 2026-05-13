//Could this be u32?
/// General purpose span structure
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    // Maybe?
    // pub fn curate(&self, other: Span) -> Span {}

    /// Creates span that contains the min start and max end of two spans
    pub fn merge(&self, other: Span) -> Span {
        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        Span::new(start, end)
    }

    pub fn contains(&self, other: Span) -> bool {
        self.start <= other.start && self.end >= other.end
    }
}

/// Takes in an array of spans and merges all of them together. Expects that there is at least 1 span
/// present.
//TODO: Should just return option span
pub fn merge_spans(spans: &[Span]) -> Span {
    let mut full_span = *spans
        .get(0)
        .expect("Call to merge_spans with span length < 0");

    for span in spans.iter().skip(1).copied() {
        full_span = full_span.merge(span);
    }

    full_span
}
