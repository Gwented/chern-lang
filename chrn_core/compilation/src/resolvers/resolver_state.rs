use bitflags::bitflags;

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum ResolverState {
//     Namespace,
//     Member,
//     Type,
//     Constraint,
// }

//TEST: May or may not have stages depend on parts of other stages so these are bitflags not enums
bitflags! {
    /// State that matches to a resolver to allow for external users to track and compare states
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ResolverState: u16 {
        const NAMESPACE = 1 << 1;
        const MEMBER = 1 << 2;
        const TYPE = 1 << 3;
        const CONSTRAINT = 1 << 4;
        const COMPLETE = 1 << 5;
    }
}

//WARN: These are unsafe...
impl ResolverState {
    /// Gets what would be the next state
    pub fn next_state(self) -> ResolverState {
        match self {
            Self::NAMESPACE => Self::MEMBER,
            Self::MEMBER => Self::TYPE,
            Self::TYPE => Self::CONSTRAINT,
            _ => Self::COMPLETE,
        }
    }

    /// Gets what would be the previous state if possible
    pub fn prev_state(self) -> Option<ResolverState> {
        match self {
            Self::COMPLETE => Some(Self::TYPE),
            Self::TYPE => Some(Self::MEMBER),
            Self::MEMBER => Some(Self::NAMESPACE),
            Self::CONSTRAINT => Some(Self::TYPE),
            _ => None,
        }
    }

    /// Mutates current state to the next possible state
    pub fn advance(&mut self) {
        let out = match *self {
            Self::NAMESPACE => Self::MEMBER,
            Self::MEMBER => Self::TYPE,
            Self::TYPE => Self::CONSTRAINT,
            _ => Self::COMPLETE,
        };

        *self = out;
    }
}
