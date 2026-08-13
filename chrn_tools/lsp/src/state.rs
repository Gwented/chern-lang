//! # state
//!
//! Core document-state types used throughout the LSP.
//!
//! ## [`DocumentState`]
//!
//! Represents the fully-analysed state of a single `.chrn` file.  It is created once
//! per document version by [`DocumentCache::get_or_create`] and then lazily populated
//! by [`DocumentState::ensure_analyzed`], which drives the compiler pipeline:
//!
//! ```text
//! DocumentState::ensure_analyzed
//!     ├─ ModuleFinder::collect_imports        — discover @import statements
//!     ├─ (module resolution runs outside this lock)
//!     ├─ ScriptCompiler::init                 — initialise HIR structures
//!     ├─ parser::parse (per module)           — build AST
//!     ├─ create_registration_envs             — envs pre-symbols
//!     ├─ NamespaceResolver::resolve           — symbol registration (per module)
//!     ├─ create_resolver_envs                 — envs that carry compilation_syms
//!     ├─ MemberResolver::resolve              — field/variant resolution
//!     ├─ TypeResolver::resolve                — type inference & checking
//!     ├─ ConstraintResolver::resolve          — constraint checking
//!     └─ build_symbol_map                     — populate the span → entity index
//! ```
//!
//! ## [`SemanticEntity`]
//!
//! A tagged union that identifies what semantic construct lives at a particular
//! source span.  Used by hover, go-to-definition, references, and rename to dispatch
//! on the kind of thing under the cursor.
//!
//! ## [`DocumentCache`]
//!
//! A thread-safe, bounded LRU-like cache of `DocumentState` values keyed by URI
//! string.  It also maintains a forward (`imports`) and reverse (`dependents`) index
//! of cross-module dependency edges so that editing a shared import file correctly
//! invalidates all documents that import it.

use compilation::lexer::Lexer;
use compilation::lexer::token::SpannedToken;
use compilation::lexer::token::Token as ScriptToken;
use compilation::lexer::trivia::Trivia;
use compilation::lookup::scopes;
use compilation::lookup::scopes::AssociatedScopeKind;
use compilation::lookup::scopes::ScopeLookupPattern;
use compilation::lookup::scopes::ScopeType;

use compilation::parser::ast::ast_concepts::{AbstractDecl, AbstractImpl, AstInfo, Item};
use compilation::parser::ast::ast_exprs::Expr;
use compilation::parser::ast::ast_exprs::PathSegment;
use compilation::parser::ast::ast_exprs::TypeExpr;
use compilation::resolvers::constraint_resolver::ConstraintResolver;
use compilation::resolvers::member_resolver::MemberResolver;
use compilation::resolvers::name_resolver::NamespaceResolver;
use compilation::resolvers::resolver_env::{RegistrationEnv, ResolverEnv};
use compilation::resolvers::type_resolver::TypeResolver;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::compilation_unit::CompilationUnit;
use compilation::semantic::hir::hir_concepts::Type;
use compilation::semantic::hir::hir_exprs::ExprHir;
use compilation::semantic::hir::hir_impls::{ImplHirKind, ImplMemberKind};
use compilation::semantic::hir::hir_symbols::MemberSymbolKind;
use compilation::semantic::hir::hir_symbols::SymbolKind;
use compilation::semantic::hir::hir_symbols::SymbolOrigin;
use compilation::semantic::hir::hir_symbols::VariableState;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::analyser;

use chrn_utils::arena::Arena;
use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::id_types::{
    AstId, ImplId, ImplMemberId, InternedId, ModuleId, SourceRegionId, SpannedContainer, SymbolId,
    TypeId,
};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::SourceDiagnosticSummary;
use chrn_utils::source_map::source_region::SourceRegion;
use chrn_utils::source_map::source_span::SourceSpan;

/// Identifies the semantic construct that occupies a particular source span.
///
/// The `symbol_map` in [`DocumentState`] is a `Vec<(SourceSpan, SemanticEntity)>`.
/// Given a byte offset, the smallest span containing it resolves to one of these
/// variants, which is then used by hover / go-to-definition / references / rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticEntity {
    /// A named symbol (type, variable, module-level function, …).
    Symbol(SymbolId),
    /// A struct field, identified by its owning struct symbol and its positional index.
    Field {
        owner_sym_id: SymbolId,
        field_idx: usize,
    },
    /// An enum variant, identified by its owning enum symbol and its positional index.
    Variant {
        owner_sym_id: SymbolId,
        variant_idx: usize,
    },
    /// An imported or aliased module reference.
    Module(ModuleId),
    /// A local binding (alias type parameter, etc.) scoped to a single declaration.
    Local {
        name_id: InternedId,
        /// The span at which this local name is declared; used as a stable identity key.
        decl_span: SourceSpan,
        /// The symbol that owns this local scope, if any (e.g. the alias symbol).
        owner_sym_id: Option<SymbolId>,
    },
    /// A nested config member block (`.fieldName { }`) inside a `complex->` block.
    /// Resolves to a `ConfigDefMember` whose `linked_member_id` points to the actual field.
    ConfigMember {
        /// `ImplId` of the `ImplHir` (config root) this member belongs to.
        cfg_root_impl_id: ImplId,
        /// `ImplMemberId` of the `ConfigDefMember` itself.
        member_id: ImplMemberId,
    },
    /// An option-assignment key (e.g. `.casing = [...]`) inside a root or member config block.
    ConfigOption {
        /// `ImplId` of the enclosing `ImplHir`.
        cfg_root_impl_id: ImplId,
        /// `ImplMemberId` of the `OptionAssignmentRoot` or `OptionAssignmentMember`.
        member_id: ImplMemberId,
    },
}

/// All analysis results for a single `.chrn` document at a specific version.
///
/// A new `DocumentState` is created by [`DocumentCache::get_or_create`] whenever the
/// document text changes.  At creation time only lexical information (`tokens`,
/// `trivia`) is available.  The remaining fields are populated lazily when
/// [`ensure_analyzed`](DocumentState::ensure_analyzed) is called.
///
/// ## Field lifetime notes
/// * `text` is shared via `Arc` to avoid copying; do not hold references to it across
///   async suspension points if possible.
/// * `compiler`, `asts`, and `symbol_map` are all `None` / empty until
///   `ensure_analyzed` completes.
/// * Error fields (`config_errors`, `parse_errors`, `ns_errors`, `member_errors`, `ty_errors`, `cn_errors`) hold
///   the diagnostics produced by each analysis stage; `None` means that stage either
///   did not run or produced no errors.
pub struct DocumentState {
    /// The raw source text of the document.
    pub text: Arc<String>,
    /// Lexical tokens for the script section (from the lexer).
    pub tokens: Vec<SpannedToken>,
    /// Trivia (comments, whitespace) from lexing; used for `offset_in_comment`.
    pub trivia: Vec<Trivia>,
    /// String/path interner shared by all analysis stages for this document.
    pub interner: Intern,
    /// Arena holding the `SourceRegion` for this file and every imported file.
    pub region_arena: Arena<SourceRegion, SourceRegionId>,
    /// Byte offset of the first token in the script section (`@def`).
    pub script_start: usize,
    /// Byte offset of the serial section start (after `@end`), or `None` if absent.
    pub serial_start: Option<usize>,
    /// The fully initialised script compiler, available after `ensure_analyzed`.
    pub compiler: Option<ScriptCompiler>,
    /// ASTs indexed by module ID; entry `0` is the main module.
    pub asts: Vec<Option<compilation::parser::ast::ast_concepts::AstInfo>>,
    /// Per-module `CompilationUnit`s produced by the namespace resolver.  Indexed by
    /// `ModuleId`; `None` means that module was skipped (e.g. failed to parse or
    /// had no region).  Consumed by the later resolver stages via `ResolverEnv`.
    pub compilation_syms: Vec<Option<Vec<CompilationUnit>>>,
    /// Diagnostics from config/import parsing (module discovery phase).
    pub config_errors: SourceDiagnosticSummary,
    /// Diagnostics from the script parser.
    pub parse_errors: SourceDiagnosticSummary,
    /// Diagnostics from namespace resolution.
    pub ns_errors: SourceDiagnosticSummary,
    /// Diagnostics from member (field/variant) resolution.
    pub member_errors: SourceDiagnosticSummary,
    /// Diagnostics from type resolution.
    pub ty_errors: SourceDiagnosticSummary,
    /// Diagnostics from constraint resolution.
    pub cn_errors: SourceDiagnosticSummary,
    /// `(span, entity)` pairs built after analysis, sorted by `span.start`; queried
    /// by offset through [`get_entity_at_offset`](DocumentState::get_entity_at_offset).
    pub symbol_map: Vec<(SourceSpan, SemanticEntity)>,
    /// Length of the longest span in `symbol_map`.  Bounds how far back
    /// `get_entity_at_offset` has to scan from its binary-search landing point.
    max_span_len: u32,
    /// LSP document version counter (used to detect stale analysis results).
    pub version: u64,
}

