//TODO: Should maybe be somewhere else but fine for now
//Could this be u32?
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn merge(&self, other: Span) -> Span {
        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        Span::new(start, end)
    }
}
