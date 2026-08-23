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
        alias_id.map(|a| SpannedContainer::new(interner.intern(&a), SourceSpan::default())),
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

pub(super) use std::path::Path;

use crate::config_loader::{ConfigLoader, ConfigLoaderOutput};
use chrn_utils::utils::containers::SpannedContainer;
pub(super) use chrn_utils::{
    arena::Arena,
    budget::mem_budget::{BudgetResult, MemoryBudget},
    chrn_config::ChrnConfig,
    core_error::ConfigLoadError,
    id_types::{InternedId, ModuleId, PathId, SourceRegionId, SymbolId, ValueId},
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic, SourceDiagnosticSummary},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};
pub(super) use lang::{keywords::Keyword, values::Value};

pub(super) use crate::{
    lexer::Lexer,
    module::module_concepts::{Import, ImportKind, Module, ModuleState},
    parser::{self},
    resolvers::{
        member_resolver::MemberResolver, name_resolver::NamespaceResolver,
        type_resolver::TypeResolver,
    },
    semantic::{compilation_unit::CompilationUnit, hir::hir_symbols::VariableState},
};

// -- Pipeline driver --

/// Last resolver stage a pipeline run should execute. Stages are ordered and cumulative:
/// running one runs every earlier one, matching the order-locked `ResolverState` machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Stage {
    Namespace,
    Member,
    Type,
    Constraint,
}

/// Output of a pipeline run: the resulting compiler state, the interner it was built with,
/// and one diagnostic summary per stage. Stages that did not run hold empty summaries.
///
/// This is the single place tests get resolver output from. Adding a parameter to a resolver
/// or reordering a stage is a change to `resolve_single_module` / `resolve_cross_module`
/// only, not to every test.
pub(super) struct Resolution {
    pub compiler: ScriptCompiler,
    pub interner: Intern,
    pub ns: SourceDiagnosticSummary,
    pub member: SourceDiagnosticSummary,
    pub ty: SourceDiagnosticSummary,
    pub cn: SourceDiagnosticSummary,
}

impl Resolution {
    /// Total error count across every stage that ran.
    pub(super) fn err_count(&self) -> u32 {
        u32::from(self.ns.err_count())
            + u32::from(self.member.err_count())
            + u32::from(self.ty.err_count())
            + u32::from(self.cn.err_count())
    }

    /// Panics if any stage reported an error, so the caller holds a known-good compiler.
    pub(super) fn expect_ok(self) -> Resolution {
        assert!(
            self.err_count() == 0,
            "Resolution failed:\nnamespace: {:?}\nmember: {:?}\ntype: {:?}\nconstraint: {:?}",
            self.ns,
            self.member,
            self.ty,
            self.cn
        );
        self
    }

    /// Drops the diagnostics, keeping the state tests assert against.
    pub(super) fn into_state(self) -> (ScriptCompiler, Intern) {
        (self.compiler, self.interner)
    }

    /// Constant value of a resolved `let` variable by name.
    pub(super) fn value_of(&self, name: &str) -> Value {
        value_of(&self.compiler, &self.interner, name)
    }
}

/// Runs the stages from `Namespace` up to and including `upto` over the given envs.
///
/// Every module's env is resolved by one resolver instance per stage, because the resolvers
/// are order-locked: constructing the same one twice trips the `ResolverState` assert.
fn run_stages(
    upto: Stage,
    cfg: &mut ChrnConfig,
    interner: &mut Intern,
    compiler: &mut ScriptCompiler,
    reg_envs: &[Option<RegistrationEnv>],
    arena: &Arena<SourceRegion, SourceRegionId>,
    asts: &[Option<AstInfo>],
) -> (
    SourceDiagnosticSummary,
    SourceDiagnosticSummary,
    SourceDiagnosticSummary,
    SourceDiagnosticSummary,
) {
    let mut ns = SourceDiagnosticSummary::default();
    let mut member = SourceDiagnosticSummary::default();
    let mut ty = SourceDiagnosticSummary::default();
    let mut cn = SourceDiagnosticSummary::default();

    let compilation_syms: Vec<Option<Vec<CompilationUnit>>> = {
        let mut ns_resolver = NamespaceResolver::new(cfg, interner, compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            match env {
                Some(env) => {
                    let (syms, diags) = ns_resolver.resolve(env);
                    ns.merge(diags);
                    symbols.push(Some(syms));
                }
                None => symbols.push(None),
            }
        }
        symbols
    };

    if upto == Stage::Namespace {
        return (ns, member, ty, cn);
    }

    let envs = build_resolver_envs(compiler, arena, asts, &compilation_syms);

    {
        let mut member_resolver = MemberResolver::new(cfg, interner, compiler);
        for env in envs.iter().flatten() {
            member.merge(member_resolver.resolve(env));
        }
    }

    if upto == Stage::Member {
        return (ns, member, ty, cn);
    }

    {
        let mut ty_resolver = TypeResolver::new(cfg, interner, compiler);
        for env in envs.iter().flatten() {
            ty.merge(ty_resolver.resolve(env));
        }
    }

    if upto == Stage::Type {
        return (ns, member, ty, cn);
    }

    {
        let mut cn_resolver = ConstraintResolver::new(cfg, interner, compiler);
        for env in envs.iter().flatten() {
            cn.merge(cn_resolver.resolve(env));
        }
    }

    (ns, member, ty, cn)
}