impl DocumentState {
    /// Creates a new `DocumentState` pre-populated with lexical data.
    ///
    /// Analysis (`compiler`, `asts`, error fields, `symbol_map`) is left in its
    /// uninitialised state; call [`ensure_analyzed`](Self::ensure_analyzed) to
    /// complete it.
    pub fn new(
        text: Arc<String>,
        tokens: Vec<SpannedToken>,
        trivia: Vec<Trivia>,
        interner: Intern,
        // Byte offset of the first script token.
        script_start: usize,
        // Byte offset of the serial section, or `None`.
        serial_start: Option<usize>,
        version: u64,
    ) -> Self {
        DocumentState {
            text,
            tokens,
            trivia,
            interner,
            region_arena: Arena::new(),
            script_start,
            serial_start,
            compiler: None,
            asts: Vec::new(),
            compilation_syms: Vec::new(),
            config_errors: SourceDiagnosticSummary::default(),
            parse_errors: SourceDiagnosticSummary::default(),
            ns_errors: SourceDiagnosticSummary::default(),
            member_errors: SourceDiagnosticSummary::default(),
            ty_errors: SourceDiagnosticSummary::default(),
            cn_errors: SourceDiagnosticSummary::default(),
            symbol_map: Vec::new(),
            max_span_len: 0,
            version,
        }
    }

    /// Analyze this document and build the compiler.
    ///
    /// Module resolution must already have been performed by
    /// [`crate::analyser::resolve_document_modules`]; this method only runs the
    /// parsing, name-resolution, type-checking, and symbol-map construction phases.
    /// Keeping module resolution outside the write lock eliminates the deadlock that
    /// occurred when the old `ensure_analyzed` held the lock while calling
    /// `DocumentCache::get_text`.
    ///
    /// Returns the list of imported module URI strings (empty if already analyzed).
    pub(crate) fn ensure_analyzed(
        &mut self,
        resolution: analyser::ModuleResolution,
    ) -> Vec<String> {
        if self.compiler.is_some() {
            return Vec::new();
        }

        let chrn_cfg = ChrnConfig::default();
        let analyser::ModuleResolution {
            bind,
            main_region,
            main_mod,
            sub_mods,
            sub_regions,
            config_errors,
            imported_uris,
        } = resolution;

        self.config_errors = config_errors; // now SourceDiagnosticSummary, moved directly

        // Main region has id 0; sub-regions were assigned ids 1..=N during resolution.
        self.region_arena.push(main_region);
        for sub_region in sub_regions {
            self.region_arena.push(sub_region);
        }

        let mut all_mods = Vec::with_capacity(sub_mods.len() + 1);
        all_mods.push(main_mod);
        let mut next_id = 1;
        for mod_opt in sub_mods {
            if let Some(mut inner) = mod_opt {
                if inner.mod_id.id != next_id {
                    inner.mod_id.id = next_id;
                }
                all_mods.push(inner);
            }
            next_id += 1;
        }

        // `ScriptCompiler::init` takes an `Arena<Module, ModuleId>`.  The compiler
        // assigns `ModuleId`s sequentially in push order, so converting from a
        // `Vec<Module>` (via `Arena::from`) preserves the index → id invariant.
        let mut compiler = ScriptCompiler::init(bind, Arena::from(all_mods));

        let mut all_asts = Vec::with_capacity(compiler.mods.len());
        for _ in 0..compiler.mods.len() {
            all_asts.push(None);
        }

        for (mod_idx, module) in compiler.mods.iter().enumerate() {
            let src_region_id = match module.region_id {
                Some(rid) => rid,
                None => continue,
            };
            // The old `extract_region` method was misleadingly named — it
            // simply returned a reference rather than removing anything.  Use
            // `get` to preserve the region in the arena for later stages.
            let region = match self.region_arena.get(src_region_id) {
                Some(r) => r,
                None => continue,
            };

            let parse_result = if mod_idx == 0 {
                // Reuse pre-computed tokens for main module
                compilation::parser::parse(&chrn_cfg, region, &self.tokens, &self.interner)
            } else {
                let lex_output =
                    Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                        .tokenize(&mut self.interner);
                let toks = lex_output.toks;
                compilation::parser::parse(&chrn_cfg, region, &toks, &self.interner)
            };

            let (ast_info, mut errs) = parse_result;

            if mod_idx == 0 {
                self.parse_errors.append_diags(&mut errs.diags);
            }

            all_asts[mod_idx] = Some(ast_info);
        }

        // Build the registration environments (pre-symbols) used by the namespace
        // resolver.  Aligned with `compiler.mods` so the resulting `compilation_syms`
        // can be indexed by `ModuleId` when we later build the `ResolverEnv`s.
        let mod_len = compiler.mods.len();
        let module_inputs = module_inputs(&all_asts, &compiler.mods, &self.region_arena);
        let registration_envs: Vec<Option<RegistrationEnv>> = module_inputs
            .iter()
            .enumerate()
            .map(|(mod_idx, parts)| {
                let (ast_info, region) = (*parts)?;
                Some(RegistrationEnv::new(
                    ast_info,
                    region,
                    ModuleId::new(mod_idx as u32),
                ))
            })
            .collect();

        // Namespace resolution: register every top-level item as a `SymbolId` per
        // module.  Mirrors the orchestrator: a single `NamespaceResolver` is
        // constructed and reused across all modules, accumulating diagnostics and
        // emitting a per-module `Vec<SymbolId>` aligned with `ModuleId`.  This
        // means the later resolver stages no longer need to walk the AST to find
        // their targets — they iterate `compilation_syms` instead.
        let mut compilation_syms: Vec<Option<Vec<CompilationUnit>>> = Vec::with_capacity(mod_len);
        {
            let mut ns_resolver = NamespaceResolver::new(&chrn_cfg, &self.interner, &mut compiler);

            for env_opt in &registration_envs {
                let Some(env) = env_opt else {
                    compilation_syms.push(None);
                    continue;
                };

                let (current_mod_symbols, mut ns_summary) = ns_resolver.resolve(env);

                if !ns_summary.diags.is_empty() {
                    self.ns_errors.append_diags(&mut ns_summary.diags);
                }

                compilation_syms.push(Some(current_mod_symbols));
            }
        }

        // Build resolver environments then run member, type, and constraint resolution.
        // This block ensures `resolver_envs` (which borrows `all_asts`) is dropped before
        // `all_asts` is moved into `self.asts` below.  Each `ResolverEnv` now carries
        // the module's `compilation_syms` slice so the resolvers can iterate over
        // symbols rather than ast nodes.
        {
            let resolver_envs: Vec<Option<ResolverEnv>> = module_inputs
                .iter()
                .enumerate()
                .map(|(mod_idx, parts)| {
                    let (ast_info, region) = (*parts)?;
                    let mod_syms = compilation_syms[mod_idx].as_ref()?;
                    Some(ResolverEnv::new(
                        ast_info,
                        region,
                        ModuleId::new(mod_idx as u32),
                        mod_syms,
                    ))
                })
                .collect();

            // Member resolution (fields/variants) for all modules.  A single
            // `MemberResolver` is reused across modules and iterates each env's
            // `compilation_syms` internally rather than walking the AST.
            let mut member_resolver = MemberResolver::new(&chrn_cfg, &self.interner, &mut compiler);

            for env in resolver_envs.iter().flatten() {
                let mut member_summary = member_resolver.resolve(env);
                if !member_summary.diags.is_empty() {
                    self.member_errors.append_diags(&mut member_summary.diags);
                }
            }

            // Type resolution for all modules. We deliberately do NOT skip the
            // main module when it has parse errors, mirroring the orchestrator's
            // behaviour: every resolver is run to completion so that the parts of
            // the file that did parse correctly still get full semantic analysis
            // (hover, go-to-def, etc.). The resolver itself is tolerant of a
            // partial AST and accumulates diagnostics per item without aborting.
            //
            // A single `TypeResolver` is created for all modules, exactly like
            // the orchestrator's `run_all`.  This matters for three reasons:
            //
            // 1. `TypeResolver::new` debug-asserts `compiler.resolver_state ==
            //    ResolverState::TYPE` and then *advances* the state machine.
            //    Creating one per module used to panic in debug builds on the
            //    second module (state had already advanced to `CONSTRAINT`) and
            //    silently corrupted the state in release builds.
            // 2. The resolver's internal `TypeContext` (pending cross-module
            //    expressions) now spans all modules instead of being discarded
            //    and re-allocated per module.
            // 3. One resolver allocation per analysis instead of one per module.
            //
            // The previous per-module construction existed only so that
            // `compiler.exprs.len()` could be read between iterations to track
            // `main_expr_range`; `build_symbol_map` now filters expressions by
            // the main module's region id instead, which needs no such borrow.
            let mut type_resolver = TypeResolver::new(&chrn_cfg, &mut self.interner, &mut compiler);
            for env in resolver_envs.iter().flatten() {
                let mut ty_summary = type_resolver.resolve(env);
                if !ty_summary.diags.is_empty() {
                    self.ty_errors.append_diags(&mut ty_summary.diags);
                }
            }

            // Constraint resolution for all modules. Same rationale as above:
            // do not abort on parse errors, the resolver will skip past
            // unparseable items and produce diagnostics only for the parts that
            // did parse.  A single `ConstraintResolver` is reused.
            let mut constraint_resolver =
                ConstraintResolver::new(&chrn_cfg, &self.interner, &mut compiler);

            for env in resolver_envs.iter().flatten() {
                let mut cn_summary = constraint_resolver.resolve(env);
                if !cn_summary.diags.is_empty() {
                    self.cn_errors.append_diags(&mut cn_summary.diags);
                }
            }
        }

        self.asts = all_asts;
        self.compilation_syms = compilation_syms;
        self.compiler = Some(compiler);

        self.build_symbol_map();

        imported_uris
    }

