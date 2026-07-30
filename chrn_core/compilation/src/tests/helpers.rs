pub(super) use crate::{
    lexer::token::{Notation, Token},
    parser::ast::ast_concepts::AstInfo,
    resolvers::{
        constraint_resolver::ConstraintResolver,
        resolver_env::{RegistrationEnv, ResolverEnv},
    },
    script_compiler::{ScriptCompiler, reporter::Reporter},
};
// -- Helpers --
/// Creates fake strings for the amounts given
pub(super) fn mock_interner(str_amt: usize, path_amt: usize) -> Intern {
    let mut interner = Intern::init();

    for idx in 0..str_amt {
        let s = format!("dummyname{idx}");
        interner.intern(&s);
    }

    for idx in 0..path_amt {
        let p = format!("dummyimport{idx}");
        let p = Path::new(&p);
        interner.intern_path(&p);
    }

    interner
}

pub(super) trait ConfigLoaderOutputExt {
    fn expect_success(self) -> SourceRegion;
}

impl ConfigLoaderOutputExt for ConfigLoaderOutput {
    fn expect_success(self) -> SourceRegion {
        match self {
            ConfigLoaderOutput::Success(region, _) => region,
            other => panic!("expected ConfigLoaderOutput::Success, got {other:?}"),
        }
    }
}

pub(super) fn get_module_region<'a>(
    arena: &'a Arena<SourceRegion, SourceRegionId>,
    module: &Module,
) -> &'a SourceRegion {
    let region_id = module
        .region_id
        .expect("Module should have a source region");
    &arena[region_id]
}

pub(super) fn mock_single_module_compiler(
    text: &str,
) -> (
    Arena<SourceRegion, SourceRegionId>,
    Intern,
    ChrnConfig,
    ScriptCompiler,
) {
    let interner = mock_interner(0, 1);
    let settings = ChrnConfig::default();
    let path_id = PathId::new(0);
    let region_id = SourceRegionId::new(0);

    let source_region = ConfigLoader::new(region_id, text.as_bytes(), path_id, &settings)
        .load_config()
        .expect_success();

    let module = Module::new(
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Some(region_id),
    );

    // Should use compiler store now
    let mut arena = Arena::<SourceRegion, SourceRegionId>::new();
    arena.push(source_region);
    let compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(vec![module]));

    (arena, interner, settings, compiler)
}

pub(super) fn mock_import(
    name: &str,
    path_name: &str,
    mod_id: ModuleId,
    alias_id: Option<&str>,
    interner: &mut Intern,
) -> Import {
    let kind = ImportKind::Source(
        SpannedContainer::new(
            interner.intern_path(&Path::new(path_name)),
            SourceSpan::default(),
        ),
        mod_id,
    );
    Import::new(
        interner.intern(name),
        kind,
        alias_id.map(|a| interner.intern(&a)),
    )
}

pub(super) fn mock_single_module(
    name: &str,
    path_name: &str,
    imports: Vec<Import>,
    mod_id: u32,
    text: &str,
    interner: &mut Intern,
) -> (Module, SourceRegion) {
    let settings = ChrnConfig::default();
    let path_id = interner.intern_path(Path::new(path_name));
    let region_id = SourceRegionId::new(mod_id as u32);

    let source_region = ConfigLoader::new(region_id, text.as_bytes(), path_id, &settings)
        .load_config()
        .expect_success();

    let module = Module::new(
        interner.intern(name),
        ModuleState::Loaded,
        ModuleId::new(mod_id),
        None,
        imports,
        Some(region_id),
    );

    (module, source_region)
}

