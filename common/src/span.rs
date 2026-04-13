//TODO: Should maybe be somewhere else but fine for now
//Could this be u32?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    // pub fn contains_inclusive(&self, span: &Span) -> bool {
    //     let self_range = self.start..=self.end;
    //     let other_range = span.start..=span.end;
    // }
}
