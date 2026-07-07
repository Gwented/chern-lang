// IGNORE THIS

/// Generic pair structure that is a wrapper around a tuple (L, R)
pub struct Pair<L, R> {
    pub left: L,
    pub right: R,
}

impl<L, R> Pair<L, R> {
    pub fn new(left: L, right: R) -> Pair<L, R> {
        Pair { left, right }
    }
}

impl<L, R> From<Pair<L, R>> for (L, R) {
    fn from(pair: Pair<L, R>) -> Self {
        (pair.left, pair.right)
    }
}

/// Generic pair vector structure that is a wrapper around a Vec<(L, R)>
///
/// This is intended to make such relationships less confusing for scenarios where a pair is needed,
/// but it also needs to use round-about iters where it's so convoluted looking to where it would be
/// more fit as it's own structure. This serves as said structure without being case-specific.
pub struct Pairs<L, R> {
    pub pairs: Vec<Pair<L, R>>,
}

impl<L, R> Pairs<L, R> {
    pub fn new() -> Pairs<L, R> {
        Pairs { pairs: Vec::new() }
    }

    pub fn push(&mut self, other: Pair<L, R>) {
        self.pairs.push(other);
    }
}

impl<L: PartialEq, R: PartialEq> Pairs<L, R> {
    pub fn contains_left(&self, target: &L) -> bool {
        self.pairs.iter().any(|pair| pair.left == *target)
    }

    pub fn contains_right(&self, target: &R) -> bool {
        self.pairs.iter().any(|pair| pair.right == *target)
    }

    pub fn contains_pair(&self, other: &Pair<L, R>) -> bool {
        self.pairs
            .iter()
            .any(|pair| pair.left == other.left && pair.right == other.right)
    }

    pub fn contains_raw_pair(&self, other: &(L, R)) -> bool {
        self.pairs
            .iter()
            .any(|pair| pair.left == other.0 && pair.right == other.1)
    }
}

// hi
impl<L, R> From<Pairs<L, R>> for Vec<(L, R)> {
    fn from(mut pairs: Pairs<L, R>) -> Self {
        pairs.pairs.drain(..).map(|p| p.into()).collect()
    }
}

#[macro_export]
macro_rules! pairs {
    () => {
        $crate::pair::Pairs::new()
    };
    ($(($left:expr, $right:expr)),+ $(,)?) => {};
}
