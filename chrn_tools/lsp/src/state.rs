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
//!     ├─ analyser::resolve_modules_lsp        — load & recurse into imported files
//!     ├─ ScriptCompiler::init                 — initialise HIR structures
//!     ├─ parser::parse (per module)           — build AST
//!     ├─ NamespaceResolver::resolve           — symbol registration
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
use compilation::lookup::scopes::LookupPattern;
use compilation::lookup::scopes::ScopeType;
use compilation::modules::Module;
use compilation::parser::ast::ast_exprs::PathSegment;
use compilation::parser::ast::ast_exprs::TypeExpr;
use compilation::resolvers::constraint_resolver::ConstraintResolver;
use compilation::resolvers::member_resolver::MemberResolver;
use compilation::resolvers::name_resolver::NamespaceResolver;
use compilation::resolvers::resolver_env::ResolverEnv;
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
use std::path::Path;
use std::sync::Arc;

use crate::analyser;

use chrn_utils::chrn_settings::ChrnSettings;
use chrn_utils::id_types::{
    InternedId, ModuleId, PathId, SourceRegionId, SpannedContainer, SymbolId, TypeId,
};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
use chrn_utils::source_map::source_region::{SourceRegion, SourceRegionArena};
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
    pub region_arena: SourceRegionArena,
    /// Byte offset of the first token in the script section (`@def`).
    pub script_start: usize,
    /// Byte offset of the serial section start (after `@end`), or `None` if absent.
    pub serial_start: Option<usize>,
    /// The fully initialised script compiler, available after `ensure_analyzed`.
    pub compiler: Option<ScriptCompiler>,
    /// ASTs indexed by module ID; entry `0` is the main module.
    pub asts: Vec<Option<compilation::parser::ast::ast_concepts::AstInfo>>,
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
            region_arena: SourceRegionArena::new(Default::default()),
            script_start,
            serial_start,
            compiler: None,
            asts: Vec::new(),
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

    /// Analyze this document, resolving modules and building the compiler.
    /// Returns the list of imported module URI strings (empty if already analyzed).
    pub fn ensure_analyzed(&mut self, doc_cache: &DocumentCache, path: &Path) -> Vec<String> {
        if self.compiler.is_some() {
            return Vec::new();
        }

        let settings = ChrnSettings::default();
        let path_buf = path.to_path_buf();

        let name = path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("<unnamed>")
            .to_string();
        let name_id = self.interner.intern(&name);
        let path_id = self.interner.intern_path(&path_buf);

        let main_region = SourceRegion::new(
            self.text.as_bytes().to_vec(),
            SourceRegionId::new(0),
            path_id,
            self.script_start,
            self.serial_start,
        );

        let mut reserved_mod_ids: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];

        let (bind, main_imports, finder_diags) =
            compilation::modules::mod_finder::ModuleFinder::new(
                self.text.as_bytes(),
                &settings,
                &mut reserved_mod_ids,
                &main_region,
                self.script_start,
                self.serial_start,
            )
            .collect_imports(&mut self.interner);

        if !finder_diags.is_empty() {
            self.config_errors = Some(finder_diags);
        }

        let main_region_id = SourceRegionId::new(self.region_arena.regions.len() as u32);
        self.region_arena.regions.push(main_region);

        let main_mod = Module::new(
            name_id,
            compilation::modules::ModuleState::Loading,
            ModuleId::new(0),
            main_imports,
            Some(main_region_id),
        );

        let mut seen: Vec<PathId> = vec![path_id];

        let mut other_mods = Vec::with_capacity(main_mod.imports.len());
        let mut sub_diags = Vec::new();
        analyser::resolve_modules_lsp(
            &mut reserved_mod_ids,
            &mut seen,
            &mut other_mods,
            &main_mod,
            &settings,
            &mut self.interner,
            doc_cache,
            &mut self.region_arena,
            &mut sub_diags,
        );
        if !sub_diags.is_empty() {
            if let Some(existing) = &mut self.config_errors {
                existing.append(&mut sub_diags);
            } else {
                self.config_errors = Some(sub_diags);
            }
        }

        // Collect imported module URIs for dependency tracking
        let imported_uris: Vec<String> = other_mods
            .iter()
            .filter_map(|mod_opt| {
                let m = mod_opt.as_ref()?;
                let region_id = m.region_id?;
                let region = self.region_arena.get_region(region_id)?;
                let p = self.interner.search_path(region.path_id);
                tower_lsp::lsp_types::Url::from_file_path(p)
                    .ok()
                    .map(|u| u.to_string())
            })
            .collect();

        let mut all_mods = Vec::with_capacity(other_mods.len() + 1);
        all_mods.push(main_mod);
        let mut next_id = 1;
        for mod_opt in other_mods.drain(..) {
            if let Some(mut inner) = mod_opt {
                if inner.mod_id.id != next_id {
                    inner.mod_id.id = next_id;
                }
                all_mods.push(inner);
            }
            next_id += 1;
        }

        let mut compiler = ScriptCompiler::init(bind, all_mods);

        let mut all_asts = Vec::with_capacity(compiler.mods.len());
        for _ in 0..compiler.mods.len() {
            all_asts.push(None);
        }

        for (mod_idx, module) in compiler.mods.iter().enumerate() {
            let src_region_id = match module.region_id {
                Some(rid) => rid,
                None => continue,
            };
            let region = self.region_arena.extract_region(src_region_id);

            let parse_result = if mod_idx == 0 {
                // Reuse pre-computed tokens for main module
                compilation::parser::parse(&settings, region, &self.tokens, &self.interner)
            } else {
                let (toks, _) =
                    Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                        .tokenize(&mut self.interner);
                compilation::parser::parse(&settings, region, &toks, &self.interner)
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

        // Namespace resolution for all modules
        for (mod_idx, ast_info) in all_asts.iter().enumerate() {
            let ast_info = match ast_info {
                Some(a) => a,
                None => continue,
            };

            let src_region_id = match compiler.mods[mod_idx].region_id {
                Some(rid) => rid,
                None => continue,
            };
            let region = match self.region_arena.get_region(src_region_id) {
                Some(r) => r,
                None => continue,
            };

            let env = ResolverEnv::new(ast_info, region, ModuleId::new(mod_idx));
            let mut ns_resolver = NamespaceResolver::new(&settings, &self.interner, &mut compiler);

            if let Err(ns_diags) = ns_resolver.resolve(&env)
                && mod_idx == 0
            {
                self.ns_errors = Some(ns_diags);
            }
        }

        // Build resolver environments then run member, type, and constraint resolution.
        // This block ensures resolver_envs (which borrows all_asts) is dropped before
        // all_asts is moved into self.asts below.
        {
            let mod_len = compiler.mods.len();
            let mut resolver_envs = Vec::with_capacity(mod_len);
            for (mod_idx, ast_info) in all_asts.iter().enumerate().take(mod_len) {
                let ast_info = match ast_info {
                    Some(a) => a,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                let src_region_id = match compiler.mods[mod_idx].region_id {
                    Some(rid) => rid,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                let region = match self.region_arena.get_region(src_region_id) {
                    Some(r) => r,
                    None => {
                        resolver_envs.push(None);
                        continue;
                    }
                };
                resolver_envs.push(Some(ResolverEnv::new(
                    ast_info,
                    region,
                    ModuleId::new(mod_idx),
                )));
            }

            // Member resolution (fields/variants) for all modules
            let member_diags =
                MemberResolver::new(&settings, &resolver_envs, &self.interner, &mut compiler)
                    .resolve();
            if !member_diags.is_empty() {
                self.member_errors = Some(member_diags);
            }

            if self.parse_errors.is_none() {
                let mut main_expr_range = 0..0;
                for (mod_idx, env) in resolver_envs.iter().enumerate().take(mod_len) {
                    let env = match env {
                        Some(e) => e,
                        None => continue,
                    };

                    let expr_start = compiler.exprs.len();
                    let mut type_resolver =
                        TypeResolver::new(&settings, &self.interner, &mut compiler);

                    if let Err(ty_diags) = type_resolver.resolve(env)
                        && mod_idx == 0
                    {
                        self.ty_errors = Some(ty_diags);
                    }
                    let expr_end = compiler.exprs.len();
                    if mod_idx == 0 {
                        main_expr_range = expr_start..expr_end;
                    }
                }

                self.main_expr_range = main_expr_range;

                // Constraint resolution for all modules
                let mut constraint_resolver =
                    ConstraintResolver::new(&settings, &self.interner, &mut compiler);

                for (mod_idx, env) in resolver_envs.iter().enumerate().take(mod_len) {
                    let env = match env {
                        Some(env) => env,
                        None => continue,
                    };

                    if let Err(cn_diags) = constraint_resolver.resolve(env)
                        && mod_idx == 0
                    {
                        self.cn_errors = Some(cn_diags);
                    }
                }
            }
        }

        self.asts = all_asts;
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
                        LookupPattern::NoRestrictions,
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
                        LookupPattern::NoRestrictions,
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
                                    LookupPattern::NamespaceOnly,
                                )
                                .or_else(|| {
                                    scopes::find_sym_id(
                                        compiler,
                                        AssociatedScopeKind::Module(found_mod.mod_id),
                                        sym_name_id,
                                        ScopeType::Var,
                                        LookupPattern::NamespaceOnly,
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
                        let base_end = acc.base.span.end as usize;
                        let field_name = interner.search(acc.field);

                        // Look for the field name in the source text between dot and end of expr
                        let mut field_span = SourceSpan {
                            region_id: SourceRegionId::new(0),
                            start: base_end.saturating_add(1) as u32,
                            end: full_span.end,
                        };

                        let search_area = &text[base_end..(full_span.end as usize).min(text.len())];
                        if let Some(dot_idx) = search_area.find('.')
                            && let Some(name_idx) = search_area[dot_idx + 1..].find(field_name)
                        {
                            let start = base_end + dot_idx + 1 + name_idx;
                            field_span = SourceSpan {
                                region_id: SourceRegionId::new(0),
                                start: start as u32,
                                end: (start + field_name.len()) as u32,
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
                            LookupPattern::NamespaceOnly,
                        )
                        .or_else(|| {
                            scopes::find_sym_id(
                                compiler,
                                AssociatedScopeKind::Module(found_mod.mod_id),
                                acc.field,
                                ScopeType::Neutral,
                                LookupPattern::NamespaceOnly,
                            )
                        }) {
                            map.push((field_span, SemanticEntity::Symbol(sym_id)));
                        }
                    }
                    collect_expr_refs(compiler, &acc.base, map, text, interner);
                }
                Expr::Default(_, def_expr) => {
                    collect_expr_refs(compiler, def_expr, map, text, interner)
                }
                Expr::Call(caller, args) => {
                    collect_expr_refs(compiler, caller, map, text, interner);
                    for arg in args {
                        collect_expr_refs(compiler, arg, map, text, interner);
                    }
                }
                Expr::Unary(u) => collect_expr_refs(compiler, &u.spanned_expr, map, text, interner),
                Expr::BinaryExpr { lhs, rhs, .. } => {
                    collect_expr_refs(compiler, lhs, map, text, interner);
                    collect_expr_refs(compiler, rhs, map, text, interner);
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
                            LookupPattern::NoRestrictions,
                        )
                        .or_else(|| {
                            scopes::find_sym_id(
                                compiler,
                                AssociatedScopeKind::Module(ModuleId::new(0)),
                                name_id,
                                ScopeType::Var,
                                LookupPattern::NoRestrictions,
                            )
                        });

                        if let Some(scopes::SymbolLookupOutput {
                            found_sym_id: sid, ..
                        }) = sym_id
                            && let Some(sym) = compiler.symbols.get(sid.id as usize)
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
                                    let var = &compiler.variables[var_id.id as usize];
                                    if let VariableState::Known(val_id) = var.state
                                        && let Some(val_info) =
                                            compiler.values.get(val_id.id as usize)
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
                                                LookupPattern::NamespaceOnly,
                                            )
                                            .or_else(|| {
                                                scopes::find_sym_id(
                                                    compiler,
                                                    AssociatedScopeKind::Module(mod_id),
                                                    seg_name_id,
                                                    ScopeType::Neutral,
                                                    LookupPattern::NamespaceOnly,
                                                )
                                            }) {
                                                if let Some(sym) =
                                                    compiler.symbols.get(sym_id.id as usize)
                                                {
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
                                                            let var = &compiler.variables
                                                                [var_id.id as usize];
                                                            if let VariableState::Known(val_id) =
                                                                var.state
                                                            {
                                                                map.push((
                                                                    seg.span,
                                                                    SemanticEntity::Symbol(sym_id),
                                                                ));
                                                                current_mod = None;
                                                                current_ty = Some(
                                                                    compiler.values
                                                                        [val_id.id as usize]
                                                                        .type_id,
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
                                            if let Some(type_info) =
                                                compiler.types.get(type_id.id as usize)
                                            {
                                                match &type_info.ty {
                                                    Type::Struct(sdef) => {
                                                        let field_idx = sdef
                                                            .fields
                                                            .iter()
                                                            .position(|member_id| {
                                                                compiler
                                                                    .members
                                                                    .get(member_id.id as usize)
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
                                                                .get(member_id.id as usize)
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
                                                                             .get(member_id.id as usize)
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
                                                                .get(member_id.id as usize)
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

        // 1. Symbol Definitions
        for (i, sym) in compiler.symbols.iter().enumerate() {
            if matches!(sym.sym_origin, SymbolOrigin::Module(mid) if mid.id == 0)
                || matches!(sym.sym_origin, SymbolOrigin::Compiler)
            {
                let sym_id = SymbolId::new(i as u32);
                if let Some(ast_id) = sym.ast_id
                    && let Some(Some(ast)) = self.asts.first()
                {
                    let span = ast.get_sym_span(ast_id);
                    map.push((span, SemanticEntity::Symbol(sym_id)));
                }
            }
        }

        // 2. Variable Usages
        for expr in &compiler.exprs[self.main_expr_range.clone()] {
            if let ExprHir::Var(sym_id) = expr.expr_hir {
                map.push((expr.span, SemanticEntity::Symbol(sym_id)));
            }
        }

        // 3. Field and Variant Definitions
        for ty_info in &compiler.types {
            if ty_info.owner.id != 0 {
                continue;
            }
            match &ty_info.ty {
                Type::Struct(sdef) => {
                    let sym = &compiler.symbols[sdef.sym_id.id as usize];
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
                    let sym = &compiler.symbols[edef.sym_id.id as usize];
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
                    let sym = &compiler.symbols[adef.sym_id.id as usize];
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
                        );
                    }
                    Item::TypeDef(def) => {
                        collect_type_refs(compiler, &def.sp_ty_expr, &mut map);
                        for cond in &def.conds {
                            collect_expr_refs(compiler, cond, &mut map, &self.text, &self.interner);
                        }
                    }
                    Item::Struct(s) => {
                        for cond in &s.glob_conds {
                            collect_expr_refs(compiler, cond, &mut map, &self.text, &self.interner);
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
                                );
                            }
                        }
                    }
                    Item::Enum(e) => {
                        for cond in &e.glob_conds {
                            collect_expr_refs(compiler, cond, &mut map, &self.text, &self.interner);
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
                                );
                            }
                        }
                    }
                    Item::Alias(a) => {
                        for cond in &a.conds {
                            collect_expr_refs(compiler, cond, &mut map, &self.text, &self.interner);
                        }
                    }
                    Item::Config(_) => {}
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
    pub fn get_entity_at_offset(&self, offset: usize) -> Option<&SemanticEntity> {
        // Find the smallest span that contains the offset, as it's the most specific.
        // This prevents broader expressions (like qualified names) from shadowing
        // their more specific components (like the module or field name).
        self.symbol_map
            .iter()
            .filter(|(span, _)| offset >= span.start as usize && offset < span.end as usize)
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
    pub fn get_symbol_at_offset(&self, byte_offset: usize) -> Option<(InternedId, usize, usize)> {
        self.get_token_at_offset(byte_offset).and_then(|st| {
            if let ScriptToken::Id(id) = st.tok {
                Some((id, st.span.start as usize, st.span.end as usize))
            } else {
                None
            }
        })
    }

    /// Returns the token that covers `byte_offset`, or `None` if no token is at that offset.
    pub fn get_token_at_offset(&self, byte_offset: usize) -> Option<&SpannedToken> {
        let idx = self
            .tokens
            .partition_point(|t| (t.span.end as usize) <= byte_offset);
        if idx < self.tokens.len() {
            let t = &self.tokens[idx];
            if byte_offset >= t.span.start as usize && byte_offset < t.span.end as usize {
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
                let sym = compiler.symbols.get(sym_id.id as usize)?;
                // Compiler-origin symbols (directives) have ast_id = None, so
                // they intentionally return no definition location — they are
                // built-in names without a user-visible definition site.
                let ast_id = sym.ast_id?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id,
                    SymbolOrigin::Compiler => 0,
                };
                let ast = self.asts.get(owner_id)?.as_ref()?;
                let span = ast.get_sym_span(ast_id);
                let module = compiler.mods.get(owner_id)?;
                let region = self.region_arena.get_region(module.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((path.to_string_lossy().to_string(), span, None))
            }
            SemanticEntity::Field {
                owner_sym_id,
                field_idx,
            } => {
                let sym = compiler.symbols.get(owner_sym_id.id as usize)?;
                let ast_id = sym.ast_id?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id,
                    SymbolOrigin::Compiler => 0,
                };
                let ast = self.asts.get(owner_id)?.as_ref()?;
                let abs_struct = ast.get_struct(ast_id);
                let field = abs_struct.fields.get(*field_idx)?;
                let module = compiler.mods.get(owner_id)?;
                let region = self.region_arena.get_region(module.region_id?)?;
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
                let sym = compiler.symbols.get(owner_sym_id.id as usize)?;
                let ast_id = sym.ast_id?;
                let owner_id = match sym.sym_origin {
                    SymbolOrigin::Module(mid) => mid.id,
                    SymbolOrigin::Compiler => 0,
                };
                let ast = self.asts.get(owner_id)?.as_ref()?;
                let abs_enum = ast.get_enum(ast_id);
                let variant = abs_enum.variants.get(*variant_idx)?;
                let module = compiler.mods.get(owner_id)?;
                let region = self.region_arena.get_region(module.region_id?)?;
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
                // For locals, they are defined in module 0 of the current state
                let region = self
                    .region_arena
                    .get_region(compiler.mods.first()?.region_id?)?;
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
                    .get_region(compiler.mods.get(mod_id.id)?.region_id?)?;
                let path = self.interner.search_path(region.path_id);
                Some((
                    path.to_string_lossy().to_string(),
                    SourceSpan::default(),
                    None,
                ))
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
    /// Returns `(state_uri, text_arc, span_start, span_end)` tuples so callers can
    /// convert byte offsets to LSP positions without re‑acquiring the document state.
    pub fn find_matching_entities(
        doc_cache: &DocumentCache,
        def_path: &str,
        def_span: SourceSpan,
        def_owner_sym_id: Option<SymbolId>,
    ) -> Vec<(String, Arc<String>, u32, u32)> {
        let mut results = Vec::new();
        doc_cache.for_each_state(|state_uri, state_arc| {
            let state = state_arc.read();
            for (span, ent) in &state.symbol_map {
                if let Some((other_def_path, other_def_span, other_def_owner_sym_id)) =
                    state.get_definition_location(ent)
                    && other_def_path == def_path
                    && other_def_span == def_span
                    && other_def_owner_sym_id == def_owner_sym_id
                {
                    results.push((
                        state_uri.to_string(),
                        Arc::clone(&state.text),
                        span.start,
                        span.end,
                    ));
                }
            }
        });
        results
    }

    pub fn offset_in_comment(&self, byte_offset: usize) -> bool {
        let idx = self
            .trivia
            .partition_point(|t| t.span.start as usize <= byte_offset);
        if idx > 0 {
            let t = &self.trivia[idx - 1];
            if byte_offset < t.span.end as usize && t.kind.is_comment() {
                return true;
            }
        }

        let text = self.text.as_bytes();
        if byte_offset >= text.len() {
            return false;
        }

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

        // 2. Perform expensive tokenization OUTSIDE any cache lock
        let mut interner = Intern::init();
        let (tokens, trivia) = Lexer::new(SourceRegionId::new(0), text.as_bytes(), script_start)
            .tokenize(&mut interner);

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
