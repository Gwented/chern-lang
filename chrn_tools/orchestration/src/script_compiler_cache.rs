use std::path::Path;

use chrn_utils::{
    chrn_config::ChrnConfig,
    core_error::{ModuleInitError, ScriptError},
    intern::Intern,
};
use compilation::{
    modules,
    script_compiler::{
        ScriptCompiler, reporter::Reporter, script_compiler_store::ScriptCompilerStore,
    },
};

//ScriptContext? CompilerContext? AbstractCompilerManager?

//TEST:
// 26 MB struct

// Not bit-flags. Stop.
// How.
pub(crate) struct ModuleCache {
    is_name_resolved: bool,
    is_type_resolved: bool,
    is_constraint_resolved: bool,
}

// Should check imports if more is needed to cache
//FIX:
pub struct ScriptCompilerCache {
    pub(crate) mod_cache: Vec<ModuleCache>,
}

impl ScriptCompilerCache {
    pub fn new() -> ScriptCompilerCache {
        ScriptCompilerCache {
            mod_cache: Default::default(),
        }
    }

    pub fn is_fully_resolved(&self) -> bool {
        let mut resolved_count = 0;
        for cache in &self.mod_cache {
            // Not wrapper method. We use pure C and MakeFile.
            if cache.is_name_resolved && cache.is_type_resolved && cache.is_constraint_resolved {
                resolved_count += 1;
            }
        }

        resolved_count == self.mod_cache.len()
    }
}
