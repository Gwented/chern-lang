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
use chrn_utils::id_types::MemberId;
use compilation::lexer::Lexer;
use compilation::lexer::token::SpannedToken;
use compilation::lexer::token::Token as ScriptToken;
use compilation::lexer::trivia::Trivia;
use compilation::lookup::scopes;
use compilation::lookup::scopes::AssociatedScopeKind;
use compilation::lookup::scopes::ScopeLookupPattern;
use compilation::lookup::scopes::ScopeType;

use compilation::parser::ast::ast_exprs::PathSegment;
use compilation::parser::ast::ast_exprs::TypeExpr;
use compilation::resolvers::constraint_resolver::ConstraintResolver;
use compilation::resolvers::member_resolver::MemberResolver;
use compilation::resolvers::name_resolver::NamespaceResolver;
use compilation::resolvers::resolver_env::{RegistrationEnv, ResolverEnv};
use compilation::resolvers::type_resolver::TypeResolver;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::hir_concepts::MemberSymbolKind;
use compilation::semantic::hir::hir_concepts::SymbolKind;
use compilation::semantic::hir::hir_concepts::SymbolOrigin;
use compilation::semantic::hir::hir_concepts::Type;
use compilation::semantic::hir::hir_concepts::VariableState;
use compilation::semantic::hir::hir_exprs::ExprHir;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use crate::analyser;

