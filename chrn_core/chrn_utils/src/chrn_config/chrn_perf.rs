// -- PERF --

use crate::utils::trackers::perf_tracker::PerfTracker;

/// Performance tracker
#[derive(Debug, Default)]
pub struct ChrnPerfTracker {
    flags: Option<u16>,
}

impl ChrnPerfTracker {
    /// Expects internal constant options like `ChrnPerfTracker::LEXER` to be used to choose what
    /// stages should have their performance tracked.
    ///
    /// If given `None`, then no performance checks are done.
    pub const fn new(flags: Option<u16>) -> Self {
        Self { flags }
    }

    pub fn report(&self) {}

    /// Whether or not the tracker can be used
    fn can_use(&self) -> bool {
        self.flags.is_some()
    }
}

#[derive(Debug)]
pub enum ChrnPerfOption {
    Lexer,
    Parser,
    NamespaceResolver,
    MemberResolver,
    TypeResolver,
    ConstraintResolver,
}

impl ChrnPerfOption {
    pub const LEXER: u16 = 1 << 0;
    pub const PARSER: u16 = 1 << 1;
    pub const NAMESPACE_RESOLVER: u16 = 1 << 2;
    pub const MEMBER_RESOLVER: u16 = 1 << 3;
    pub const TYPE_RESOLVER: u16 = 1 << 4;
    pub const CONSTRAINT_RESOLVER: u16 = 1 << 5;

    /// Does perf check for all stages
    pub const ALL: u16 = Self::LEXER
        | Self::PARSER
        | Self::NAMESPACE_RESOLVER
        | Self::MEMBER_RESOLVER
        | Self::TYPE_RESOLVER
        | Self::CONSTRAINT_RESOLVER;

    pub fn to_bits(self) -> u16 {
        match self {
            ChrnPerfOption::Lexer => Self::LEXER,
            ChrnPerfOption::Parser => Self::PARSER,
            ChrnPerfOption::NamespaceResolver => Self::NAMESPACE_RESOLVER,
            ChrnPerfOption::MemberResolver => Self::MEMBER_RESOLVER,
            ChrnPerfOption::TypeResolver => Self::TYPE_RESOLVER,
            ChrnPerfOption::ConstraintResolver => Self::CONSTRAINT_RESOLVER,
        }
    }
}

#[cfg(test)]
mod tests {
    fn chrn_perf_alignment_test() {}
}
