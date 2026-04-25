// j*b

use std::collections::VecDeque;

use chrn_core::id_types::{AstId, ModuleId, ScopeId, SymbolId};
use common::span::Span;

use crate::semantic::scopes::ScopeType;

// Names are a little everywhere here..
#[derive(Debug)]
pub struct ValueContext {
    // SymbolInfo has the module of origin so not needed
    /// Symbol
    pub(super) jobs: VecDeque<Job>,
    pub(super) in_new_cycle: bool,
    pub(super) last_cycle_job_len: usize,
    // What if this was bitwise what if this was bitwise wh
    pub(super) status: JobStatus,
    // May help with dbg info
}

impl ValueContext {
    pub fn new() -> ValueContext {
        ValueContext {
            jobs: VecDeque::new(),
            last_cycle_job_len: 0,
            in_new_cycle: true,
            status: JobStatus::InProgress,
        }
    }

    pub fn is_done(&self) -> bool {
        self.jobs.len() == 0 || self.status == JobStatus::Failed
    }
}

// Will just keep all this information for now since this is not final
#[derive(Debug, Clone, Copy)]
pub(super) struct Job {
    pub(super) sym_id: SymbolId,
    pub(super) mod_id: ModuleId,
    pub(super) ast_id: AstId,
    pub(super) span: Span,
    // May remove either
    pub(super) scope_id: ScopeId,
    pub(super) scope_type: ScopeType,
}

impl Job {
    pub(super) fn new(
        sym_id: SymbolId,
        mod_id: ModuleId,
        ast_id: AstId,
        span: Span,
        scope_id: ScopeId,
        scope_type: ScopeType,
    ) -> Job {
        Job {
            sym_id,
            mod_id,
            ast_id,
            span,
            scope_id,
            scope_type,
        }
    }
}

/// Values for ensuring value resolution states are tracked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum JobStatus {
    InProgress,
    Complete,
    Failed,
}
