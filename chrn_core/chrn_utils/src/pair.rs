// IGNORE THIS

/// Generic pair structure that is a wrapper around a tuple (L, R)
pub struct Pair<L, R> {
    // Stays tuple internally so underlying translation is seamless
    pub inner: (L, R),
    // pub left: L,
    // pub right: R,
}

impl<L, R> Pair<L, R> {
    pub fn new(left: L, right: R) -> Pair<L, R> {
        Pair {
            inner: (left, right),
        }
    }

    pub fn from_tuple(tup: (L, R)) -> Pair<L, R> {
        Pair { inner: tup }
    }

    pub fn left(&self) -> &L {
        &self.inner.0
    }

    pub fn right(&self) -> &R {
        &self.inner.1
    }

    pub fn left_mut(&mut self) -> &mut L {
        &mut self.inner.0
    }

    pub fn right_mut(&mut self) -> &mut R {
        &mut self.inner.1
    }
}

impl<L, R> From<Pair<L, R>> for (L, R) {
    fn from(pair: Pair<L, R>) -> Self {
        pair.inner
    }
}

impl<L, R> Into<Pair<L, R>> for (L, R) {
    fn into(self) -> Pair<L, R> {
        Pair::from_tuple(self)
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
        self.pairs.iter().any(|pair| pair.inner.0 == *target)
    }

    pub fn contains_right(&self, target: &R) -> bool {
        self.pairs.iter().any(|pair| pair.inner.1 == *target)
    }

    pub fn contains_pair(&self, other: &Pair<L, R>) -> bool {
        self.pairs
            .iter()
            .any(|pair| pair.left() == other.left() && pair.right() == other.right())
    }

    pub fn contains_raw_pair(&self, other: &(L, R)) -> bool {
        self.pairs
            .iter()
            .any(|pair| pair.inner.0 == other.0 && pair.inner.1 == other.1)
    }
}

impl<L, R> From<Pairs<L, R>> for Vec<(L, R)> {
    fn from(mut pairs: Pairs<L, R>) -> Self {
        pairs.pairs.drain(..).map(|p| p.into()).collect()
    }
}

impl<L, R> Into<Pairs<L, R>> for Vec<(L, R)> {
    fn into(mut self) -> Pairs<L, R> {
        let mut pairs: Pairs<L, R> = Pairs::new();
        for tup in self.drain(..) {
            // (L, R) into Pair<L,R>
            pairs.push(tup.into());
        }
        pairs
    }
}

#[macro_export]
macro_rules! pairs {
    () => {
        $crate::pair::Pairs::new()
    };
    ($($pair:expr),+ $(,)?) => {{
        let mut pairs = $crate::pair::Pairs::new();
        $(
            pairs.push($pair.into());
        )*
        pairs
    }};
}

fn hi() {
    let pairs: Pairs<i32, i32> = pairs![Pair::new(0, 3), (2, 34)];
    let pairs: Pairs<i32, i32> = vec![(2, 5)].into();
}
