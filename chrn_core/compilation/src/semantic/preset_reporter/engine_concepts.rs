use chrn_utils::id_types::TypeId;

/// Allows for `EngineOption` to hold the current option to perform, as well as options to execute
/// on failure.
///
/// For example, if an operation wants to priotize similar member identifier help over listing all
/// members inside a struct/enum, it can make that the starting point, then on failure just list
/// all members.
#[derive(Debug, Clone)]
pub(super) struct EngineOptionBase {
    pub(super) opt: EngineOption,
    pub(super) on_failure: Option<Box<EngineOptionBase>>,
}

impl EngineOptionBase {
    pub(super) fn new(opt: EngineOption, on_failure: Option<Box<EngineOptionBase>>) -> Self {
        Self { opt, on_failure }
    }

    pub(super) fn builder(opt: EngineOption) -> EngineOptionBaseBuilder {
        EngineOptionBaseBuilder {
            opt,
            on_failure: None,
        }
    }
}

/// Convenience builder for `EngineOptionBase`
pub(super) struct EngineOptionBaseBuilder {
    pub(super) opt: EngineOption,
    pub(super) on_failure: Option<Box<EngineOptionBaseBuilder>>,
}

impl EngineOptionBaseBuilder {
    pub(super) fn on_failure(mut self, on_failure: EngineOptionBaseBuilder) -> Self {
        // Goes to end of tree before placing on_failure
        // Should this just, store a vector internally, push, then turn recursive?
        //
        // But at the same time, if enough on_failures are given to where this traversal is a
        // bottleneck I am concerned by my own actions far more than I am for performance.
        match self.on_failure {
            Some(inner) => {
                self.on_failure = Some(Box::new(inner.on_failure(on_failure)));
            }
            None => self.on_failure = Some(Box::new(on_failure)),
        }
        self
    }

    pub(super) fn build(self) -> EngineOptionBase {
        EngineOptionBase {
            opt: self.opt,
            on_failure: self.on_failure.map(|b| Box::new(b.build())),
        }
    }
}

/// Engine options
#[derive(Debug, Clone)]
pub(super) enum EngineOption {
    /// TypeId of parent to list the `AvailableKind` of
    ListAvailable(ListAvailable),
}

#[derive(Debug, Clone)]
pub(super) struct ListAvailable {
    /// TypeId to search for the `AvailableKind` of
    pub(super) type_id: TypeId,
    pub(super) kind: AvailableKind,
}

impl ListAvailable {
    pub(super) fn new(type_id: TypeId, kind: AvailableKind) -> Self {
        Self { type_id, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AvailableKind {
    Member,
    // Maybe stuff like function arguments should NOT be coupled with this since that seems like it
    // would want a differently structured message
    Args,
}

// impl Display for AvailableKind {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let out = match self {
//             AvailableKind::Member => "member",
//             AvailableKind::Args => "arg",
//         };
//         write!(f, "{out}")
//     }
// }