pub(super) fn mock_multiple_module_compiler(
    modules_with_regions: Vec<(Module, SourceRegion)>,
) -> (
    Arena<SourceRegion, SourceRegionId>,
    Intern,
    ChrnConfig,
    ScriptCompiler,
) {
    let interner = mock_interner(0, modules_with_regions.len());
    let settings = ChrnConfig::default();

    let (modules, regions): (Vec<Module>, Vec<SourceRegion>) =
        modules_with_regions.into_iter().unzip();

    let mut arena = Arena::<SourceRegion, SourceRegionId>::new();
    for region in regions {
        arena.push(region);
    }
    let compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(modules));

    (arena, interner, settings, compiler)
}
/// Builds registration environments aligned with compiler modules from their ASTs.
///
/// Mirrors the orchestrator's `create_registration_envs`: a module may have a `region_id`
/// (e.g. a `BrokenRegion` module that the config loader still allocated) without an `AstInfo`
/// entry ever having been produced for it, because the AST is created *from* the region but
/// the orchestrator can skip ast creation when the lexer returns `None`. Such modules must
/// produce `None` here, not panic.
pub(super) fn build_registration_envs<'a>(
    compiler: &ScriptCompiler,
    arena: &'a Arena<SourceRegion, SourceRegionId>,
    asts: &'a [Option<AstInfo>],
) -> Vec<Option<RegistrationEnv<'a>>> {
    let mut all_envs = Vec::new();
    for i in 0..compiler.mods.len() {
        let mod_id = ModuleId::new(i as u32);
        let module = &compiler.mods[mod_id];

        // No region => no env. This is the path for lib-style modules with no source
        // (e.g. the implicit `core` module injected by `ScriptCompiler::init`).
        let current_region = match &module.region_id {
            Some(region_id) => &arena[*region_id],
            None => {
                all_envs.push(None);
                continue;
            }
        };

        // A module can have a region id without a corresponding AstInfo: the ast is built
        // from the region, but a broken region (or a `Loaded` module whose lexer step was
        // skipped) means the slot in `asts` is still `None`. Drop the env rather than
        // crashing, matching the orchestrator.
        let current_ast = match asts[i].as_ref() {
            Some(ast) => ast,
            None => {
                all_envs.push(None);
                continue;
            }
        };

        let env = RegistrationEnv::new(current_ast, current_region, module.mod_id);
        all_envs.push(Some(env));
    }
    all_envs
}

/// Builds resolver environments aligned with compiler modules from their ASTs.
///
/// Mirrors the orchestrator's `create_resolver_envs`: a module must have a region, an
/// `AstInfo` entry, and a `compilation_syms` entry to produce a `ResolverEnv`. Any missing
/// piece means the env slot is `None`, never a panic. The AST is derived from the region
/// but its presence is not implied by the region's presence, and the `compilation_syms` slot
/// is independently populated by the namespace resolver pass, so each is checked separately.
pub(super) fn build_resolver_envs<'a>(
    compiler: &ScriptCompiler,
    arena: &'a Arena<SourceRegion, SourceRegionId>,
    asts: &'a [Option<AstInfo>],
    compilation_syms: &'a [Option<Vec<CompilationUnit>>],
) -> Vec<Option<ResolverEnv<'a>>> {
    let mut all_envs = Vec::new();
    for i in 0..compiler.mods.len() {
        let mod_id = ModuleId::new(i as u32);
        let module = &compiler.mods[mod_id];

        let current_region = match &module.region_id {
            Some(region_id) => &arena[*region_id],
            None => {
                all_envs.push(None);
                continue;
            }
        };

        let current_ast = match asts[i].as_ref() {
            Some(ast) => ast,
            None => {
                all_envs.push(None);
                continue;
            }
        };

        // `compilation_syms` is filled in by the namespace resolver pass. If that pass
        // didn't run for this module (e.g. its `RegistrationEnv` was `None`), this slot is
        // `None` and we must skip rather than panic.
        let comp_syms = match compilation_syms[i].as_ref() {
            Some(syms) => syms,
            None => {
                all_envs.push(None);
                continue;
            }
        };

        let env = ResolverEnv::new(current_ast, current_region, module.mod_id, comp_syms);
        all_envs.push(Some(env));
    }
    all_envs
}

/// Runs namespace resolution across all registration environments, returning the
/// module-aligned compilation symbol lists. Panics if any module produces diagnostics.
pub(super) fn run_namespace_resolver(
    settings: &ChrnConfig,
    interner: &Intern,
    compiler: &mut ScriptCompiler,
    reg_envs: &[Option<RegistrationEnv>],
) -> Vec<Option<Vec<CompilationUnit>>> {
    let mut ns_resolver = NamespaceResolver::new(settings, interner, compiler);
    let mut mod_symbols = Vec::new();
    for env in reg_envs.iter() {
        if let Some(env) = env {
            let (current_mod_symbols, diags) = ns_resolver.resolve(env);
            assert!(
                diags.diags.is_empty(),
                "Namespace resolution failed: {:?}",
                diags
            );
            mod_symbols.push(Some(current_mod_symbols));
        } else {
            mod_symbols.push(None);
        }
    }
    mod_symbols
}