/// Lexes, parses, and resolves a single-module script up to `upto`, collecting diagnostics
/// instead of asserting on them.
pub(super) fn resolve_single_module(text: &str, upto: Stage) -> Resolution {
    let (arena, mut interner, mut cfg, mut compiler) = mock_single_module_compiler(text);

    let asts = build_asts(&arena, &mut cfg, &mut interner, &compiler);
    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let (ns, member, ty, cn) = run_stages(
        upto,
        &mut cfg,
        &mut interner,
        &mut compiler,
        &reg_envs,
        &arena,
        &asts,
    );

    Resolution {
        compiler,
        interner,
        ns,
        member,
        ty,
        cn,
    }
}

/// Same as `resolve_single_module` for a two-module program where `main` imports
/// `sub_module` by path.
pub(super) fn resolve_cross_module(main_text: &str, sub_text: &str, upto: Stage) -> Resolution {
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

    let (arena, _, mut cfg, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let asts = build_asts(&arena, &mut cfg, &mut interner, &compiler);
    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let (ns, member, ty, cn) = run_stages(
        upto,
        &mut cfg,
        &mut interner,
        &mut compiler,
        &reg_envs,
        &arena,
        &asts,
    );

    Resolution {
        compiler,
        interner,
        ns,
        member,
        ty,
        cn,
    }
}

/// Lexes and parses every module that has a region, keeping the result module-aligned.
/// A module without a region (the implicit `core` module) gets a `None` slot.
fn build_asts(
    arena: &Arena<SourceRegion, SourceRegionId>,
    cfg: &mut ChrnConfig,
    interner: &mut Intern,
    compiler: &ScriptCompiler,
) -> Vec<Option<AstInfo>> {
    let mut asts = Vec::new();
    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };

        let toks = Lexer::new(
            region.region_id,
            &region.src_bytes,
            region.script_start,
            cfg,
        )
        .tokenize(interner)
        .toks;

        asts.push(Some(parser::parse(cfg, region, &toks, interner).0));
    }
    asts
}

// -- Stage shorthands --

/// Full pipeline over a single module, panicking on any resolution error so the returned
/// compiler state is known to be fully resolved.
pub(super) fn compile_and_resolve_single_module(text: &str) -> (ScriptCompiler, Intern) {
    resolve_single_module(text, Stage::Constraint)
        .expect_ok()
        .into_state()
}

/// Full pipeline across two modules, panicking on any resolution error.
pub(super) fn compile_and_resolve_cross_module(
    main_text: &str,
    sub_text: &str,
) -> (ScriptCompiler, Intern) {
    resolve_cross_module(main_text, sub_text, Stage::Constraint)
        .expect_ok()
        .into_state()
}

/// Runs up to type resolution and returns the compiler and interner even on error, so
/// callers can inspect the partial resolution state (e.g. verify that variables involved in
/// a circular dependency remain in `ReservedTypeSlot`).
pub(super) fn type_resolve_single_module_keep_state(
    text: &str,
) -> (Result<(), Vec<SourceDiagnostic>>, ScriptCompiler, Intern) {
    let res = resolve_single_module(text, Stage::Type);
    let failed = res.err_count() > 0;
    let Resolution {
        compiler,
        interner,
        ns,
        member,
        ty,
        ..
    } = res;

    let result = if failed {
        let mut diags = ns.diags;
        diags.extend(member.diags);
        diags.extend(ty.diags);
        Err(diags)
    } else {
        Ok(())
    };

    (result, compiler, interner)
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

/// `Value` has no `PartialEq`. Only the constant variants tests assert on are compared, and
/// floats compare by bits so a value that lost precision fails rather than rounding into place.
pub(super) fn values_eq(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::I64(l), Value::I64(r)) => l == r,
        (Value::F64(l), Value::F64(r)) => l.to_bits() == r.to_bits(),
        (Value::Bool(l), Value::Bool(r)) => l == r,
        (Value::Char(l), Value::Char(r)) => l == r,
        (Value::InternedStr(l), Value::InternedStr(r)) => l == r,
        _ => false,
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
