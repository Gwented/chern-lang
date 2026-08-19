// -- PERF --

use std::time::{Duration, Instant};

use crate::utils::trackers::perf_tracker::{PerfOutput, PerfTracker};

// Looking. Odd.

// pub const LEXER: u16 = 1 << 0;
// pub const PARSER: u16 = 1 << 1;
// pub const NAMESPACE_RESOLVER: u16 = 1 << 2;
// pub const MEMBER_RESOLVER: u16 = 1 << 3;
// pub const TYPE_RESOLVER: u16 = 1 << 4;
// pub const CONSTRAINT_RESOLVER: u16 = 1 << 5;
// /// Does perf check for all stages
// pub const ALL: u16 =
//     LEXER | PARSER | NAMESPACE_RESOLVER | MEMBER_RESOLVER | TYPE_RESOLVER | CONSTRAINT_RESOLVER;

//TODO: How will this compensate for each module without being massively inconvenient

/// Compiler stages considered for performance review
pub const STAGES_COUNT: usize = ChrnPerfStage::CONSTRAINT_RESOLVER_IDX + 1;

/// Holds and orchestrates tracking info
#[derive(Debug, Default)]
pub struct ChrnPerf {
    can_use: bool,
    active_tracker: Option<PerfTracker>,
    tracked: [Option<PerfOutput>; STAGES_COUNT],
}

impl ChrnPerf {
    pub const fn new(can_use: bool) -> Self {
        Self {
            can_use,
            active_tracker: None,
            tracked: [None; STAGES_COUNT],
        }
    }

    // What if this took in a stage, and if the stage on stop doesn't match the start then it fails
    // an assertion?
    /// Returns perf tracker to use for the current run
    pub fn start(&mut self) {
        if self.can_use() {
            self.active_tracker = Some(PerfTracker::new(Instant::now()));
        }
    }

    // Do we want to assert this exists to prevent dev errors?
    /// Stores stage's time using the active perf
    pub fn stop(&mut self, stage: ChrnPerfStage) {
        // Equivalent to can_use
        if let Some(active) = self.active_tracker {
            if let Some(present) = &mut self.tracked[stage.to_idx()] {
                present.merge(active.stop());
            } else {
                self.tracked[stage.to_idx()] = Some(active.stop());
            }
            self.active_tracker = None;
        }
    }

    // Why are we trying so hard to keep it const!
    pub fn form_report(&self) -> ChrnPerfReport {
        let mut time_reports: [Option<ChrnPerfTimeReport>; STAGES_COUNT] = [None; STAGES_COUNT];
        // This is...random..seeming
        for (i, tracked) in self.tracked.iter().enumerate() {
            let time_report_opt = if let Some(out) = tracked {
                let stage = ChrnPerfStage::from_idx(i).expect("Idx should be aligned");
                // dbg!(stage, i);
                Some(ChrnPerfTimeReport::new(stage, out.elapsed, out.times))
            } else {
                None
            };
            time_reports[i] = time_report_opt;
        }
        ChrnPerfReport::new(time_reports)
    }

    // Might change again so stays wrapped
    /// Whether or not the tracker can be used
    pub const fn can_use(&self) -> bool {
        self.can_use
    }
}

///
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ChrnPerfStage {
    ModuleGraph,
    Lexer,
    Parser,
    NamespaceResolver,
    MemberResolver,
    TypeResolver,
    ConstraintResolver,
}

impl ChrnPerfStage {
    pub const MODULE_GRAPH: usize = 0;
    pub const LEXER_IDX: usize = 1;
    pub const PARSER_IDX: usize = 2;
    pub const NAMESPACE_RESOLVER_IDX: usize = 3;
    pub const MEMBER_RESOLVER_IDX: usize = 4;
    pub const TYPE_RESOLVER_IDX: usize = 5;
    pub const CONSTRAINT_RESOLVER_IDX: usize = 6;

    pub const fn to_idx(self) -> usize {
        match self {
            ChrnPerfStage::ModuleGraph => Self::MODULE_GRAPH,
            ChrnPerfStage::Lexer => Self::LEXER_IDX,
            ChrnPerfStage::Parser => Self::PARSER_IDX,
            ChrnPerfStage::NamespaceResolver => Self::NAMESPACE_RESOLVER_IDX,
            ChrnPerfStage::MemberResolver => Self::MEMBER_RESOLVER_IDX,
            ChrnPerfStage::TypeResolver => Self::TYPE_RESOLVER_IDX,
            ChrnPerfStage::ConstraintResolver => Self::CONSTRAINT_RESOLVER_IDX,
        }
    }

    pub const fn from_idx(idx: usize) -> Option<ChrnPerfStage> {
        match idx {
            Self::MODULE_GRAPH => Some(ChrnPerfStage::ModuleGraph),
            Self::LEXER_IDX => Some(ChrnPerfStage::Lexer),
            Self::PARSER_IDX => Some(ChrnPerfStage::Parser),
            Self::NAMESPACE_RESOLVER_IDX => Some(ChrnPerfStage::NamespaceResolver),
            Self::MEMBER_RESOLVER_IDX => Some(ChrnPerfStage::MemberResolver),
            Self::TYPE_RESOLVER_IDX => Some(ChrnPerfStage::TypeResolver),
            Self::CONSTRAINT_RESOLVER_IDX => Some(ChrnPerfStage::ConstraintResolver),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ChrnPerfReport {
    pub time_reports: [Option<ChrnPerfTimeReport>; STAGES_COUNT],
}

impl ChrnPerfReport {
    pub const fn new(time_reports: [Option<ChrnPerfTimeReport>; STAGES_COUNT]) -> Self {
        Self { time_reports }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ChrnPerfTimeReport {
    pub stage: ChrnPerfStage,
    pub time_spent: Duration,
    pub times: u16,
}

impl ChrnPerfTimeReport {
    pub const fn new(stage: ChrnPerfStage, time_spent: Duration, times: u16) -> Self {
        Self {
            stage,
            time_spent,
            times,
        }
    }
}

#[cfg(test)]
mod tests {
    fn chrn_perf_alignment_test() {}
}