/// Runs member resolution across all resolver environments, panicking on diagnostics
pub(super) fn run_member_resolver(
    settings: &ChrnConfig,
    envs: &[Option<ResolverEnv>],
    interner: &Intern,
    compiler: &mut ScriptCompiler,
) {
    let mut member_resolver = MemberResolver::new(settings, interner, compiler);
    for env in envs.iter() {
        if let Some(env) = env {
            let diags = member_resolver.resolve(env);
            assert!(
                diags.diags.is_empty(),
                "Member resolution failed: {:?}",
                diags
            );
        }
    }
}

pub(super) use std::path::Path;

use crate::config_loader::{ConfigLoader, ConfigLoaderOutput};
use chrn_utils::id_types::SpannedContainer;
pub(super) use chrn_utils::{
    arena::Arena,
    budget::mem_budget::{BudgetResult, MemoryBudget},
    chrn_config::ChrnConfig,
    core_error::ConfigLoadError,
    id_types::{InternedId, ModuleId, PathId, SourceRegionId, SymbolId, ValueId},
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};
pub(super) use lang::{keywords::Keyword, values::Value};

pub(super) use crate::{
    lexer::Lexer,
    lookup::scopes::ScopeType,
    modules::{Import, ImportKind, Module, ModuleState},
    parser::{self},
    resolvers::{
        member_resolver::MemberResolver, name_resolver::NamespaceResolver,
        type_resolver::TypeResolver,
    },
    semantic::{
        compilation_unit::CompilationUnit,
        hir::hir_symbols::VariableState,
    },
};

// -- Const dependency test helpers --

/// Parses a single-module script and runs the full resolution pipeline up to and including
/// constraints. Panics on any resolution error so that the returned compiler state is known to
/// be fully resolved.
pub(super) fn compile_and_resolve_single_module(text: &str) -> (ScriptCompiler, Intern) {
    let (arena, mut interner, cfg, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&cfg, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (comp_syms, _) = NamespaceResolver::new(&cfg, &interner, &mut compiler).resolve(&reg_env);

    let res_env = ResolverEnv::new(&ast_info, region, mod_id, &comp_syms);
    let envs = vec![Some(res_env)];
    run_member_resolver(&cfg, &envs, &interner, &mut compiler);
    let env = envs[0].as_ref().expect("Env should exist");

    let ty_summary = TypeResolver::new(&cfg, &mut interner, &mut compiler).resolve(env);
    assert!(ty_summary.err_count() == 0, "Type resolution failed");
    let constraint_summary = ConstraintResolver::new(&cfg, &interner, &mut compiler).resolve(env);
    assert!(
        constraint_summary.err_count() == 0,
        "Constraint resolution failed"
    );

    (compiler, interner)
}

/// Returns the constant value of a resolved `let` variable by name.
pub(super) fn value_of(compiler: &ScriptCompiler, interner: &Intern, name: &str) -> Value {
    let name_id = interner
        .try_search_str(name)
        .unwrap_or_else(|| panic!("Variable '{}' was not interned", name));
    let var_def = compiler
        .variables
        .iter()
        .find(|v| v.name_id == name_id)
        .unwrap_or_else(|| panic!("Variable '{}' not found", name));

    match &var_def.state {
        VariableState::Known(value_id) => compiler.values[*value_id]
            .const_val
            .clone()
            .unwrap_or_else(|| panic!("Variable '{}' has no constant value", name)),
        VariableState::ReservedTypeSlot(_) => {
            panic!("Variable '{}' is still a reserved type slot", name)
        }
    }
}

/// Runs namespace and member resolution, then returns the result of type resolution. This is
/// useful for tests that want to assert that type resolution fails (e.g. circular const
/// dependencies) without the constraint pass running.
pub(super) fn type_resolve_single_module(
    text: &str,
) -> Result<(ScriptCompiler, Intern), Vec<SourceDiagnostic>> {
    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (comp_syms, _) =
        NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);

    let res_env = ResolverEnv::new(&ast_info, region, mod_id, &comp_syms);
    let envs = vec![Some(res_env)];
    run_member_resolver(&settings, &envs, &interner, &mut compiler);
    let env = envs[0].as_ref().expect("Env should exist");

    match TypeResolver::new(&settings, &mut interner, &mut compiler).resolve(env) {
        summary if summary.err_count() == 0 => Ok((compiler, interner)),
        summary => Err(summary.diags),
    }
}

