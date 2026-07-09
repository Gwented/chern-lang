use chrn_utils::{
    arena::Arena,
    chrn_config::ChrnConfig,
    id_types::{SourceRegionId, SymbolId},
    intern::Intern,
    source_map::source_region::SourceRegion,
};

use crate::{
    lexer::{token::SpannedToken, trivia::Trivia},
    parser::ast::ast_concepts::AstInfo,
};

// NOTE:

/// Stores all essential data collected through compilation stages
///
/// These are not recognized as cache, but more so as a structure that holds data for the sake of
/// maintaing compilation steps in a structure instead of requiring it to be stored by users of
/// this structure explicitly.
///
/// NOTE: These are all intended to be dense arrays that should align with `ModuleId`
#[derive(Debug)]
pub struct ScriptCompilerStore {
    /// Settings given to chrn compilation instance
    pub cfg: ChrnConfig,
    /// Region arena found after building module graph
    pub region_arena: Arena<SourceRegion, SourceRegionId>,
    // Beautiful
    /// Interner 😭
    pub interner: Intern,
    /// These are `Option` types due to modules being stored in a dense array
    pub toks: Vec<Option<Vec<SpannedToken>>>,
    /// These are `Option` types due to modules being stored in a dense array
    pub trivias: Vec<Option<Vec<Trivia>>>,
    /// These are `Option` types due to modules being stored in a dense array
    pub asts: Vec<Option<AstInfo>>,
    /// Symbols specific to a module's compilation
    /// These are `Option` types due to modules being stored in a dense array
    pub compilation_syms: Vec<Option<Vec<SymbolId>>>,
}

impl ScriptCompilerStore {
    pub fn new(
        cfg: ChrnConfig,
        region_arena: Arena<SourceRegion, SourceRegionId>,
        interner: Intern,
        toks: Vec<Option<Vec<SpannedToken>>>,
        trivias: Vec<Option<Vec<Trivia>>>,
        asts: Vec<Option<AstInfo>>,
        compilation_syms: Vec<Option<Vec<SymbolId>>>,
    ) -> ScriptCompilerStore {
        ScriptCompilerStore {
            cfg,
            region_arena,
            interner,
            toks,
            trivias,
            asts,
            compilation_syms,
        }
    }
}
