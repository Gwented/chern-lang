use chrn_utils::{
    arena::Arena,
    id_types::{InternedId, ModuleId, PathId, ScopeId, SourceRegionId, SymbolId},
    source_map::{source_region::SourceRegion, source_span::SourceSpan},
    utils::containers::SpannedContainer,
};

/// Imports used by user and compiler generated modules
#[derive(Debug, Clone)]
pub struct Import {
    pub name_id: InternedId,
    pub kind: ImportKind,
    pub sp_alias_id: Option<SpannedContainer<InternedId>>,
}

impl Import {
    pub const fn new(
        name_id: InternedId,
        kind: ImportKind,
        sp_alias_id: Option<SpannedContainer<InternedId>>,
    ) -> Import {
        Import {
            name_id,
            kind,
            sp_alias_id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImportKind {
    /// Import that is from a source file and fully resolved.
    /// Contains it's spanned path and module id
    Source(SpannedContainer<PathId>, ModuleId),
    /// Import from a source file that has a path attached to it, but no module id yet
    UnresolvedSource(SpannedContainer<PathId>),
    /// Import from source that had an unrecoverable error occur.
    /// This means the import should NOT be touched in any resolution scenario, unless for
    /// reporting or storing metadata.
    ErrorSource(SpannedContainer<PathId>),
    /// Core module originated importt
    Core(ModuleId),
}

#[derive(Debug, Default, Clone)]
pub struct Bind {
    pub path_id: PathId,
    pub path_span: SourceSpan,
}

impl Bind {
    pub const fn new(path_id: PathId, path_span: SourceSpan) -> Bind {
        Bind { path_id, path_span }
    }
}

//TODO:
//Maybe, a kind field that says user or builtin,
//or, a wrapper that has a module that could explicitly represent if it's user or not
//OR maybe src_metadata is actually a kind, which says whether it's user defined or not so it's
//just not a basic nullable field, and actually has meaning
/// Module
#[derive(Debug, Clone)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: InternedId,
    /// It's own module id position
    pub mod_id: ModuleId,
    /// Imports found in the module
    // What if imports were tagged with bit-wise?
    pub imports: Vec<Import>,
    /// Representation of the module's state
    pub state: ModuleState,
    pub bind: Option<Bind>,
    /// Represents the 5 known scopes as well as any local scopes
    pub scopes: Vec<ScopeId>,
    // HashSet maybe
    pub exports: Vec<SymbolId>,
    /// Metadata that exists if the module contains a source file
    // As of right now this represents the difference between a pre-loaded and user space module
    pub region_id: Option<SourceRegionId>,
}

impl Module {
    pub const fn new(
        name_id: InternedId,
        state: ModuleState,
        mod_id: ModuleId,
        bind: Option<Bind>,
        imports: Vec<Import>,
        //TODO: Convert to explicit kind
        region_id: Option<SourceRegionId>,
    ) -> Module {
        Module {
            name_id,
            mod_id,
            state,
            bind,
            imports,
            exports: Vec::new(),
            scopes: Vec::new(),
            region_id,
        }
    }
}

pub enum ModuleKind {
    User,
    Builtin,
}

/// A state for modules to be tracked by
// May or may not add more specific states like parsed and such, but this is fine
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    BrokenRegion,
    #[default]
    Loading,
    Loaded,
}

//TEST:
// Kind of what registry is doing right now
// pub struct SectionIndices {
//     neutral: Option<usize>,
//     var: Option<usize>,
//     nest: Option<usize>,
//     complex: Option<usize>,
//     // Ok
//     overrid: Option<usize>,
// }

//TODO: Methods
/// State needed to build the module graph from main to all sub modules
pub struct ModuleGraph {
    /// Arena
    pub region_arena: Arena<SourceRegion, SourceRegionId>,
    /// This is a Vector relationship stored where, the path id of an import is stored along with a
    /// `ModuleId`. So, we store main, go into main's imports then fill in OR create the module id of
    /// unknown imports based off of reserved len(). This works during the recursive process because
    /// it MUST look at all imports before ever recursing further, and it reserves it's spot as
    /// `None`.
    pub registered_mod_ids: Vec<(PathId, ModuleId)>,
    /// All paths seen
    pub seen: Vec<PathId>,
}

impl ModuleGraph {
    pub const fn new(
        region_arena: Arena<SourceRegion, SourceRegionId>,
        registered_mod_ids: Vec<(PathId, ModuleId)>,
        seen: Vec<PathId>,
    ) -> ModuleGraph {
        ModuleGraph {
            region_arena,
            registered_mod_ids,
            seen,
        }
    }
    // THEY MADE ME DO IT

    pub const fn region_arena(&self) -> &Arena<SourceRegion, SourceRegionId> {
        &self.region_arena
    }

    pub const fn registered_mod_ids(&self) -> &Vec<(PathId, ModuleId)> {
        &self.registered_mod_ids
    }

    // pub const fn other_mods(&self) -> &Vec<Option<Module>> {
    //     &self.other_mods
    // }

    pub const fn seen(&self) -> &Vec<PathId> {
        &self.seen
    }
}

/// One identifier a module binds. An import binds its alias when given, otherwise its file name,
/// so a collision between any two of them is a duplicate identifier.
#[derive(Debug, Clone, Eq)]
pub(super) struct ModuleIdent {
    /// Identifier which is an import's alias if present, it's file name otherwise.
    pub(super) ident_id: InternedId,
    /// Whether `ident_id` came from an alias. Only used to pick the diagnostic wording.
    pub(super) is_alias: bool,
    /// Is either a path span or alias span.
    ///
    /// This is only `Option` because a root module does not have a span to point to.
    pub(super) span: Option<SourceSpan>,
}

impl ModuleIdent {
    pub(super) const fn new(
        ident_id: InternedId,
        is_alias: bool,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            ident_id,
            is_alias,
            span,
        }
    }
}

// Is this good though? It DOES encode the meaning we want, but it's also pretty transient, but it
// also would be fairly inconvenient to make another system that does what the tracker does just
// because it changes a trait. It's not like this is an operator overload it's just defining how
// this struct should be hashed.

// `is_alias` and `path_span` are metadata for reporting, so only the identifier is checked for
// equality and hashing. This is done to let `DuplicateTracker<T>` hash every colliding identifier
// together, whether it came from a file name or an alias.
impl PartialEq for ModuleIdent {
    fn eq(&self, other: &Self) -> bool {
        self.ident_id == other.ident_id
    }
}

impl std::hash::Hash for ModuleIdent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ident_id.hash(state);
    }
}