/// Like `type_resolve_single_module` but returns the compiler and interner even on error, so
/// callers can inspect the partial resolution state (e.g. verify that variables involved in a
/// circular dependency remain in `ReservedTypeSlot`).
pub(super) fn type_resolve_single_module_keep_state(
    text: &str,
) -> (Result<(), Vec<SourceDiagnostic>>, ScriptCompiler, Intern) {
    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (comp_syms, _) =
        NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);

    let res_env = ResolverEnv::new(&ast_info, region, mod_id, &comp_syms);
    let envs = vec![Some(res_env)];
    run_member_resolver(&settings, &envs, &interner, &mut compiler);
    let env = envs[0].as_ref().expect("Env should exist");

    let summary = TypeResolver::new(&settings, &mut interner, &mut compiler).resolve(env);
    if summary.err_count() == 0 {
        (Ok(()), compiler, interner)
    } else {
        (Err(summary.diags), compiler, interner)
    }
}

pub(super) fn load_cfg_bytes(bytes: &[u8]) -> ConfigLoaderOutput {
    let mut interner = mock_interner(0, 1);
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    ConfigLoader::new(region_id, bytes, path_id, &ChrnConfig::default()).load_config()
}

/// Helper: runs the config loader on a string and returns the resulting region.
pub(super) fn load_cfg(text: &str) -> ConfigLoaderOutput {
    load_cfg_bytes(text.as_bytes())
}

/// `@def` immediately followed by `@end` with no separator. The loader does
/// `self.skip(4)` then unconditionally `self.advance()` after the `@def` match, which
/// consumes one extra byte and skips the `@` of `@end`. This test pins the current behavior
/// so a future fix surfaces as a real test change rather than a silent regression.
pub(super) fn compile_and_resolve_cross_module(
    main_text: &str,
    sub_text: &str,
) -> (ScriptCompiler, Intern) {
    let mut interner = Intern::init();

    let import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        None,
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![import],
        0,
        main_text,
        &mut interner,
    );

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        Default::default(),
        1,
        sub_text,
        &mut interner,
    );

    let (arena, _, cfg, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();
    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner)
            .toks;
        asts.push(Some(parser::parse(&cfg, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<CompilationUnit>>> = {
        let mut ns_resolver = NamespaceResolver::new(&cfg, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(
                    diags.diags.is_empty(),
                    "Namespace resolution failed: {:?}",
                    diags
                );
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&cfg, &resolver_envs, &interner, &mut compiler);

    let mut ty_resolver = TypeResolver::new(&cfg, &mut interner, &mut compiler);
    for env in resolver_envs.iter() {
        if let Some(env) = env {
            let summary = ty_resolver.resolve(env);
            assert!(summary.err_count() == 0, "Type resolution failed");
        }
    }

    let mut contraint_resolver = ConstraintResolver::new(&cfg, &interner, &mut compiler);
    for env in resolver_envs.iter().flatten() {
        let summary = contraint_resolver.resolve(env);
        assert!(summary.err_count() == 0, "Constraint resolution failed");
    }

    (compiler, interner)
}

/// Runs namespace and member resolution across two modules, then type resolution.
/// Returns Ok if all type resolutions pass, Err with all diagnostics otherwise.
pub(super) fn type_resolve_cross_module(
    main_text: &str,
    sub_text: &str,
) -> Result<(ScriptCompiler, Intern), Vec<SourceDiagnostic>> {
    let mut interner = Intern::init();

    let import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        None,
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![import],
        0,
        main_text,
        &mut interner,
    );

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        Default::default(),
        1,
        sub_text,
        &mut interner,
    );

    let (arena, _, settings, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();
    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner)
            .toks;
        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<CompilationUnit>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(
                    diags.diags.is_empty(),
                    "Namespace resolution failed: {:?}",
                    diags
                );
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    let mut all_diags = Vec::new();

    let mut ty_resolver = TypeResolver::new(&settings, &mut interner, &mut compiler);
    for env in resolver_envs.iter() {
        if let Some(env) = env {
            let summary = ty_resolver.resolve(env);
            if summary.err_count() > 0 {
                all_diags.extend(summary.diags);
            }
        }
    }

    if all_diags.is_empty() {
        Ok((compiler, interner))
    } else {
        Err(all_diags)
    }
}

pub(super) fn make_diagnostics(amt: usize) -> Vec<SourceDiagnostic> {
    let mut diags = Vec::new();
    for i in 0..amt {
        diags.push(SourceDiagnostic::new(
            None,
            DiagnosticLevel::Error,
            Default::default(),
            PathId::new(i as u32),
            Default::default(),
            Default::default(),
            Default::default(),
        ));
    }
    diags
}
