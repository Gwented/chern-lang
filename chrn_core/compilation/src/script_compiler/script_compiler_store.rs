use chrn_utils::{
    arena::Arena, chrn_config::ChrnConfig, id_types::SourceRegionId, intern::Intern,
    source_map::source_region::SourceRegion,
};

use crate::{
    lexer::{token::SpannedToken, trivia::Trivia},
    parser::ast::ast_concepts::AstInfo,
};

/// Stores all essential data collected through compilation stages
///
/// These are not recognized as cache, but more so as a structure that holds data for the sake of
/// maintaing compilation steps in a structure instead of requiring it to be stored by users of
/// this structure explicitly.
#[derive(Debug)]
pub struct ScriptCompilerStore {
    /// Settings given to chrn compilation instance
    pub settings: ChrnConfig,
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
}

impl ScriptCompilerStore {
    pub fn new(
        settings: ChrnConfig,
        region_arena: Arena<SourceRegion, SourceRegionId>,
        interner: Intern,
        toks: Vec<Option<Vec<SpannedToken>>>,
        trivias: Vec<Option<Vec<Trivia>>>,
        asts: Vec<Option<AstInfo>>,
    ) -> ScriptCompilerStore {
        ScriptCompilerStore {
            settings,
            region_arena,
            interner,
            toks,
            trivias,
            asts,
        }
    }
}
