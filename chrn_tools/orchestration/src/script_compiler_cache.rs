use std::path::Path;

use chrn_utils::{
    chrn_config::ChrnConfig,
    core_error::{ModuleInitError, ScriptError},
    intern::Intern,
    source_map::source_diagnostic::Reporter,
};
use compilation::{
    modules,
    script_compiler::{ScriptCompiler, script_compiler_store::ScriptCompilerStore},
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

/// Creates
pub fn create_compiler_with_cache(
    path: &Path,
    reporter: &mut Reporter,
    cfg: ChrnConfig,
    // I'm so scared
) -> Result<(ScriptCompiler, ScriptCompilerStore, ScriptCompilerCache), ModuleInitError> {
    let interner = Intern::init();
    // let mut spans = SpanArena::new(Vec::new());

    // I'm so scared
    let (compiler, compiler_store, mut diags) =
        modules::extract_modules(path, cfg, reporter, interner)?;
    reporter.append_safe(&mut diags);

    let cache = ScriptCompilerCache {
        // spans,
        mod_cache: Default::default(),
    };

    Ok((compiler, compiler_store, cache))
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
