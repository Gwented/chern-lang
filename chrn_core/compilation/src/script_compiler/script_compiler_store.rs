use chrn_utils::{
    chrn_settings::ChrnSettings, intern::Intern, source_map::source_region::SourceRegionArena,
};
use lang::trivia::Trivia;

use crate::{parser::ast::AstInfo, token::SpannedToken};

/// Stores all essential data collected through compilation stages
///
/// These are not recognized as cache, but more so as a structure that holds data for the sake of
/// maintaing compilation steps in a structure instead of requiring it to be stored by users of
/// this structure explicitly.
#[derive(Debug)]
pub struct ScriptCompilerStore {
    /// Settings given to chrn compilation instance
    pub settings: ChrnSettings,
    /// Region arena found after building module graph
    pub region_arena: SourceRegionArena,
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
        settings: ChrnSettings,
        region_arena: SourceRegionArena,
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