    fn build_symbol_map(&mut self) {
        let compiler = match &self.compiler {
            Some(c) => c,
            None => return,
        };

        let mut map = Vec::new();

        // 1. Symbol Definitions
        for (i, sym) in compiler.symbols.iter().enumerate() {
            if matches!(sym.sym_origin, SymbolOrigin::Module(mid) if mid.id == 0)
                || matches!(sym.sym_origin, SymbolOrigin::Compiler)
            {
                let sym_id = SymbolId::new(i as u32);
                if let Some(ast_id) = sym.ast_id
                    && let Some(Some(ast)) = self.asts.first()
                {
                    let span = ast.get_name_span(ast_id);
                    map.push((span, SemanticEntity::Symbol(sym_id)));
                }
            }
        }

        // 2. Variable Usages
        // Only expressions produced from the main module's region are indexed.
        // Filtering by `span.region_id` replaces the old `main_expr_range` slice,
        // which required reading `compiler.exprs.len()` between per-module
        // resolver iterations — the borrow that forced a fresh `TypeResolver`
        // per module (and the resolver-state corruption that came with it).
        let main_region_id = compiler.mods[ModuleId::new(0)].region_id;
        for expr in &compiler.exprs.items {
            if Some(expr.span.region_id) != main_region_id {
                continue;
            }
            if let ExprHir::Var(sym_id) = expr.expr_hir {
                map.push((expr.span, SemanticEntity::Symbol(sym_id)));
            }
        }

        // 3. Field and Variant Definitions
        // `Arena` does not implement `IntoIterator`, so iterate over its inner `items` vec.
        for ty_info in &compiler.types.items {
            if ty_info.owner.id != 0 {
                continue;
            }
            match &ty_info.ty {
                Type::Struct(sdef) => {
                    let sym = &compiler.symbols[sdef.sym_id];
                    if let Some(Some(ast)) = self.asts.first()
                        && let Some(ast_id) = sym.ast_id
                    {
                        let abs_struct = ast.get_struct(ast_id);
                        for (i, field) in abs_struct.fields.iter().enumerate() {
                            map.push((
                                field.name_span,
                                SemanticEntity::Field {
                                    owner_sym_id: sdef.sym_id,
                                    field_idx: i,
                                },
                            ));
                        }
                    }
                }
                Type::Enum(edef) => {
                    let sym = &compiler.symbols[edef.sym_id];
                    if let Some(Some(ast)) = self.asts.first()
                        && let Some(ast_id) = sym.ast_id
                    {
                        let abs_enum = ast.get_enum(ast_id);
                        for (i, variant) in abs_enum.variants.iter().enumerate() {
                            map.push((
                                variant.name_span,
                                SemanticEntity::Variant {
                                    owner_sym_id: edef.sym_id,
                                    variant_idx: i,
                                },
                            ));
                        }
                    }
                }
                Type::Alias(adef) => {
                    let sym = &compiler.symbols[adef.sym_id];
                    if let Some(Some(ast)) = self.asts.first()
                        && let Some(ast_id) = sym.ast_id
                    {
                        let abs_alias = ast.get_alias(ast_id);
                        for (i, _param) in adef.params.iter().enumerate() {
                            if let Some(abs_param) = abs_alias.params.get(i) {
                                map.push((
                                    abs_param.name_span,
                                    SemanticEntity::Local {
                                        name_id: abs_param.name_id,
                                        decl_span: abs_param.name_span,
                                        owner_sym_id: Some(adef.sym_id),
                                    },
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // 3.5. Configuration Definitions
        // Config roots are now stored as Impl compilation units.
        for comp_unit in self
            .compilation_syms
            .first()
            .into_iter()
            .flatten()
            .flatten()
        {
            let impl_id = match comp_unit {
                CompilationUnit::Impl(impl_id) => impl_id,
                _ => continue,
            };
            let impl_hir = &compiler.impls[*impl_id];
            let ImplHirKind::Config(cfg_root_id) = &impl_hir.kind;
            let cfg_root = &compiler.cfgs[*cfg_root_id];

            let mut queue: Vec<ImplMemberId> = Vec::new();

            // Root options
            for &impl_member_id in &cfg_root.opt_assignments {
                if let ImplMemberKind::OptAssignmentRoot(opt) =
                    &compiler.impl_members[impl_member_id]
                {
                    map.push((
                        opt.name_span,
                        SemanticEntity::ConfigOption {
                            cfg_root_impl_id: cfg_root.impl_id,
                            member_id: impl_member_id,
                        },
                    ));
                }
            }

            // Root members
            for &impl_member_id in &cfg_root.cfg_members {
                if let ImplMemberKind::ConfigDefMember(mem) = &compiler.impl_members[impl_member_id]
                {
                    map.push((
                        mem.name_span,
                        SemanticEntity::ConfigMember {
                            cfg_root_impl_id: cfg_root.impl_id,
                            member_id: impl_member_id,
                        },
                    ));
                    queue.push(impl_member_id);
                }
            }

            // Traverse nested members
            while let Some(current_member_id) = queue.pop() {
                if let ImplMemberKind::ConfigDefMember(mem) =
                    &compiler.impl_members[current_member_id]
                {
                    for &opt_id in &mem.opt_assignments {
                        if let ImplMemberKind::OptAssignmentMember(opt) =
                            &compiler.impl_members[opt_id]
                        {
                            map.push((
                                opt.name_span,
                                SemanticEntity::ConfigOption {
                                    cfg_root_impl_id: cfg_root.impl_id,
                                    member_id: opt_id,
                                },
                            ));
                        }
                    }
                    for &child_member_id in &mem.cfg_def_members {
                        if let ImplMemberKind::ConfigDefMember(child_mem) =
                            &compiler.impl_members[child_member_id]
                        {
                            map.push((
                                child_mem.name_span,
                                SemanticEntity::ConfigMember {
                                    cfg_root_impl_id: cfg_root.impl_id,
                                    member_id: child_member_id,
                                },
                            ));
                            queue.push(child_member_id);
                        }
                    }
                }
            }
        }

        // 4. Type and Expr References in AST
        if let Some(Some(ast)) = self.asts.first() {
            let mut collector = RefCollector::new(
                compiler,
                &self.text,
                &self.interner,
                self.script_start,
                &mut map,
            );
            for item in ast.items() {
                match item {
                    Item::Decl(AbstractDecl::Var(v)) => collector.expr_refs(&v.spanned_expr),
                    Item::Decl(AbstractDecl::TypeDef(def)) => {
                        collector.type_refs(&def.sp_ty_expr);
                        for cond in &def.conds {
                            collector.expr_refs(cond);
                        }
                    }
                    Item::Decl(AbstractDecl::Struct(s)) => {
                        for cond in &s.glob_conds {
                            collector.expr_refs(cond);
                        }
                        for field in &s.fields {
                            collector.type_refs(&field.sp_ty_expr);
                            for cond in &field.conds {
                                collector.expr_refs(cond);
                            }
                        }
                    }
                    Item::Decl(AbstractDecl::Enum(e)) => {
                        for cond in &e.glob_conds {
                            collector.expr_refs(cond);
                        }
                        for variant in &e.variants {
                            if let Some(ty) = &variant.sp_ty_expr {
                                collector.type_refs(ty);
                            }
                            for cond in &variant.conds {
                                collector.expr_refs(cond);
                            }
                        }
                    }
                    Item::Decl(AbstractDecl::Alias(a)) => {
                        for cond in &a.conds {
                            collector.expr_refs(cond);
                        }
                    }
                    Item::Impl(AbstractImpl::Config(cfg)) => collector.cfg_refs(cfg),
                }
            }
        }

        // 5. Compiler-origin symbols (directives, etc.) — match by name against Id tokens
        // Pre-index compiler-origin symbols by name_id for O(1) lookup.
        let directive_symbols: HashMap<u32, SymbolId> = compiler
            .symbols
            .iter()
            .filter(|sym| sym.ast_id.is_none() && matches!(sym.sym_origin, SymbolOrigin::Compiler))
            .map(|sym| (sym.name_id.id, sym.sym_id))
            .collect();

        // Track spans already in `map` to avoid shadowing user-defined symbols
        // with same name as a directive (e.g. `let warn = 5`).
        let covered_starts: HashSet<u32> = map.iter().map(|(s, _)| s.start).collect();

        for st in &self.tokens {
            if let ScriptToken::Id(id) = st.tok {
                if covered_starts.contains(&st.span.start) {
                    continue;
                }
                if let Some(&sym_id) = directive_symbols.get(&id.id) {
                    map.push((st.span, SemanticEntity::Symbol(sym_id)));
                }
            }
        }

        self.set_symbol_map(map);
    }

    /// Replaces the symbol map, restoring the ordering and span-length bound that
    /// [`get_entity_at_offset`](Self::get_entity_at_offset) depends on.
    ///
    /// Assign through this rather than writing `symbol_map` directly; that field
    /// is public for iteration (references, rename) only.
    pub fn set_symbol_map(&mut self, mut map: Vec<(SourceSpan, SemanticEntity)>) {
        // Sorting by start offset lets the lookup binary-search instead of
        // scanning the whole map; the longest span bounds how far back from the
        // landing point a containing span can begin.
        map.sort_unstable_by_key(|(span, _)| (span.start, span.end));
        self.max_span_len = map
            .iter()
            .map(|(span, _)| span.end.saturating_sub(span.start))
            .max()
            .unwrap_or(0);
        self.symbol_map = map;
    }

    /// Returns the most specific [`SemanticEntity`] whose span contains `offset`.
    ///
    /// "Most specific" is defined as the entry with the smallest span length.  This
    /// prevents a broader expression span (e.g. a qualified path `mod::Field`) from
    /// shadowing the individual component (e.g. the module name or the field name)
    /// when the cursor is on that component.
    ///
    /// `offset` is an **absolute** byte offset in the document (e.g. derived from
    /// an LSP `Position`). The method internally subtracts `script_start` to convert
    /// it to a relative offset that matches the spans stored in `symbol_map`.
    pub fn get_entity_at_offset(&self, offset: usize) -> Option<&SemanticEntity> {
        let rel_offset = offset.saturating_sub(self.script_start);

        // `symbol_map` is sorted by `span.start`, so every span that can contain
        // `rel_offset` sits at or before the first entry starting after it, and no
        // further back than `max_span_len` bytes.  This bounds the scan to the
        // spans actually overlapping the cursor; it used to walk the entire map on
        // every call, which the semantic-tokens pass does once per identifier
        // token — quadratic in file size.
        let upper = self
            .symbol_map
            .partition_point(|(span, _)| (span.start as usize) <= rel_offset);
        let lowest_start = rel_offset.saturating_sub(self.max_span_len as usize);

        // Find the smallest span that contains the offset, as it's the most specific.
        // This prevents broader expressions (like qualified names) from shadowing
        // their more specific components (like the module or field name).
        self.symbol_map[..upper]
            .iter()
            .rev()
            .take_while(|(span, _)| (span.start as usize) >= lowest_start)
            .filter(|(span, _)| rel_offset < span.end as usize)
            .min_by_key(|(span, _)| span.end.saturating_sub(span.start))
            .map(|(_, entity)| entity)
    }

    /// Collects all error phases into a flat LSP diagnostic list.
    ///
    /// Phases are emitted in order: config errors → parse errors → namespace
    /// errors → type errors.  Each phase uses the appropriate `source` tag so that
    /// editors can filter by category:
    ///
    /// | Phase           | `source` tag        |
    /// |-----------------|---------------------|
    /// | Config / import | `"chrn-config"`     |
    /// | Parser          | `"chrn-parser"`     |
    /// | Namespace       | `"chrn-namespace"`  |
    /// | Member          | `"chrn-member"`     |
    /// | Type checker    | `"chrn-type"`       |
    pub fn get_lsp_diagnostics(&self) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let stages: [(&SourceDiagnosticSummary, &str); 6] = [
            (&self.config_errors, "chrn-config"),
            (&self.parse_errors, "chrn-parser"),
            (&self.ns_errors, "chrn-namespace"),
            (&self.member_errors, "chrn-member"),
            (&self.ty_errors, "chrn-type"),
            (&self.cn_errors, "chrn-constraint"),
        ];

        let mut lsp_diags = Vec::new();
        let doc_len = self.text.len();
        for (summary, source) in stages {
            analyser::push_diagnostic(
                &mut lsp_diags,
                summary.diags(),
                &self.region_arena,
                &self.text,
                doc_len,
                source,
            );
        }
        lsp_diags
    }

    /// Returns the interned ID, start byte, and end byte of the identifier token
    /// that covers `byte_offset`, or `None` if no identifier token is at that offset.
    ///
    /// `byte_offset` is an **absolute** byte offset in the document. The returned
    /// `start` and `end` byte positions are also **absolute** in the document, so
    /// callers can pass them directly to [`crate::text::offset_to_position`].
    /// This is what LSP feature handlers want: absolute positions are immediately
    /// usable as LSP `Position`s.
    pub fn get_symbol_at_offset(&self, byte_offset: usize) -> Option<(InternedId, usize, usize)> {
        self.get_token_at_offset(byte_offset).and_then(|st| {
            if let ScriptToken::Id(id) = st.tok {
                Some((
                    id,
                    crate::text::rel_to_abs_offset(st.span.start, self.script_start) as usize,
                    crate::text::rel_to_abs_offset(st.span.end, self.script_start) as usize,
                ))
            } else {
                None
            }
        })
    }

    /// Returns the token that covers `byte_offset`, or `None` if no token is at that offset.
    ///
    /// `byte_offset` is an **absolute** byte offset in the document. The method
    /// internally subtracts `script_start` to convert it to the relative offset
    /// against which the token spans are stored.
    pub fn get_token_at_offset(&self, byte_offset: usize) -> Option<&SpannedToken> {
        let rel_offset = byte_offset.saturating_sub(self.script_start);
        let idx = self
            .tokens
            .partition_point(|t| (t.span.end as usize) <= rel_offset);
        if idx < self.tokens.len() {
            let t = &self.tokens[idx];
            if rel_offset >= t.span.start as usize && rel_offset < t.span.end as usize {
                return Some(t);
            }
        }
        None
    }

    /// Convenience wrapper around [`get_symbol_at_offset`](Self::get_symbol_at_offset)
    /// that returns the identifier text as a `String`.
    pub fn get_identifier_at_offset(&self, byte_offset: usize) -> Option<String> {
        self.get_symbol_at_offset(byte_offset)
            .map(|(id, _, _)| self.interner.search(id).to_string())
    }

    /// Resolves a symbol to the AST it was declared in, its declaration node, and
    /// the path of the file that AST came from.
    ///
    /// Compiler-origin symbols (directives) have `ast_id = None` and return `None`:
    /// they are built-in names without a user-visible definition site.
    fn symbol_site(&self, sym_id: SymbolId) -> Option<(&AstInfo, AstId, &Path)> {
        let compiler = self.compiler.as_ref()?;
        let sym = compiler.symbols.get(sym_id)?;
        let ast_id = sym.ast_id?;
        let owner_id = match sym.sym_origin {
            SymbolOrigin::Module(mid) => mid.id as usize,
            SymbolOrigin::Compiler => 0,
        };
        let ast = self.asts.get(owner_id)?.as_ref()?;
        let module = compiler.mods.get(ModuleId::new(owner_id as u32))?;
        let region = self.region_arena.get(module.region_id?)?;
        Some((ast, ast_id, self.interner.search_path(region.path_id)))
    }

    /// Path of the file backing `mod_id`'s source region.
    fn module_path(&self, mod_id: ModuleId) -> Option<&Path> {
        let compiler = self.compiler.as_ref()?;
        let region = self
            .region_arena
            .get(compiler.mods.get(mod_id)?.region_id?)?;
        Some(self.interner.search_path(region.path_id))
    }

    /// Resolves a [`SemanticEntity`] to its definition site, borrowing the path
    /// out of the interner instead of allocating it.
    ///
    /// Returns `(file_path, span, owning_symbol_id)` where:
    /// * `file_path` is the path of the file containing the definition.
    /// * `span` is the byte span of the definition name token within that file.
    /// * `owning_symbol_id` is only meaningful for `Field` and `Variant` variants;
    ///   it identifies the struct/enum that owns the member.
    ///
    /// Returns `None` when the definition cannot be located (e.g. builtin module,
    /// missing AST, or unresolved region).
    pub fn definition_site(
        &self,
        entity: &SemanticEntity,
    ) -> Option<(&Path, SourceSpan, Option<SymbolId>)> {
        match entity {
            SemanticEntity::Symbol(sym_id) => {
                let (ast, ast_id, path) = self.symbol_site(*sym_id)?;
                Some((path, ast.get_name_span(ast_id), None))
            }
            SemanticEntity::Field {
                owner_sym_id,
                field_idx,
            } => {
                let (ast, ast_id, path) = self.symbol_site(*owner_sym_id)?;
                let field = ast.get_struct(ast_id).fields.get(*field_idx)?;
                Some((path, field.name_span, Some(*owner_sym_id)))
            }
            SemanticEntity::Variant {
                owner_sym_id,
                variant_idx,
            } => {
                let (ast, ast_id, path) = self.symbol_site(*owner_sym_id)?;
                let variant = ast.get_enum(ast_id).variants.get(*variant_idx)?;
                Some((path, variant.name_span, Some(*owner_sym_id)))
            }
            // Locals and configs are always declared in the main module.
            SemanticEntity::Local {
                decl_span,
                owner_sym_id,
                ..
            } => Some((
                self.module_path(ModuleId::new(0))?,
                *decl_span,
                *owner_sym_id,
            )),
            SemanticEntity::ConfigMember { member_id, .. } => {
                let compiler = self.compiler.as_ref()?;
                let name_span = compiler.get_cfg_def_member(*member_id).name_span;
                Some((self.module_path(ModuleId::new(0))?, name_span, None))
            }
            SemanticEntity::Module(mod_id) => {
                Some((self.module_path(*mod_id)?, SourceSpan::default(), None))
            }
            // Schema-defined names have no source declaration to jump to.
            SemanticEntity::ConfigOption { .. } => None,
        }
    }

    /// Owning [`String`] form of [`definition_site`](Self::definition_site), for
    /// callers that need the path beyond the borrow of `self`.
    pub fn get_definition_location(
        &self,
        entity: &SemanticEntity,
    ) -> Option<(String, SourceSpan, Option<SymbolId>)> {
        self.definition_site(entity)
            .map(|(path, span, owner)| (path.to_string_lossy().into_owned(), span, owner))
    }

    /// Check if a given byte offset falls within a comment (single or multi).
    /// Uses binary search for O(log n) performance.
    /// Also checks for single-line comments by looking for // before the cursor on the current line.
    /// Finds all symbol-map entries across every cached document that share the same
    /// definition key `(def_path, def_span, def_owner_sym_id)`.  Used by references
    /// and rename to implement cross-module search without duplicating the iteration
    /// logic.
    ///
    /// Returns `(state_uri, text_arc, span_start, span_end, script_start)` tuples
    /// so callers can convert byte offsets to LSP positions without re‑acquiring
    /// the document state.  `script_start` is included because the spans returned
    /// by the symbol map are **relative** to the region's `src_bytes`; callers
    /// add `script_start` to obtain absolute file coordinates.
    pub fn find_matching_entities(
        doc_cache: &DocumentCache,
        def_path: &Path,
        def_span: SourceSpan,
        def_owner_sym_id: Option<SymbolId>,
    ) -> Vec<EntityOccurrence> {
        // Collect state Arcs while holding the cache lock, then release it before
        // acquiring any DocumentState locks.  This prevents the lock-order inversion
        // that contributed to the deadlock: a reader holding DocumentCache while
        // waiting for DocumentState, while analysis holds DocumentState and waits for
        // DocumentCache.
        let mut states: Vec<(String, Arc<RwLock<DocumentState>>)> = Vec::new();
        doc_cache.for_each_state(|state_uri, state_arc| {
            states.push((state_uri.to_string(), Arc::clone(&state_arc)));
        });

        let mut results = Vec::new();
        for (state_uri, state_arc) in states {
            let state = state_arc.read();
            for (span, ent) in &state.symbol_map {
                // `definition_site` walks arenas and, for members, the owning AST
                // node.  The owner check reads the entity's own fields, so it
                // rejects whole categories of entry before any of that runs.
                if !entity_may_define(ent, def_span, def_owner_sym_id) {
                    continue;
                }
                // `definition_site` borrows the path out of the interner, so this
                // runs allocation-free — one `String` per symbol-map entry per
                // cached document used to be built here just to be compared away.
                // The two integer fields are checked first because they reject
                // almost every entry before the path comparison is reached.
                if let Some((other_path, other_span, other_owner)) = state.definition_site(ent)
                    && other_span.start == def_span.start
                    && other_span.end == def_span.end
                    && other_owner == def_owner_sym_id
                    && other_path == def_path
                {
                    results.push((
                        state_uri.clone(),
                        Arc::clone(&state.text),
                        span.start,
                        span.end,
                        state.script_start,
                    ));
                }
            }
        }
        results
    }

    pub fn offset_in_comment(&self, byte_offset: usize) -> bool {
        // Trivia spans are relative to the region's `src_bytes`, so convert the
        // absolute byte offset to a relative one before comparing.
        let rel_offset = byte_offset.saturating_sub(self.script_start);

        let idx = self
            .trivia
            .partition_point(|t| t.span.start as usize <= rel_offset);
        if idx > 0 {
            let t = &self.trivia[idx - 1];
            if rel_offset < t.span.end as usize && t.kind.is_comment() {
                return true;
            }
        }

        let text = self.text.as_bytes();
        if byte_offset >= text.len() {
            return false;
        }

        // The `//` line-comment check operates on the absolute document text so
        // the same line is examined regardless of where the script section starts.
        let line_start = text[..byte_offset]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|p| p + 1)
            .unwrap_or(0);

        if text[line_start..byte_offset].windows(2).any(|w| w == b"//") {
            return true;
        }

        false
    }
}

/// One occurrence of a symbol found by [`DocumentState::find_matching_entities`]:
/// `(uri, file text, span start, span end, script start)`.
///
/// The span endpoints are **relative** to the region's `src_bytes`; `script_start`
/// shifts them into the absolute file coordinates an LSP position needs.
pub type EntityOccurrence = (String, Arc<String>, u32, u32, usize);

/// Pairs each module with the AST and source region its resolvers read, in `ModuleId`
/// order, with `None` where either is missing.
///
/// Both the registration environments and the resolver environments need exactly this
/// lookup; each used to walk the module list and unwrap the same three `Option`s
/// itself, in twenty-odd lines apiece.
fn module_inputs<'a>(
    asts: &'a [Option<AstInfo>],
    mods: &Arena<compilation::modules::Module, ModuleId>,
    regions: &'a Arena<SourceRegion, SourceRegionId>,
) -> Vec<Option<(&'a AstInfo, &'a SourceRegion)>> {
    (0..mods.len())
        .map(|mod_idx| {
            let ast_info = asts.get(mod_idx)?.as_ref()?;
            let region_id = mods[ModuleId::new(mod_idx as u32)].region_id?;
            Some((ast_info, regions.get(region_id)?))
        })
        .collect()
}

/// Whether `entity` could possibly resolve to the definition keyed by `def_span` and
/// `def_owner_sym_id`, judged from the entity's own fields alone.
///
/// A prefilter for [`DocumentState::find_matching_entities`]: the owning symbol that
/// [`DocumentState::definition_site`] reports is fixed per entity kind, so a mismatch
/// here rules the entry out without touching the compiler arenas.  Returning `true` is
/// not a match — the full `definition_site` comparison still decides.
fn entity_may_define(
    entity: &SemanticEntity,
    def_span: SourceSpan,
    def_owner_sym_id: Option<SymbolId>,
) -> bool {
    match entity {
        // These report no owning symbol, so they can only key a definition that has none.
        SemanticEntity::Symbol(_)
        | SemanticEntity::Module(_)
        | SemanticEntity::ConfigMember { .. } => def_owner_sym_id.is_none(),
        SemanticEntity::Field { owner_sym_id, .. }
        | SemanticEntity::Variant { owner_sym_id, .. } => def_owner_sym_id == Some(*owner_sym_id),
        SemanticEntity::Local {
            decl_span,
            owner_sym_id,
            ..
        } => *decl_span == def_span && *owner_sym_id == def_owner_sym_id,
        // Schema-defined names have no definition site at all.
        SemanticEntity::ConfigOption { .. } => false,
    }
}

/// `(source text, analysis state, access tick)`.
///
/// The tick is a plain `AtomicU64` rather than an `Arc<AtomicU64>`: it is only
/// ever touched through a borrow of the entry, so the extra allocation and
/// refcount bought nothing.
type CacheEntry = (Arc<String>, Arc<RwLock<DocumentState>>, AtomicU64);

/// Internal storage for [`DocumentCache`].
struct CacheInner {
    /// Primary document map: URI → (source text, analysis state, access_tick).
    docs: HashMap<String, CacheEntry>,
    /// URI → set of module URIs it imports (forward dependency edges).
    imports: HashMap<String, HashSet<String>>,
    /// URI → set of URIs that import it (reverse dependency index).
    dependents: HashMap<String, HashSet<String>>,
}

/// Thread-safe, bounded cache of analysed document states.
///
/// ## Caching strategy
/// * Documents are keyed by their URI string.
/// * The cache stores both the source text (`Arc<String>`) and the
///   [`DocumentState`] wrapped in a `RwLock`.
/// * Tokenisation (via `Lexer`) happens **outside** any lock to avoid blocking
///   readers while lexing a large file.
/// * When the cache exceeds `max_size` a simple eviction strategy removes the
///   oldest entries in HashMap iteration order (not strictly LRU, but sufficient
///   for typical usage where a few dozen files are open at once).
///
/// ## Dependency tracking
/// The cache maintains two complementary maps:
/// * `imports`: for each URI, the set of URIs it imports.
/// * `dependents`: the reverse index — for each URI, the set of URIs that import it.
///
/// When a document is saved or changed, [`invalidate`](Self::invalidate) performs a
/// BFS over `dependents` to evict all transitive dependents, ensuring that stale
/// analysis results are never served.
pub struct DocumentCache {
    inner: RwLock<CacheInner>,
    max_size: usize,
    tick: AtomicU64,
}

impl std::fmt::Debug for DocumentCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentCache")
            .field("max_size", &self.max_size)
            .finish()
    }
}

impl DocumentCache {
    /// Creates a new cache with the given maximum number of documents.
    pub fn new(max_size: usize) -> Self {
        DocumentCache {
            inner: RwLock::new(CacheInner {
                docs: HashMap::new(),
                imports: HashMap::new(),
                dependents: HashMap::new(),
            }),
            max_size,
            tick: AtomicU64::new(0),
        }
    }

    /// Returns the cached state for `uri` when `text` matches what is stored,
    /// marking the entry as most recently used.
    ///
    /// Shared by the read-lock fast path and the write-lock double check of both
    /// [`get_or_create`](Self::get_or_create) and [`insert_or_get`](Self::insert_or_get),
    /// which each used to spell the same comparison out by hand.
    fn hit(
        &self,
        cache: &CacheInner,
        uri: &str,
        text: &Arc<String>,
    ) -> Option<Arc<RwLock<DocumentState>>> {
        let (cached_text, existing, access_tick) = cache.docs.get(uri)?;
        if !Arc::ptr_eq(cached_text, text) && **cached_text != **text {
            return None;
        }
        access_tick.store(self.next_tick(), Ordering::Relaxed);
        Some(Arc::clone(existing))
    }

    /// Allocates the next monotonic access tick used for LRU ordering.
    fn next_tick(&self) -> u64 {
        self.tick.fetch_add(1, Ordering::Relaxed)
    }

    /// Returns an existing [`DocumentState`] for `uri` when the source text is
    /// unchanged, or creates a new one by tokenising `text`.
    ///
    /// The double-checked locking pattern is used: a read lock is acquired first to
    /// avoid the cost of a write lock on cache hits.  The expensive tokenisation step
    /// runs without holding any lock, and the write lock is acquired only to insert.
    ///
    /// If the cache is at capacity, one entry is evicted before the new one is inserted.
    pub fn get_or_create(
        &self,
        uri: &str,
        text: Arc<String>,
        script_start: usize,
        serial_start: Option<usize>,
        version: u64,
    ) -> Arc<RwLock<DocumentState>> {
        // 1. Check existing under read lock first (cheap)
        if let Some(existing) = self.hit(&self.inner.read(), uri, &text) {
            return existing;
        }

        // 2. Perform expensive tokenization OUTSIDE any cache lock.
        //
        // The lexer is given the *relative* script section bytes (sliced out
        // of the full document by `[script_start..]`) and the *absolute*
        // `script_start`.  Token and trivia spans come back relative to the
        // script section, which is the same contract `resolve_document_modules`
        // uses on the production path.  Without this slice the lexer would
        // see the full text and emit spans in the document's absolute
        // coordinate system, which every downstream consumer
        // (`get_token_at_offset`, `offset_in_comment`, `find_matching_entities`,
        // `hover`, `references`, `rename`) treats as relative.
        let mut interner = Intern::init();
        let script_src = &text.as_bytes()[script_start..];
        let lex_output =
            Lexer::new(SourceRegionId::new(0), script_src, script_start).tokenize(&mut interner);
        let tokens = lex_output.toks;
        let trivia = lex_output.trivia;

        // 3. Re-acquire write lock to insert
        let mut cache = self.inner.write();

        // Double check after acquiring write lock in case another thread created it
        if let Some(existing) = self.hit(&cache, uri, &text) {
            return existing;
        }

        self.evict_if_needed(&mut cache);

        let state = Arc::new(RwLock::new(DocumentState::new(
            Arc::clone(&text),
            tokens,
            trivia,
            interner,
            script_start,
            serial_start,
            version,
        )));

        cache.docs.insert(
            uri.to_string(),
            (text, Arc::clone(&state), AtomicU64::new(self.next_tick())),
        );
        state
    }

    /// Inserts a pre-built [`DocumentState`] for `uri`, or returns the existing
    /// cached state if the source text matches.
    ///
    /// This is used by the analysis pipeline after module resolution has been
    /// performed outside of any `DocumentState` lock.  The cache hit check is
    /// identical to [`get_or_create`](Self::get_or_create).
    pub fn insert_or_get(
        &self,
        uri: &str,
        text: Arc<String>,
        state: DocumentState,
    ) -> Arc<RwLock<DocumentState>> {
        // 1. Fast path: exact text already cached
        if let Some(existing) = self.hit(&self.inner.read(), uri, &text) {
            return existing;
        }

        // 2. Insert under write lock
        let mut cache = self.inner.write();

        // Double-check after acquiring write lock
        if let Some(existing) = self.hit(&cache, uri, &text) {
            return existing;
        }

        self.evict_if_needed(&mut cache);

        let state_arc = Arc::new(RwLock::new(state));

        cache.docs.insert(
            uri.to_string(),
            (
                text,
                Arc::clone(&state_arc),
                AtomicU64::new(self.next_tick()),
            ),
        );

        state_arc
    }

    /// Evicts the least-recently-used entries when the cache is at capacity.
    fn evict_if_needed(&self, cache: &mut CacheInner) {
        if cache.docs.len() >= self.max_size {
            let to_remove = cache.docs.len() - self.max_size + 1;
            let mut entries: Vec<_> = cache
                .docs
                .iter()
                .map(|(k, (_, _, tick))| (k.clone(), tick.load(Ordering::Relaxed)))
                .collect();
            entries.sort_unstable_by_key(|(_, t)| *t);
            let keys_to_remove: Vec<String> = entries
                .into_iter()
                .take(to_remove)
                .map(|(k, _)| k)
                .collect();
            for key in &keys_to_remove {
                cache.docs.remove(key);
                if let Some(imports) = cache.imports.remove(key) {
                    for imp in imports {
                        if let Some(dep_set) = cache.dependents.get_mut(&imp) {
                            dep_set.remove(key);
                        }
                    }
                }
                cache.dependents.remove(key);
            }
        }
    }

    /// Register the set of module URIs that `uri` imports.
    /// Updates the reverse `dependents` index accordingly.
    pub fn register_dependencies(&self, uri: &str, imported_uris: &[String]) {
        let mut cache = self.inner.write();

        // Remove old reverse entries for previous imports of this URI
        if let Some(old_imports) = cache.imports.remove(uri) {
            for old_dep in &old_imports {
                if let Some(dep_set) = cache.dependents.get_mut(old_dep.as_str()) {
                    dep_set.remove(uri);
                }
            }
        }

        // Insert new imports and update reverse index
        let new_imports: HashSet<String> = imported_uris.iter().map(|s| s.to_string()).collect();
        for dep_uri in &new_imports {
            cache
                .dependents
                .entry(dep_uri.to_string())
                .or_default()
                .insert(uri.to_string());
        }
        cache.imports.insert(uri.to_string(), new_imports);
    }

    /// Invalidate a document and all transitive dependents (BFS).
    pub fn invalidate(&self, uri: &str) {
        let mut cache = self.inner.write();
        let mut worklist = VecDeque::new();
        worklist.push_back(uri.to_string());

        while let Some(current) = worklist.pop_front() {
            cache.docs.remove(&current);

            if let Some(deps) = cache.dependents.get(&current) {
                for dep in deps {
                    if cache.docs.contains_key(dep.as_str()) {
                        worklist.push_back(dep.to_string());
                    }
                }
            }

            cache.dependents.remove(&current);
            if let Some(imports) = cache.imports.remove(&current) {
                for imp in imports {
                    if let Some(dep_set) = cache.dependents.get_mut(&imp) {
                        dep_set.remove(&current);
                    }
                }
            }
        }
    }

    /// Looks up the [`DocumentState`] for `uri`, returning `None` if not cached.
    pub fn get(&self, uri: &str) -> Option<Arc<RwLock<DocumentState>>> {
        self.inner.read().docs.get(uri).map(|(_, state, tick)| {
            tick.store(self.next_tick(), Ordering::Relaxed);
            Arc::clone(state)
        })
    }

    /// Looks up only the source text for `uri` without acquiring a state lock.
    pub fn get_text(&self, uri: &str) -> Option<Arc<String>> {
        self.inner.read().docs.get(uri).map(|(text, _, tick)| {
            tick.store(self.next_tick(), Ordering::Relaxed);
            Arc::clone(text)
        })
    }

    /// Calls `f` with the URI and state for every cached document.
    ///
    /// The entire cache is read-locked for the duration of the iteration; `f` must
    /// not call other `DocumentCache` methods to avoid deadlock.
    pub fn for_each_state<F>(&self, mut f: F)
    where
        F: FnMut(&str, Arc<RwLock<DocumentState>>),
    {
        let cache = self.inner.read();
        for (uri, (_, state, _)) in &cache.docs {
            f(uri, Arc::clone(state));
        }
    }

    /// Removes all documents, imports, and dependents from the cache.
    pub fn clear(&self) {
        let mut cache = self.inner.write();
        cache.docs.clear();
        cache.imports.clear();
        cache.dependents.clear();
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new(50)
    }
}

/// Read-only context shared by the AST reference walks that populate
/// [`DocumentState::symbol_map`].
///
/// The walks are mutually recursive and each needed the compiler, the document
/// text, the interner, the script start, and the output map; threading five
/// parameters through every call site (and every recursive step) made the call
/// sites longer than the logic. Bundling them here also documents that
/// everything except `map` is read-only.
struct RefCollector<'a> {
    compiler: &'a ScriptCompiler,
    /// Whole-document text.  Spans in the AST are relative to the region's
    /// `src_bytes`, so `script_start` must be added before slicing into this.
    text: &'a str,
    interner: &'a Intern,
    script_start: usize,
    /// Module name → id, built once.  The walks resolve a module by name at every
    /// path and member access; a linear scan of `compiler.mods` per node made that
    /// cost grow with the import count on every expression in the file.
    mods_by_name: HashMap<InternedId, ModuleId>,
    map: &'a mut Vec<(SourceSpan, SemanticEntity)>,
}

/// What resolving one segment of a `::` path leaves the walk pointing at.
///
/// The path walk is a state machine over these three cases; before, the same
/// transitions were written twice — once for the head segment and once, nested five
/// blocks deep, for the tail — with `current_mod` / `current_ty` / `matched` locals
/// standing in for the state.
enum PathCursor {
    /// The path so far names a module; the next segment is looked up in it.
    Module(ModuleId),
    /// The path so far names a value or type; the next segment is a field/variant.
    Type(TypeId),
    /// Resolved to something that cannot own further segments, or failed to
    /// resolve.  Remaining segments are not indexed.
    Opaque,
}

impl<'a> RefCollector<'a> {
    fn new(
        compiler: &'a ScriptCompiler,
        text: &'a str,
        interner: &'a Intern,
        script_start: usize,
        map: &'a mut Vec<(SourceSpan, SemanticEntity)>,
    ) -> Self {
        // First module wins on a duplicate name, matching the `find` this replaces.
        let mut mods_by_name = HashMap::with_capacity(compiler.mods.len());
        for module in &compiler.mods.items {
            mods_by_name.entry(module.name_id).or_insert(module.mod_id);
        }
        RefCollector {
            compiler,
            text,
            interner,
            script_start,
            mods_by_name,
            map,
        }
    }

    /// Looks up a module by the name it is referred to under.
    fn module_named(&self, name_id: InternedId) -> Option<ModuleId> {
        self.mods_by_name.get(&name_id).copied()
    }

    /// Resolves `name_id` in `mod_id`'s exported namespace, trying each scope in
    /// `order` until one hits.
    ///
    /// Every path walk needs the same two-scope fallback and differs only in which
    /// scope it prefers, which is why the order is a parameter rather than fixed.
    fn lookup_in_module(
        &self,
        mod_id: ModuleId,
        name_id: InternedId,
        order: [ScopeType; 2],
    ) -> Option<SymbolId> {
        self.lookup(mod_id, name_id, order, ScopeLookupPattern::NamespaceOnly)
    }

    /// Resolves `name_id` as it would be written in the main module, following
    /// imports and enclosing scopes.
    fn lookup_visible(&self, name_id: InternedId, order: [ScopeType; 2]) -> Option<SymbolId> {
        self.lookup(
            ModuleId::new(0),
            name_id,
            order,
            ScopeLookupPattern::NoRestrictions,
        )
    }

    fn lookup(
        &self,
        mod_id: ModuleId,
        name_id: InternedId,
        order: [ScopeType; 2],
        pattern: ScopeLookupPattern,
    ) -> Option<SymbolId> {
        order.into_iter().find_map(|scope_ty| {
            scopes::find_sym_id(
                self.compiler,
                AssociatedScopeKind::Module(mod_id),
                name_id,
                scope_ty,
                pattern,
            )
            .map(|out| out.found_sym_id)
        })
    }
}

impl RefCollector<'_> {
    /// Indexes the symbols named by a type expression (and its generic arguments).
    fn type_refs(&mut self, type_expr: &SpannedContainer<TypeExpr>) {
        match &type_expr.inner {
            TypeExpr::Var(name_id) => {
                if let Some(sym_id) =
                    self.lookup_visible(*name_id, [ScopeType::Var, ScopeType::Neutral])
                {
                    self.map
                        .push((type_expr.span, SemanticEntity::Symbol(sym_id)));
                }
            }
            TypeExpr::Path(path) => {
                if path.len() == 2 {
                    let mod_name_part = &path[0];
                    let sym_name_part = &path[1];
                    if let PathSegment::Ident(mod_name_id) = mod_name_part.kind
                        && let Some(found_mod) = self.module_named(mod_name_id)
                    {
                        self.map
                            .push((mod_name_part.span, SemanticEntity::Module(found_mod)));
                        if let PathSegment::Ident(sym_name_id) = sym_name_part.kind
                            && let Some(sym_id) = self.lookup_in_module(
                                found_mod,
                                sym_name_id,
                                [ScopeType::Neutral, ScopeType::Var],
                            )
                        {
                            self.map
                                .push((sym_name_part.span, SemanticEntity::Symbol(sym_id)));
                        }
                        return;
                    }
                }
                for part in path {
                    if let PathSegment::Generic(generic) = &part.kind {
                        for arg in &generic.inputs {
                            self.type_refs(arg);
                        }
                    }
                }
            }
            TypeExpr::Generic(generic) => {
                for arg in &generic.inputs {
                    self.type_refs(arg);
                }
            }
        }
    }

    /// Locates the field name inside `base.field`, given the relative end offsets of
    /// the base expression and of the whole access.
    ///
    /// The AST records no span for the name itself, so it is found in the source text
    /// after the dot.  Falls back to "everything after the base" when the text does
    /// not contain the expected shape.  The returned span is relative to the region's
    /// `src_bytes`, like the rest of `symbol_map`; `self.text` is the whole document,
    /// so `script_start` shifts between the two.
    fn member_name_span(&self, base_end: u32, access_end: u32, field: InternedId) -> SourceSpan {
        let fallback = SourceSpan {
            region_id: SourceRegionId::new(0),
            start: base_end.saturating_add(1),
            end: access_end,
        };

        let search_start = base_end as usize + self.script_start;
        let search_end = (access_end as usize + self.script_start).min(self.text.len());
        let Some(search_area) = self.text.get(search_start..search_end) else {
            return fallback;
        };

        let field_name = self.interner.search(field);
        let Some(dot_idx) = search_area.find('.') else {
            return fallback;
        };
        let Some(name_idx) = search_area[dot_idx + 1..].find(field_name) else {
            return fallback;
        };

        let start = search_start + dot_idx + 1 + name_idx;
        SourceSpan {
            region_id: SourceRegionId::new(0),
            start: (start - self.script_start) as u32,
            end: (start + field_name.len() - self.script_start) as u32,
        }
    }

    /// Indexes the symbols, modules, fields, and variants named by an expression.
    fn expr_refs(&mut self, expr: &compilation::parser::ast::ast_exprs::SpannedExpr) {
        match &expr.expr {
            AstExprMemberAccess(acc) => {
                if let AstExprVar(base_id) = acc.base.expr
                    && let Some(found_mod) = self.module_named(base_id)
                {
                    self.map
                        .push((acc.base.span, SemanticEntity::Module(found_mod)));

                    let field_span = self.member_name_span(acc.base.span.end, expr.span.end, acc.field);
                    if let Some(sym_id) = self.lookup_in_module(
                        found_mod,
                        acc.field,
                        [ScopeType::Var, ScopeType::Neutral],
                    ) {
                        self.map.push((field_span, SemanticEntity::Symbol(sym_id)));
                    }
                }
                self.expr_refs(&acc.base);
            }
            AstExprDefault(_, def_expr) => self.expr_refs(def_expr),
            AstExprCall(caller, args) => {
                self.expr_refs(caller);
                for arg in args {
                    self.expr_refs(arg);
                }
            }
            AstExprUnary(u) => self.expr_refs(&u.spanned_expr),
            AstExprBinaryExpr { lhs, rhs, .. } => {
                self.expr_refs(lhs);
                self.expr_refs(rhs);
            }
            AstExprStaticAccess(segments) => self.static_access_refs(segments),
            _ => {}
        }
    }

    /// Indexes every segment of a `a::b::c` path, walking left to right and
    /// resolving each segment against what the previous one named.
    fn static_access_refs(
        &mut self,
        segments: &[compilation::parser::ast::ast_exprs::SpannedPathSegment],
    ) {
        if segments.len() < 2 {
            return;
        }
        let PathSegment::Ident(head_name) = segments[0].kind else {
            return;
        };
        let Some(mut cursor) = self.static_access_head(segments[0].span, head_name) else {
            return;
        };

        for seg in &segments[1..] {
            let PathSegment::Ident(name_id) = seg.kind else {
                continue;
            };
            cursor = match cursor {
                PathCursor::Module(mod_id) => self.segment_in_module(seg.span, name_id, mod_id),
                PathCursor::Type(type_id) => self.segment_in_type(seg.span, name_id, type_id),
                PathCursor::Opaque => PathCursor::Opaque,
            };
        }
    }

    /// Resolves the leading segment of a path against the main module's scope.
    ///
    /// Returns `None` — indexing nothing at all, including the later segments —
    /// when the name resolves to something a path cannot start with.
    fn static_access_head(&mut self, span: SourceSpan, name_id: InternedId) -> Option<PathCursor> {
        let compiler = self.compiler;
        let sym_id = self.lookup_visible(name_id, [ScopeType::Neutral, ScopeType::Var])?;
        let sym = compiler.symbols.get(sym_id)?;

        match sym.kind {
            SymbolKind::Namespace => Some(self.push_namespace(span, sym_id, sym)),
            SymbolKind::Type(type_id) => {
                self.map.push((span, SemanticEntity::Symbol(sym_id)));
                Some(PathCursor::Type(type_id))
            }
            SymbolKind::Variable(var_id) => {
                let type_id = self.variable_type(var_id)?;
                self.map.push((span, SemanticEntity::Symbol(sym_id)));
                Some(PathCursor::Type(type_id))
            }
            SymbolKind::Directive(_) => None,
        }
    }

    /// Resolves one segment inside the module the path has reached.
    fn segment_in_module(
        &mut self,
        span: SourceSpan,
        name_id: InternedId,
        mod_id: ModuleId,
    ) -> PathCursor {
        let compiler = self.compiler;
        let Some(sym_id) =
            self.lookup_in_module(mod_id, name_id, [ScopeType::Var, ScopeType::Neutral])
        else {
            return PathCursor::Opaque;
        };
        let Some(sym) = compiler.symbols.get(sym_id) else {
            return PathCursor::Opaque;
        };

        match sym.kind {
            SymbolKind::Namespace => self.push_namespace(span, sym_id, sym),
            SymbolKind::Type(type_id) => {
                self.map.push((span, SemanticEntity::Symbol(sym_id)));
                PathCursor::Type(type_id)
            }
            SymbolKind::Variable(var_id) => match self.variable_type(var_id) {
                Some(type_id) => {
                    self.map.push((span, SemanticEntity::Symbol(sym_id)));
                    PathCursor::Type(type_id)
                }
                // A variable with no resolved value ends the walk.  The old code
                // left the module cursor in place here, so the following segment
                // was looked up in the module as though the variable segment had
                // not been written.
                None => PathCursor::Opaque,
            },
            _ => {
                self.map.push((span, SemanticEntity::Symbol(sym_id)));
                PathCursor::Opaque
            }
        }
    }

    /// Resolves one segment as a field or variant of the type the path has reached.
    fn segment_in_type(
        &mut self,
        span: SourceSpan,
        name_id: InternedId,
        type_id: TypeId,
    ) -> PathCursor {
        let compiler = self.compiler;
        let Some(ty_info) = compiler.types.get(type_id) else {
            return PathCursor::Opaque;
        };

        let (entity, member_type_id) = match &ty_info.ty {
            Type::Struct(sdef) => {
                let Some(field_idx) = sdef.fields.iter().position(|member_id| {
                    matches!(
                        compiler.sym_members.get(*member_id),
                        Some(MemberSymbolKind::Field(f)) if f.name_id == name_id
                    )
                }) else {
                    return PathCursor::Opaque;
                };
                let member_type_id = match compiler.sym_members.get(sdef.fields[field_idx]) {
                    Some(MemberSymbolKind::Field(f)) => Some(f.type_id),
                    _ => None,
                };
                (
                    SemanticEntity::Field {
                        owner_sym_id: sdef.sym_id,
                        field_idx,
                    },
                    member_type_id,
                )
            }
            Type::Enum(edef) => {
                let Some(variant_idx) = edef.variants.iter().position(|member_id| {
                    matches!(
                        compiler.sym_members.get(*member_id),
                        Some(MemberSymbolKind::Variant(v)) if v.name_id == name_id
                    )
                }) else {
                    return PathCursor::Opaque;
                };
                let member_type_id = match compiler.sym_members.get(edef.variants[variant_idx]) {
                    Some(MemberSymbolKind::Variant(v)) => v.type_id,
                    _ => None,
                };
                (
                    SemanticEntity::Variant {
                        owner_sym_id: edef.sym_id,
                        variant_idx,
                    },
                    member_type_id,
                )
            }
            _ => return PathCursor::Opaque,
        };

        self.map.push((span, entity));
        match member_type_id {
            Some(type_id) => PathCursor::Type(type_id),
            None => PathCursor::Opaque,
        }
    }

    /// Indexes a namespace segment, which names either a module or a plain scope.
    fn push_namespace(
        &mut self,
        span: SourceSpan,
        sym_id: SymbolId,
        sym: &compilation::semantic::hir::hir_symbols::Symbol,
    ) -> PathCursor {
        match sym
            .associated_scope
            .expect("Namespace should have associated scope")
        {
            AssociatedScopeKind::Module(mod_id) => {
                self.map.push((span, SemanticEntity::Module(mod_id)));
                PathCursor::Module(mod_id)
            }
            AssociatedScopeKind::Scope(_) => {
                self.map.push((span, SemanticEntity::Symbol(sym_id)));
                PathCursor::Opaque
            }
        }
    }

    /// The type of a variable whose value is already known, if it has one.
    fn variable_type(&self, var_id: chrn_utils::id_types::VariableId) -> Option<TypeId> {
        let VariableState::Known(val_id) = self.compiler.variables[var_id].state else {
            return None;
        };
        Some(self.compiler.values.get(val_id)?.type_id)
    }

    /// Walks a `complex->` config block and its nested members.
    fn cfg_refs(&mut self, cfg: &compilation::parser::ast::ast_concepts::AbstractConfig) {
        use compilation::parser::ast::ast_concepts::AbstractConfigKind;

        if let AbstractConfigKind::Root(sp_ty) = &cfg.kind {
            self.type_refs(sp_ty);
        }
        for opt in &cfg.opt_assignments {
            self.expr_refs(&opt.array_expr);
        }
        for child in &cfg.cfg_members {
            self.cfg_refs(child);
        }
    }
}
