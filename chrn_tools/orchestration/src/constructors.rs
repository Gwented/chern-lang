use std::path::Path;

use chrn_utils::{chrn_config::ChrnConfig, core_error::ModuleInitError, intern::Intern};
use compilation::{
    modules,
    script_compiler::{
        ScriptCompiler, reporter::Reporter, script_compiler_store::ScriptCompilerStore,
    },
};

use crate::script_compiler_cache::ScriptCompilerCache;

/// Performs the minimum operates to create a `ScriptCompiler` and `ScriptCompilerStore`.
///
/// If the loading stage fails critically, returns `Err(ModuleInitError)`
pub fn create_compiler(
    path: &Path,
    reporter: &mut Reporter,
    cfg: ChrnConfig,
) -> Result<(ScriptCompiler, ScriptCompilerStore), ModuleInitError> {
    let (compiler, store, mut diags) = modules::extract_all_modules(path, cfg, reporter)?;
    reporter.append_safe(&mut diags);
    Ok((compiler, store))
}

// Not sure if this will stay
/// Performs the minimum operates to create a `ScriptCompiler` and `ScriptCompilerStore`, then
/// creates `ScriptCompilerCache` alongside it.
///
/// If the loading stage fails critically, returns `Err(ModuleInitError)`
pub fn create_compiler_with_cache(
    path: &Path,
    reporter: &mut Reporter,
    cfg: ChrnConfig,
    // I'm so scared
) -> Result<(ScriptCompiler, ScriptCompilerStore, ScriptCompilerCache), ModuleInitError> {
    let (compiler, compiler_store, mut diags) = modules::extract_all_modules(path, cfg, reporter)?;
    reporter.append_safe(&mut diags);

    let cache = ScriptCompilerCache {
        mod_cache: Default::default(),
    };

    Ok((compiler, compiler_store, cache))
}