use chrn_utils::arena::Arena;
use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::id_types::{
    InternedId, ModuleId, SourceRegionId, SpannedContainer, SymbolId, TypeId,
};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
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
        /// `SymbolId` of the `ConfigDefRoot` this member belongs to.
        cfg_root_sym_id: SymbolId,
        /// `MemberId` of the `ConfigDefMember` itself.
        member_id: MemberId,
    },
    /// An option-assignment key (e.g. `.casing = [...]`) inside a root or member config block.
    ConfigOption {
        /// `SymbolId` of the enclosing `ConfigDefRoot`.
        cfg_root_sym_id: SymbolId,
        /// `MemberId` of the `OptionAssignmentRoot` or `OptionAssignmentMember`.
        member_id: MemberId,
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
    /// Per-module `SymbolId`s produced by the namespace resolver.  Indexed by
    /// `ModuleId`; `None` means that module was skipped (e.g. failed to parse or
    /// had no region).  Consumed by the later resolver stages via `ResolverEnv`.
    pub compilation_syms: Vec<Option<Vec<SymbolId>>>,
    /// Diagnostics from config/import parsing (module discovery phase).
    pub config_errors: Option<Vec<SourceDiagnostic>>,
    /// Diagnostics from the script parser.
    pub parse_errors: Option<Vec<SourceDiagnostic>>,
    /// Diagnostics from namespace resolution.
    pub ns_errors: Option<Vec<SourceDiagnostic>>,
    /// Diagnostics from member (field/variant) resolution.
    pub member_errors: Option<Vec<SourceDiagnostic>>,
    /// Diagnostics from type resolution.
    pub ty_errors: Option<Vec<SourceDiagnostic>>,
    /// Diagnostics from constraint resolution.
    pub cn_errors: Option<Vec<SourceDiagnostic>>,
    /// Sorted list of `(span, entity)` pairs built after analysis; queried by offset.
    pub symbol_map: Vec<(SourceSpan, SemanticEntity)>,
    /// The sub-slice of `compiler.exprs` that belongs to the main module.
    pub main_expr_range: std::ops::Range<usize>,
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
            config_errors: None,
            parse_errors: None,
            ns_errors: None,
            member_errors: None,
            ty_errors: None,
            cn_errors: None,
            symbol_map: Vec::new(),
            main_expr_range: 0..0,
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

        self.config_errors = config_errors;

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
                let (toks, _) =
                    Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                        .tokenize(&mut self.interner);
                compilation::parser::parse(&chrn_cfg, region, &toks, &self.interner)
            };

            let (ast_info, errs) = parse_result;
            let parse_errors = if errs.is_empty() { None } else { Some(errs) };

            if mod_idx == 0 {
                if let Some(diags) = parse_errors {
                    self.parse_errors = Some(diags);
                } else {
                    self.parse_errors = None;
                }
            }

            all_asts[mod_idx] = Some(ast_info);
        }

        // Build the registration environments (pre-symbols) used by the namespace
        // resolver.  Aligned with `compiler.mods` so the resulting `compilation_syms`
        // can be indexed by `ModuleId` when we later build the `ResolverEnv`s.
        let mod_len = compiler.mods.len();
        let mut registration_envs: Vec<Option<RegistrationEnv>> = Vec::with_capacity(mod_len);
        for (mod_idx, ast_info) in all_asts.iter().enumerate().take(mod_len) {
            let ast_info = match ast_info {
                Some(a) => a,
                None => {
                    registration_envs.push(None);
                    continue;
                }
            };
            let src_region_id = match compiler.mods[ModuleId::new(mod_idx as u32)].region_id {
                Some(rid) => rid,
                None => {
                    registration_envs.push(None);
                    continue;
                }
            };
            let region = match self.region_arena.get(src_region_id) {
                Some(r) => r,
                None => {
                    registration_envs.push(None);
                    continue;
                }
            };
            registration_envs.push(Some(RegistrationEnv::new(
                ast_info,
                region,
                ModuleId::new(mod_idx as u32),
            )));
        }

        // Namespace resolution: register every top-level item as a `SymbolId` per
        // module.  Mirrors the orchestrator: a single `NamespaceResolver` is
        // constructed and reused across all modules, accumulating diagnostics and
        // emitting a per-module `Vec<SymbolId>` aligned with `ModuleId`.  This
        // means the later resolver stages no longer need to walk the AST to find
        // their targets — they iterate `compilation_syms` instead.
        let mut compilation_syms: Vec<Option<Vec<SymbolId>>> = Vec::with_capacity(mod_len);
        {
            let mut ns_resolver = NamespaceResolver::new(&chrn_cfg, &self.interner, &mut compiler);

            for (mod_idx, env_opt) in registration_envs.iter().take(mod_len).enumerate() {
                let env = match env_opt {
                    Some(e) => e,
                    None => {
                        compilation_syms.push(None);
                        continue;
                    }
                };

                let (current_mod_symbols, ns_diags) = ns_resolver.resolve(env);

                if !ns_diags.is_empty() {
                    if mod_idx == 0 {
                        self.ns_errors = Some(ns_diags);
                    } else {
                        // Imported modules: extend the existing collection so
                        // their diagnostics still surface in the editor.
                        match &mut self.ns_errors {
                            Some(existing) => existing.extend(ns_diags),
                            None => self.ns_errors = Some(ns_diags),
                        }
                    }
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
            let mod_len = compiler.mods.len();
            let mut resolver_envs: Vec<Option<ResolverEnv>> = Vec::with_capacity(mod_len);
            for (mod_idx, ast_info) in all_asts.iter().enumerate().take(mod_len) {
                let ast_info = match ast_info {
                    Some(a) => a,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                let src_region_id = match compiler.mods[ModuleId::new(mod_idx as u32)].region_id {
                    Some(rid) => rid,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                let region = match self.region_arena.get(src_region_id) {
                    Some(r) => r,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                let mod_syms = match compilation_syms[mod_idx].as_ref() {
                    Some(s) => s,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                resolver_envs.push(Some(ResolverEnv::new(
                    ast_info,
                    region,
                    ModuleId::new(mod_idx as u32),
                    mod_syms,
                )));
            }

            // Member resolution (fields/variants) for all modules.  A single
            // `MemberResolver` is reused across modules and iterates each env's
            // `compilation_syms` internally rather than walking the AST.
            let mut member_resolver = MemberResolver::new(&chrn_cfg, &self.interner, &mut compiler);

            for (mod_idx, env) in resolver_envs.iter().take(mod_len).enumerate() {
                let env = match env {
                    Some(e) => e,
                    None => continue,
                };

                let member_diags = member_resolver.resolve(env);
                if !member_diags.is_empty() {
                    if mod_idx == 0 {
                        self.member_errors = Some(member_diags);
                    } else {
                        match &mut self.member_errors {
                            Some(existing) => existing.extend(member_diags),
                            None => self.member_errors = Some(member_diags),
                        }
                    }
                }
            }

            // Type resolution for all modules. We deliberately do NOT skip the
            // main module when it has parse errors, mirroring the orchestrator's
            // behaviour: every resolver is run to completion so that the parts of
            // the file that did parse correctly still get full semantic analysis
            // (hover, go-to-def, etc.). The resolver itself is tolerant of a
            // partial AST and accumulates diagnostics per item without aborting.
            //
            // Unlike the orchestrator (which keeps a single `TypeResolver` so
            // its internal type context spans all modules) the LSP creates a
            // fresh resolver per module iteration.  This sidesteps the
            // simultaneous `&mut compiler` borrow that the `TypeResolver` holds
            // versus the immutable borrow required to read
            // `compiler.exprs.len()` for `main_expr_range` tracking.  Each
            // module is self-contained for LSP purposes, so losing the
            // cross-module type context is acceptable.
            let mut main_expr_range = 0..0;
            for (mod_idx, env) in resolver_envs.iter().take(mod_len).enumerate() {
                let env = match env {
                    Some(e) => e,
                    None => continue,
                };

                let expr_start = compiler.exprs.len();
                let mut type_resolver = TypeResolver::new(&chrn_cfg, &self.interner, &mut compiler);

                if let Err(ty_diags) = type_resolver.resolve(env)
                    && !ty_diags.is_empty()
                {
                    if mod_idx == 0 {
                        self.ty_errors = Some(ty_diags);
                    } else {
                        // Imported modules: extend the existing collection so
                        // their diagnostics still surface in the editor.
                        match &mut self.ty_errors {
                            Some(existing) => existing.extend(ty_diags),
                            None => self.ty_errors = Some(ty_diags),
                        }
                    }
                }
                let expr_end = compiler.exprs.len();
                if mod_idx == 0 {
                    main_expr_range = expr_start..expr_end;
                }
            }

            self.main_expr_range = main_expr_range;

            // Constraint resolution for all modules. Same rationale as above:
            // do not abort on parse errors, the resolver will skip past
            // unparseable items and produce diagnostics only for the parts that
            // did parse.  A single `ConstraintResolver` is reused.
            let mut constraint_resolver =
                ConstraintResolver::new(&chrn_cfg, &self.interner, &mut compiler);

            for (mod_idx, env) in resolver_envs.iter().take(mod_len).enumerate() {
                let env = match env {
                    Some(env) => env,
                    None => continue,
                };

                if let Err(cn_diags) = constraint_resolver.resolve(env)
                    && !cn_diags.is_empty()
                {
                    if mod_idx == 0 {
                        self.cn_errors = Some(cn_diags);
                    } else {
                        match &mut self.cn_errors {
                            Some(existing) => existing.extend(cn_diags),
                            None => self.cn_errors = Some(cn_diags),
                        }
                    }
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

        // Helper to collect type references from AST
        fn collect_type_refs(
            compiler: &ScriptCompiler,
            type_expr: &SpannedContainer<TypeExpr>,
            map: &mut Vec<(SourceSpan, SemanticEntity)>,
        ) {
            match &type_expr.inner {
                TypeExpr::Var(name_id) => {
                    let interned = *name_id;
                    if let Some(scopes::SymbolLookupOutput {
                        found_sym_id: sym_id,
                        ..
                    }) = scopes::find_sym_id(
                        compiler,
                        AssociatedScopeKind::Module(ModuleId::new(0)),
                        interned,
                        ScopeType::Var,
                        ScopeLookupPattern::NoRestrictions,
                    ) {
                        map.push((type_expr.span, SemanticEntity::Symbol(sym_id)));
                    } else if let Some(scopes::SymbolLookupOutput {
                        found_sym_id: sym_id,
                        ..
                    }) = scopes::find_sym_id(
                        compiler,
                        AssociatedScopeKind::Module(ModuleId::new(0)),
                        interned,
                        ScopeType::Neutral,
                        ScopeLookupPattern::NoRestrictions,
                    ) {
                        map.push((type_expr.span, SemanticEntity::Symbol(sym_id)));
                    }
                }
                TypeExpr::Path(path) => {
                    if path.len() == 2 {
                        let mod_name_part = &path[0];
                        let sym_name_part = &path[1];
                        if let PathSegment::Ident(mod_name_id) = mod_name_part.kind
                            && let Some(found_mod) =
                                compiler.mods.iter().find(|m| m.name_id == mod_name_id)
                        {
                            map.push((
                                mod_name_part.span,
                                SemanticEntity::Module(found_mod.mod_id),
                            ));
                            if let PathSegment::Ident(sym_name_id) = sym_name_part.kind
                                && let Some(scopes::SymbolLookupOutput {
                                    found_sym_id: sym_id,
                                    ..
                                }) = scopes::find_sym_id(
                                    compiler,
                                    AssociatedScopeKind::Module(found_mod.mod_id),
                                    sym_name_id,
                                    ScopeType::Neutral,
                                    ScopeLookupPattern::NamespaceOnly,
                                )
                                .or_else(|| {
                                    scopes::find_sym_id(
                                        compiler,
                                        AssociatedScopeKind::Module(found_mod.mod_id),
                                        sym_name_id,
                                        ScopeType::Var,
                                        ScopeLookupPattern::NamespaceOnly,
                                    )
                                })
                            {
                                map.push((sym_name_part.span, SemanticEntity::Symbol(sym_id)));
                            }
                            return;
                        }
                    }
                    for part in path {
                        if let PathSegment::Generic(generic) = &part.kind {
                            for arg in &generic.inputs {
                                collect_type_refs(compiler, arg, map);
                            }
                        }
                    }
                }
                TypeExpr::Generic(generic) => {
                    for arg in &generic.inputs {
                        collect_type_refs(compiler, arg, map);
                    }
                }
            }
        }

        // Helper to collect expression references from AST
        fn collect_expr_refs(
            compiler: &ScriptCompiler,
            expr: &compilation::parser::ast::ast_exprs::SpannedExpr,
            map: &mut Vec<(SourceSpan, SemanticEntity)>,
            text: &str,
            interner: &Intern,
            script_start: usize,
        ) {
            use compilation::parser::ast::ast_exprs::Expr;
            match &expr.expr {
                Expr::MemberAccess(acc) => {
                    if let Expr::Var(base_id) = acc.base.expr
                        && let Some(found_mod) = compiler.mods.iter().find(|m| m.name_id == base_id)
                    {
                        map.push((acc.base.span, SemanticEntity::Module(found_mod.mod_id)));

                        // Try to find a precise span for the field name by searching after the dot
                        let full_span = expr.span;
                        // The AST's `span` values are relative to the region's
                        // `src_bytes`.  `text` is the whole document, so add
                        // `script_start` to shift the offsets into the
                        // coordinate system the string slice below operates in.
                        let base_end = (acc.base.span.end as usize) + script_start;
                        let full_end = (full_span.end as usize) + script_start;
                        let field_name = interner.search(acc.field);

                        // Look for the field name in the source text between dot and end of expr
                        let mut field_span = SourceSpan {
                            region_id: SourceRegionId::new(0),
                            start: (base_end.saturating_add(1) - script_start) as u32,
                            end: full_span.end,
                        };

                        let search_end = full_end.min(text.len());
                        let search_area = &text[base_end..search_end];
                        if let Some(dot_idx) = search_area.find('.')
                            && let Some(name_idx) = search_area[dot_idx + 1..].find(field_name)
                        {
                            // The search produced a position in absolute
                            // coordinates, so subtract `script_start` to keep
                            // the stored span relative (consistent with the
                            // rest of `symbol_map`).
                            let start = base_end + dot_idx + 1 + name_idx;
                            field_span = SourceSpan {
                                region_id: SourceRegionId::new(0),
                                start: (start - script_start) as u32,
                                end: (start + field_name.len() - script_start) as u32,
                            };
                        }

                        if let Some(scopes::SymbolLookupOutput {
                            found_sym_id: sym_id,
                            ..
                        }) = scopes::find_sym_id(
                            compiler,
                            AssociatedScopeKind::Module(found_mod.mod_id),
                            acc.field,
                            ScopeType::Var,
                            ScopeLookupPattern::NamespaceOnly,
                        )
                        .or_else(|| {
                            scopes::find_sym_id(
                                compiler,
                                AssociatedScopeKind::Module(found_mod.mod_id),
                                acc.field,
                                ScopeType::Neutral,
                                ScopeLookupPattern::NamespaceOnly,
                            )
                        }) {
                            map.push((field_span, SemanticEntity::Symbol(sym_id)));
                        }
                    }
                    collect_expr_refs(compiler, &acc.base, map, text, interner, script_start);
                }
                Expr::Default(_, def_expr) => {
                    collect_expr_refs(compiler, def_expr, map, text, interner, script_start)
                }
                Expr::Call(caller, args) => {
                    collect_expr_refs(compiler, caller, map, text, interner, script_start);
                    for arg in args {
                        collect_expr_refs(compiler, arg, map, text, interner, script_start);
                    }
                }
                Expr::Unary(u) => {
                    collect_expr_refs(compiler, &u.spanned_expr, map, text, interner, script_start)
                }
                Expr::BinaryExpr { lhs, rhs, .. } => {
                    collect_expr_refs(compiler, lhs, map, text, interner, script_start);
                    collect_expr_refs(compiler, rhs, map, text, interner, script_start);
                }
                Expr::StaticAccess(segments) => {
                    if segments.len() >= 2
                        && let PathSegment::Ident(name_id) = segments[0].kind
                    {
                        let sym_id = scopes::find_sym_id(
                            compiler,
                            AssociatedScopeKind::Module(ModuleId::new(0)),
                            name_id,
                            ScopeType::Neutral,
                            ScopeLookupPattern::NoRestrictions,
                        )
                        .or_else(|| {
                            scopes::find_sym_id(
                                compiler,
                                AssociatedScopeKind::Module(ModuleId::new(0)),
                                name_id,
                                ScopeType::Var,
                                ScopeLookupPattern::NoRestrictions,
                            )
                        });

                        if let Some(scopes::SymbolLookupOutput {
                            found_sym_id: sid, ..
                        }) = sym_id
                            && let Some(sym) = compiler.symbols.get(sid)
                        {
                            let mut current_mod: Option<ModuleId> = None;
                            let mut current_ty: Option<TypeId> = None;
                            let mut matched = false;
                            match sym.kind {
                                SymbolKind::Module(mid) => {
                                    map.push((segments[0].span, SemanticEntity::Module(mid)));
                                    current_mod = Some(mid);
                                    matched = true;
                                }
                                SymbolKind::Type(tid) => {
                                    map.push((segments[0].span, SemanticEntity::Symbol(sid)));
                                    current_ty = Some(tid);
                                    matched = true;
                                }
                                SymbolKind::Variable(var_id) => {
                                    let var = &compiler.variables[var_id];
                                    if let VariableState::Known(val_id) = var.state
                                        && let Some(val_info) = compiler.values.get(val_id)
                                    {
                                        map.push((segments[0].span, SemanticEntity::Symbol(sid)));
                                        current_ty = Some(val_info.type_id);
                                        matched = true;
                                    }
                                }
                                _ => {}
                            }
                            if matched {
                                for seg in &segments[1..] {
                                    if let PathSegment::Ident(seg_name_id) = seg.kind {
                                        if let Some(mod_id) = current_mod {
                                            if let Some(scopes::SymbolLookupOutput {
                                                found_sym_id: sym_id,
                                                ..
                                            }) = scopes::find_sym_id(
                                                compiler,
                                                AssociatedScopeKind::Module(mod_id),
                                                seg_name_id,
                                                ScopeType::Var,
                                                ScopeLookupPattern::NamespaceOnly,
                                            )
                                            .or_else(|| {
                                                scopes::find_sym_id(
                                                    compiler,
                                                    AssociatedScopeKind::Module(mod_id),
                                                    seg_name_id,
                                                    ScopeType::Neutral,
                                                    ScopeLookupPattern::NamespaceOnly,
                                                )
                                            }) {
                                                if let Some(sym) = compiler.symbols.get(sym_id) {
                                                    match sym.kind {
                                                        SymbolKind::Module(mid) => {
                                                            map.push((
                                                                seg.span,
                                                                SemanticEntity::Module(mid),
                                                            ));
                                                            current_mod = Some(mid);
                                                            current_ty = None;
                                                        }
                                                        SymbolKind::Type(tid) => {
                                                            map.push((
                                                                seg.span,
                                                                SemanticEntity::Symbol(sym_id),
                                                            ));
                                                            current_mod = None;
                                                            current_ty = Some(tid);
                                                        }
                                                        SymbolKind::Variable(var_id) => {
                                                            let var = &compiler.variables[var_id];
                                                            if let VariableState::Known(val_id) =
                                                                var.state
                                                            {
                                                                map.push((
                                                                    seg.span,
                                                                    SemanticEntity::Symbol(sym_id),
                                                                ));
                                                                current_mod = None;
                                                                current_ty = Some(
                                                                    compiler.values[val_id].type_id,
                                                                );
                                                            }
                                                        }
                                                        _ => {
                                                            map.push((
                                                                seg.span,
                                                                SemanticEntity::Symbol(sym_id),
                                                            ));
                                                            current_mod = None;
                                                            current_ty = None;
                                                        }
                                                    }
                                                } else {
                                                    current_mod = None;
                                                    current_ty = None;
                                                }
                                            } else {
                                                current_mod = None;
                                                current_ty = None;
                                            }
                                        } else if let Some(type_id) = current_ty {
                                            if let Some(ty_info) = compiler.types.get(type_id) {
                                                match &ty_info.ty {
                                                    Type::Struct(sdef) => {
                                                        let field_idx = sdef
                                                            .fields
                                                            .iter()
                                                            .position(|member_id| {
                                                                compiler
                                                                    .members
                                                                    .get(*member_id)
                                                                    .and_then(|m| match m {
                                                                        MemberSymbolKind::Field(
                                                                            f,
                                                                        ) => Some(
                                                                            f.name_id
                                                                                == seg_name_id,
                                                                        ),
                                                                        _ => None,
                                                                    })
                                                                    .unwrap_or(false)
                                                            });
                                                        if let Some(field_idx) = field_idx {
                                                            let member_id = sdef.fields[field_idx];
                                                            let field_type_id = compiler
                                                                .members
                                                                .get(member_id)
                                                                .and_then(|m| match m {
                                                                    MemberSymbolKind::Field(f) => {
                                                                        Some(f.type_id)
                                                                    }
                                                                    _ => None,
                                                                });
                                                            map.push((
                                                                seg.span,
                                                                SemanticEntity::Field {
                                                                    owner_sym_id: sdef.sym_id,
                                                                    field_idx,
                                                                },
                                                            ));
                                                            current_ty = field_type_id;
                                                        } else {
                                                            current_ty = None;
                                                        }
                                                    }
                                                    Type::Enum(edef) => {
                                                        let v_idx = edef
                                                                     .variants
                                                                     .iter()
                                                                     .position(|member_id| {
                                                                         compiler.members
                                                                             .get(*member_id )
                                                                             .and_then(|m| match m {
                                                                                 MemberSymbolKind::Variant(v) => Some(v.name_id == seg_name_id),
                                                                                 _ => None,
                                                                             })
                                                                             .unwrap_or(false)
                                                                     });
                                                        if let Some(v_idx) = v_idx {
                                                            let member_id = edef.variants[v_idx];
                                                            let variant_type_id = compiler
                                                                .members
                                                                .get(member_id)
                                                                .and_then(|m| match m {
                                                                    MemberSymbolKind::Variant(
                                                                        v,
                                                                    ) => v.type_id,
                                                                    _ => None,
                                                                });
                                                            map.push((
                                                                seg.span,
                                                                SemanticEntity::Variant {
                                                                    owner_sym_id: edef.sym_id,
                                                                    variant_idx: v_idx,
                                                                },
                                                            ));
                                                            current_ty = variant_type_id;
                                                        } else {
                                                            current_ty = None;
                                                        }
                                                    }
                                                    _ => {
                                                        current_ty = None;
                                                    }
                                                }
                                            } else {
                                                current_ty = None;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        fn collect_cfg_refs(
            compiler: &ScriptCompiler,
            cfg: &compilation::parser::ast::ast_concepts::AbstractConfig,
            map: &mut Vec<(SourceSpan, SemanticEntity)>,
            text: &str,
            interner: &Intern,
            script_start: usize,
        ) {
            for opt in &cfg.opt_assignments {
                collect_expr_refs(compiler, &opt.array_expr, map, text, interner, script_start);
            }
            for child in &cfg.cfg_members {
                collect_cfg_refs(compiler, child, map, text, interner, script_start);
            }
        }

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
        // `Arena` only implements `Index<I>` and `Index<usize>`, not `Index<Range<usize>>`,
        // so we slice into `compiler.exprs.items` directly.
        for expr in &compiler.exprs.items[self.main_expr_range.clone()] {
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
        for sym in compiler.symbols.iter() {
            if let SymbolKind::Config(cfg_id) = sym.kind {
                if !matches!(sym.sym_origin, SymbolOrigin::Module(mid) if mid.id == 0) {
                    continue;
                }
                let sym_id = sym.sym_id;
                let cfg_root = &compiler.cfgs[cfg_id];

                let mut queue = Vec::new();

                // Root options
                for &member_id in &cfg_root.opt_assignments {
                    if let MemberSymbolKind::OptAssignmentRoot(opt) = &compiler.members[member_id] {
                        map.push((
                            opt.name_span,
                            SemanticEntity::ConfigOption {
                                cfg_root_sym_id: sym_id,
                                member_id,
                            },
                        ));
                    }
                }

                // Root members
                for &member_id in &cfg_root.cfg_members {
                    if let MemberSymbolKind::ConfigDefMember(mem) = &compiler.members[member_id] {
                        map.push((
                            mem.name_span,
                            SemanticEntity::ConfigMember {
                                cfg_root_sym_id: sym_id,
                                member_id,
                            },
                        ));
                        queue.push(member_id);
                    }
                }

                // Traverse nested members
                while let Some(current_member_id) = queue.pop() {
                    if let MemberSymbolKind::ConfigDefMember(mem) =
                        &compiler.members[current_member_id]
                    {
                        for &opt_id in &mem.opt_assignments {
                            if let MemberSymbolKind::OptAssignmentMember(opt) =
                                &compiler.members[opt_id]
                            {
                                map.push((
                                    opt.name_span,
                                    SemanticEntity::ConfigOption {
                                        cfg_root_sym_id: sym_id,
                                        member_id: opt_id,
                                    },
                                ));
                            }
                        }
                        for &child_member_id in &mem.cfg_def_members {
                            if let MemberSymbolKind::ConfigDefMember(child_mem) =
                                &compiler.members[child_member_id]
                            {
                                map.push((
                                    child_mem.name_span,
                                    SemanticEntity::ConfigMember {
                                        cfg_root_sym_id: sym_id,
                                        member_id: child_member_id,
                                    },
                                ));
                                queue.push(child_member_id);
                            }
                        }
                    }
                }
            }
        }

        // 4. Type and Expr References in AST
        if let Some(Some(ast)) = self.asts.first() {
            for item in ast.items() {
                use compilation::parser::ast::ast_concepts::Item;
                match item {
                    Item::Var(v) => {
                        collect_expr_refs(
                            compiler,
                            &v.spanned_expr,
                            &mut map,
                            &self.text,
                            &self.interner,
                            self.script_start,
                        );
                    }
                    Item::TypeDef(def) => {
                        collect_type_refs(compiler, &def.sp_ty_expr, &mut map);
                        for cond in &def.conds {
                            collect_expr_refs(
                                compiler,
                                cond,
                                &mut map,
                                &self.text,
                                &self.interner,
                                self.script_start,
                            );
                        }
                    }
                    Item::Struct(s) => {
                        for cond in &s.glob_conds {
                            collect_expr_refs(
                                compiler,
                                cond,
                                &mut map,
                                &self.text,
                                &self.interner,
                                self.script_start,
                            );
                        }
                        for field in &s.fields {
                            collect_type_refs(compiler, &field.sp_ty_expr, &mut map);
                            for cond in &field.conds {
                                collect_expr_refs(
                                    compiler,
                                    cond,
                                    &mut map,
                                    &self.text,
                                    &self.interner,
                                    self.script_start,
                                );
                            }
                        }
                    }
                    Item::Enum(e) => {
                        for cond in &e.glob_conds {
                            collect_expr_refs(
                                compiler,
                                cond,
                                &mut map,
                                &self.text,
                                &self.interner,
                                self.script_start,
                            );
                        }
                        for variant in &e.variants {
                            if let Some(ty) = &variant.sp_ty_expr {
                                collect_type_refs(compiler, ty, &mut map);
                            }
                            for cond in &variant.conds {
                                collect_expr_refs(
                                    compiler,
                                    cond,
                                    &mut map,
                                    &self.text,
                                    &self.interner,
                                    self.script_start,
                                );
                            }
                        }
                    }
                    Item::Alias(a) => {
                        for cond in &a.conds {
                            collect_expr_refs(
                                compiler,
                                cond,
                                &mut map,
                                &self.text,
                                &self.interner,
                                self.script_start,
                            );
                        }
                    }
                    Item::Config(cfg) => {
                        collect_cfg_refs(
                            compiler,
                            cfg,
                            &mut map,
                            &self.text,
                            &self.interner,
                            self.script_start,
                        );
                    }
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
        // Find the smallest span that contains the offset, as it's the most specific.
        // This prevents broader expressions (like qualified names) from shadowing
        // their more specific components (like the module or field name).
        self.symbol_map
            .iter()
            .filter(|(span, _)| rel_offset >= span.start as usize && rel_offset < span.end as usize)
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
        let mut lsp_diags = Vec::new();
        let doc_len = self.text.len();

        if let Some(diags) = &self.config_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                &self.region_arena,
                &self.text,
                doc_len,
                "chrn-config",
            );
        }
        if let Some(diags) = &self.parse_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                &self.region_arena,
                &self.text,
                doc_len,
                "chrn-parser",
            );
        }
        if let Some(diags) = &self.ns_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                &self.region_arena,
                &self.text,
                doc_len,
                "chrn-namespace",
            );
        }
        if let Some(diags) = &self.member_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                &self.region_arena,
                &self.text,
                doc_len,
                "chrn-member",
            );
        }
        if let Some(diags) = &self.ty_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                &self.region_arena,
                &self.text,
                doc_len,
                "chrn-type",
            );
        }
        if let Some(diags) = &self.cn_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                &self.region_arena,
                &self.text,
                doc_len,
                "chrn-constraint",
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

    /// Resolves a [`SemanticEntity`] to its definition location.
    ///
    /// Returns `(file_path, span, owning_symbol_id)` where:
    /// * `file_path` is the OS path string of the file containing the definition.
    /// * `span` is the byte span of the definition name token within that file.
    /// * `owning_symbol_id` is only meaningful for `Field` and `Variant` variants;
    ///   it identifies the struct/enum that owns the member.
    ///
    /// Returns `None` when the definition cannot be located (e.g. builtin module,
    /// missing AST, or unresolved region).
    pub fn get_definition_location(
        &self,
        entity: &SemanticEntity,
    ) -> Option<(String, SourceSpan, Option<SymbolId>)> {
        let compiler = self.compiler.as_ref()?;
        match entity {
            SemanticEntity::Symbol(sym_id) => {
                let sym = compiler.symbols.get(*sym_id)?;
                // Compiler-origin symbols (directives) have ast_id = None, so
                // they intentionally return no definition location — they are
                // built-in names without a user-visible definition site.
                let ast_id = sym.ast_id?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id as usize,
                    SymbolOrigin::Compiler => 0,
                };
                let ast = self.asts.get(owner_id)?.as_ref()?;
                let span = ast.get_name_span(ast_id);
                let module = compiler.mods.get(ModuleId::new(owner_id as u32))?;
                let region = self.region_arena.get(module.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((path.to_string_lossy().to_string(), span, None))
            }
            SemanticEntity::Field {
                owner_sym_id,
                field_idx,
            } => {
                let sym = compiler.symbols.get(*owner_sym_id)?;
                let ast_id = sym.ast_id?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id as usize,
                    SymbolOrigin::Compiler => 0,
                };
                let ast = self.asts.get(owner_id)?.as_ref()?;
                let abs_struct = ast.get_struct(ast_id);
                let field = abs_struct.fields.get(*field_idx)?;
                let module = compiler.mods.get(ModuleId::new(owner_id as u32))?;
                let region = self.region_arena.get(module.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((
                    path.to_string_lossy().to_string(),
                    field.name_span,
                    Some(*owner_sym_id),
                ))
            }
            SemanticEntity::Variant {
                owner_sym_id,
                variant_idx,
            } => {
                let sym = compiler.symbols.get(*owner_sym_id)?;
                let ast_id = sym.ast_id?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id as usize,
                    SymbolOrigin::Compiler => 0,
                };
                let ast = self.asts.get(owner_id)?.as_ref()?;
                let abs_enum = ast.get_enum(ast_id);
                let variant = abs_enum.variants.get(*variant_idx)?;
                let module = compiler.mods.get(ModuleId::new(owner_id as u32))?;
                let region = self.region_arena.get(module.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((
                    path.to_string_lossy().to_string(),
                    variant.name_span,
                    Some(*owner_sym_id),
                ))
            }
            SemanticEntity::Local {
                decl_span,
                owner_sym_id,
                ..
            } => {
                // For locals, they are defined in module 0 of the current state.
                // `Arena` does not expose a `first()` method, so access the inner
                // `items` vec directly.
                let region = self
                    .region_arena
                    .get(compiler.mods.items.first()?.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((
                    path.to_string_lossy().to_string(),
                    *decl_span,
                    *owner_sym_id,
                ))
            }
            SemanticEntity::Module(mod_id) => {
                let region = self
                    .region_arena
                    .get(compiler.mods.get(*mod_id)?.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((
                    path.to_string_lossy().to_string(),
                    SourceSpan::default(),
                    None,
                ))
            }
            SemanticEntity::ConfigMember {
                cfg_root_sym_id,
                member_id,
            } => {
                // A config member is its own construct; reference it by its own
                // member id rather than by the field/variant it configures.
                let cfg_member = compiler.get_cfg_def_member(*member_id);
                let sym = compiler.symbols.get(*cfg_root_sym_id)?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id as usize,
                    SymbolOrigin::Compiler => 0,
                };
                let module = compiler.mods.get(ModuleId::new(owner_id as u32))?;
                let region = self.region_arena.get(module.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((
                    path.to_string_lossy().to_string(),
                    cfg_member.name_span,
                    Some(*cfg_root_sym_id),
                ))
            }
            SemanticEntity::ConfigOption { .. } => {
                // Schema-defined names have no source declaration to jump to.
                None
            }
        }
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
        def_path: &str,
        def_span: SourceSpan,
        def_owner_sym_id: Option<SymbolId>,
    ) -> Vec<(String, Arc<String>, u32, u32, usize)> {
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
                if let Some((other_def_path, other_def_span, other_def_owner_sym_id)) =
                    state.get_definition_location(ent)
                    && other_def_path == def_path
                    && other_def_span.start == def_span.start
                    && other_def_span.end == def_span.end
                    && other_def_owner_sym_id == def_owner_sym_id
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

type CacheEntry = (
    Arc<String>,
    Arc<RwLock<DocumentState>>,
    Arc<std::sync::atomic::AtomicU64>,
);

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
    tick: std::sync::atomic::AtomicU64,
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
            tick: std::sync::atomic::AtomicU64::new(0),
        }
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
        {
            let cache = self.inner.read();
            if let Some((cached_text, existing, access_tick)) = cache.docs.get(uri)
                && (Arc::ptr_eq(cached_text, &text) || **cached_text == *text)
            {
                access_tick.store(
                    self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                return Arc::clone(existing);
            }
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
        let (tokens, trivia) =
            Lexer::new(SourceRegionId::new(0), script_src, script_start).tokenize(&mut interner);

        // 3. Re-acquire write lock to insert
        let mut cache = self.inner.write();

        // Double check after acquiring write lock in case another thread created it
        if let Some((cached_text, existing, access_tick)) = cache.docs.get(uri)
            && (Arc::ptr_eq(cached_text, &text) || **cached_text == *text)
        {
            access_tick.store(
                self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::sync::atomic::Ordering::Relaxed,
            );
            return Arc::clone(existing);
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
            (
                text,
                Arc::clone(&state),
                Arc::new(std::sync::atomic::AtomicU64::new(
                    self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                )),
            ),
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
        {
            let cache = self.inner.read();
            if let Some((cached_text, existing, access_tick)) = cache.docs.get(uri)
                && (Arc::ptr_eq(cached_text, &text) || **cached_text == *text)
            {
                access_tick.store(
                    self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                return Arc::clone(existing);
            }
        }

        // 2. Insert under write lock
        let mut cache = self.inner.write();

        // Double-check after acquiring write lock
        if let Some((cached_text, existing, access_tick)) = cache.docs.get(uri)
            && (Arc::ptr_eq(cached_text, &text) || **cached_text == *text)
        {
            access_tick.store(
                self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::sync::atomic::Ordering::Relaxed,
            );
            return Arc::clone(existing);
        }

        self.evict_if_needed(&mut cache);

        let state_arc = Arc::new(RwLock::new(state));

        cache.docs.insert(
            uri.to_string(),
            (
                text,
                Arc::clone(&state_arc),
                Arc::new(std::sync::atomic::AtomicU64::new(
                    self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                )),
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
                .map(|(k, (_, _, tick))| {
                    (
                        k.to_string(),
                        tick.load(std::sync::atomic::Ordering::Relaxed),
                    )
                })
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
            tick.store(
                self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::sync::atomic::Ordering::Relaxed,
            );
            Arc::clone(state)
        })
    }

    /// Looks up only the source text for `uri` without acquiring a state lock.
    pub fn get_text(&self, uri: &str) -> Option<Arc<String>> {
        self.inner.read().docs.get(uri).map(|(text, _, tick)| {
            tick.store(
                self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                std::sync::atomic::Ordering::Relaxed,
            );
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
