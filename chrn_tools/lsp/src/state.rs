use parking_lot::RwLock;
use script_lib::lexer::Lexer;
use script_lib::modules::Module;
use script_lib::modules::ModuleMetadata;
use script_lib::script_compiler::ScriptCompiler;
use script_lib::semantic::name_resolver::NamespaceResolver;
use script_lib::semantic::type_resolver::TypeResolver;
use script_lib::semantic::type_resolver::type_context::TypeContext;
use script_lib::token::SpannedToken;
use script_lib::token::Token as ScriptToken;
use script_lib::trivia::Trivia;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::analyser;

use chrn_utils::id_types::{InternedId, ModuleId, PathId, SymbolId};
use chrn_utils::intern::Intern;
use common::chrn_settings::ChrnSettings;
use common::reporter::diagnostic::Diagnostic;
use common::span::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticEntity {
    Symbol(SymbolId),
    Field {
        owner_sym_id: SymbolId,
        field_idx: usize,
    },
    Variant {
        owner_sym_id: SymbolId,
        variant_idx: usize,
    },
    Module(ModuleId),
    Local {
        name_id: InternedId,
        decl_span: SourceSpan,
        owner_sym_id: Option<SymbolId>,
    },
}

pub struct DocumentState {
    pub text: Arc<String>,
    pub tokens: Vec<SpannedToken>,
    pub trivia: Vec<Trivia>,
    pub interner: Intern,
    pub script_start: usize,
    pub serial_start: Option<usize>,
    pub compiler: Option<ScriptCompiler>,
    pub asts: Vec<Option<script_lib::parser::ast::AstInfo>>,
    pub config_errors: Option<Vec<Diagnostic>>,
    pub parse_errors: Option<Vec<Diagnostic>>,
    pub ns_errors: Option<Vec<Diagnostic>>,
    pub ty_errors: Option<Vec<Diagnostic>>,
    pub symbol_map: Vec<(SourceSpan, SemanticEntity)>,
    pub main_expr_range: std::ops::Range<usize>,
    pub version: u64,
}

impl DocumentState {
    pub fn new(
        text: Arc<String>,
        tokens: Vec<SpannedToken>,
        trivia: Vec<Trivia>,
        interner: Intern,
        script_start: usize,
        serial_start: Option<usize>,
        version: u64,
    ) -> Self {
        DocumentState {
            text,
            tokens,
            trivia,
            interner,
            script_start,
            serial_start,
            compiler: None,
            asts: Vec::new(),
            config_errors: None,
            parse_errors: None,
            ns_errors: None,
            ty_errors: None,
            symbol_map: Vec::new(),
            main_expr_range: 0..0,
            version: version,
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
        let name_id = InternedId::new(self.interner.intern(&name));
        let path_id = PathId::new(self.interner.intern_path(&path_buf));

        let (bind, main_imports) = match script_lib::modules::mod_finder::ModuleFinder::new(
            self.text.as_bytes(),
            &settings,
            path_buf.clone(),
            self.script_start,
            self.serial_start,
        )
        .collect_imports(&mut self.interner)
        {
            Ok(res) => res,
            Err(e) => {
                self.config_errors = Some(vec![Self::extract_diag(e)]);
                (None, Vec::new())
            }
        };

        let main_mod = Module::new(
            name_id,
            ModuleId::new(0),
            main_imports,
            Some(ModuleMetadata::new(
                self.text.as_bytes().to_vec(),
                path_id,
                self.script_start,
                self.serial_start,
            )),
        );

        let mut mod_map = HashMap::new();
        mod_map.insert(name_id, ModuleId::new(0));

        let mut seen = std::collections::HashSet::new();
        seen.insert(path_id);

        let mut other_mods = Vec::with_capacity(main_mod.imports.len());
        // Errors from resolving imported modules belong to those modules' own documents
        // and are already recorded in their cached DocumentState. Propagating them into
        // this document's config_errors would attach byte offsets from the wrong file
        // (the dependency's), causing diagnostics to appear at random locations.
        let _ = analyser::resolve_modules_lsp(
            &mut seen,
            &mut other_mods,
            &main_mod,
            &mut mod_map,
            &settings,
            &mut self.interner,
            &doc_cache,
        );

        // Collect imported module URIs for dependency tracking
        let imported_uris: Vec<String> = other_mods
            .iter()
            .filter_map(|mod_opt| {
                let m = mod_opt.as_ref()?;
                let metadata = m.src_metadata.as_ref()?;
                let p = self.interner.search_path(metadata.path_id.id as usize);
                tower_lsp::lsp_types::Url::from_file_path(p)
                    .ok()
                    .map(|u| u.to_string())
            })
            .collect();

        let mut all_mods = Vec::with_capacity(other_mods.len() + 1);
        all_mods.push(main_mod);
        for mod_opt in other_mods.drain(..) {
            let known = mod_opt.expect("ModuleId reserving failed");
            all_mods.push(known);
        }

        let mut compiler = ScriptCompiler::init(bind, mod_map, all_mods);

        let mut all_asts = Vec::with_capacity(compiler.mods.len());
        for _ in 0..compiler.mods.len() {
            all_asts.push(None);
        }

        for mod_idx in 0..compiler.mods.len() {
            let module = &compiler.mods[mod_idx];
            let metadata = match &module.src_metadata {
                Some(m) => m,
                None => continue,
            };

            let parse_result = if mod_idx == 0 {
                // Reuse pre-computed tokens for main module
                script_lib::parser::parse(&settings, metadata, &self.tokens, &self.interner)
            } else {
                let (toks, _) = Lexer::new(&metadata.src_bytes, metadata.script_start)
                    .tokenize(&mut self.interner);
                script_lib::parser::parse(&settings, metadata, &toks, &self.interner)
            };

            let (ast_info, parse_errors) = match parse_result {
                Ok(ast_info) => (ast_info, None),
                Err((partial_ast, err)) => (partial_ast, Some(err)),
            };

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
        for mod_idx in 0..compiler.mods.len() {
            let ast_info = match &all_asts[mod_idx] {
                Some(a) => a,
                None => continue,
            };

            let mut ns_resolver = NamespaceResolver::new(
                &settings,
                ast_info,
                &self.interner,
                ModuleId::new(mod_idx),
                &mut compiler,
            );

            if let Err(ns_diags) = ns_resolver.resolve() {
                if mod_idx == 0 {
                    self.ns_errors = Some(ns_diags);
                }
            }
        }

        if self.parse_errors.is_none() && self.ns_errors.is_none() {
            let mut ty_ctx = TypeContext::new();
            let mut main_expr_range = 0..0;
            for mod_idx in 0..compiler.mods.len() {
                let ast_info = match &all_asts[mod_idx] {
                    Some(a) => a,
                    None => continue,
                };

                let expr_start = compiler.exprs.len();
                let mut type_resolver = TypeResolver::new(
                    &settings,
                    ast_info,
                    ModuleId::new(mod_idx),
                    &mut ty_ctx,
                    &self.interner,
                    &mut compiler,
                );

                if let Err(ty_diags) = type_resolver.resolve() {
                    if mod_idx == 0 {
                        self.ty_errors = Some(ty_diags);
                    }
                }
                let expr_end = compiler.exprs.len();
                if mod_idx == 0 {
                    main_expr_range = expr_start..expr_end;
                }
            }

            self.main_expr_range = main_expr_range;
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
            type_expr: &script_lib::parser::ast::SpannedTypeExpr,
            map: &mut Vec<(SourceSpan, SemanticEntity)>,
        ) {
            use script_lib::parser::ast::TypeExpr;
            match &type_expr.ty_expr {
                TypeExpr::Var(name_id) => {
                    let interned = *name_id;
                    // Look in common scopes for types
                    if let Some(sym_id) = compiler.get_sym_id(
                        interned,
                        script_lib::semantic::scopes::ScopeType::Var,
                        ModuleId::new(0),
                    ) {
                        map.push((type_expr.span, SemanticEntity::Symbol(sym_id)));
                    } else if let Some(sym_id) = compiler.get_sym_id(
                        interned,
                        script_lib::semantic::scopes::ScopeType::Neutral,
                        ModuleId::new(0),
                    ) {
                        map.push((type_expr.span, SemanticEntity::Symbol(sym_id)));
                    }
                }
                TypeExpr::Path(path) => {
                    if path.len() == 2 {
                        let mod_name_part = &path[0];
                        let sym_name_part = &path[1];
                        if let TypeExpr::Var(mod_name_id) = mod_name_part.ty_expr {
                            if let Some(mod_id) = compiler.mod_map.get(&mod_name_id) {
                                map.push((mod_name_part.span, SemanticEntity::Module(*mod_id)));
                                if let TypeExpr::Var(sym_name_id) = sym_name_part.ty_expr {
                                    // Search for symbol in target module
                                    if let Some(sym_id) = script_lib::semantic::scopes::get_sym_id(
                                        compiler,
                                        *mod_id,
                                        sym_name_id,
                                        script_lib::semantic::scopes::ScopeType::Neutral,
                                        script_lib::semantic::scopes::LookupPattern::ModuleOnly,
                                    )
                                    .or_else(|| {
                                        script_lib::semantic::scopes::get_sym_id(
                                            compiler,
                                            *mod_id,
                                            sym_name_id,
                                            script_lib::semantic::scopes::ScopeType::Var,
                                            script_lib::semantic::scopes::LookupPattern::ModuleOnly,
                                        )
                                    }) {
                                        map.push((
                                            sym_name_part.span,
                                            SemanticEntity::Symbol(sym_id),
                                        ));
                                    }
                                }
                                return;
                            }
                        }
                    }
                    for part in path {
                        collect_type_refs(compiler, part, map);
                    }
                }
                TypeExpr::Generic(generic) => {
                    for arg in &generic.args {
                        collect_type_refs(compiler, arg, map);
                    }
                }
            }
        }

        // Helper to collect expression references from AST
        fn collect_expr_refs(
            compiler: &ScriptCompiler,
            expr: &script_lib::parser::ast::SpannedExpr,
            map: &mut Vec<(SourceSpan, SemanticEntity)>,
            text: &str,
            interner: &Intern,
        ) {
            use script_lib::parser::ast::Expr;
            match &expr.expr {
                Expr::MemberAccess(acc) => {
                    if let Expr::Var(base_id) = acc.base.expr {
                        if let Some(mod_id) = compiler.mod_map.get(&base_id) {
                            map.push((acc.base.span, SemanticEntity::Module(*mod_id)));

                            // Try to find a precise span for the field name by searching after the dot
                            let full_span = expr.span;
                            let base_end = acc.base.span.end;
                            let field_name = interner.search(acc.field.id as usize);

                            // Look for the field name in the source text between dot and end of expr
                            let mut field_span = SourceSpan {
                                start: base_end.saturating_add(1),
                                end: full_span.end,
                            };

                            let search_area =
                                &text[base_end..=(full_span.end).min(text.len().saturating_sub(1))];
                            if let Some(dot_idx) = search_area.find('.') {
                                if let Some(name_idx) = search_area[dot_idx + 1..].find(field_name)
                                {
                                    let start = base_end + dot_idx + 1 + name_idx;
                                    field_span = SourceSpan {
                                        start,
                                        end: start + field_name.len() - 1,
                                    };
                                }
                            }

                            if let Some(sym_id) = script_lib::semantic::scopes::get_sym_id(
                                compiler,
                                *mod_id,
                                acc.field,
                                script_lib::semantic::scopes::ScopeType::Var,
                                script_lib::semantic::scopes::LookupPattern::ModuleOnly,
                            )
                            .or_else(|| {
                                script_lib::semantic::scopes::get_sym_id(
                                    compiler,
                                    *mod_id,
                                    acc.field,
                                    script_lib::semantic::scopes::ScopeType::Neutral,
                                    script_lib::semantic::scopes::LookupPattern::ModuleOnly,
                                )
                            }) {
                                map.push((field_span, SemanticEntity::Symbol(sym_id)));
                            }
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
                _ => {}
            }
        }

        // 1. Symbol Definitions
        for (i, sym) in compiler.symbols.iter().enumerate() {
            if sym.owner.id == 0 {
                let sym_id = SymbolId::new(i as u32);
                if let Some(ast_id) = sym.ast_id {
                    if let Some(Some(ast)) = self.asts.get(0) {
                        let span = ast.get_sym_span(ast_id);
                        map.push((span, SemanticEntity::Symbol(sym_id)));
                    }
                }
            }
        }

        // 2. Variable Usages
        for expr in &compiler.exprs[self.main_expr_range.clone()] {
            if let script_lib::semantic::representation::ExprHir::Var(sym_id) = expr.expr_hir {
                map.push((expr.span, SemanticEntity::Symbol(sym_id)));
            }
        }

        // 3. Field and Variant Definitions
        for ty_info in &compiler.types {
            if ty_info.owner.id != 0 {
                continue;
            }
            use script_lib::semantic::representation::Type;
            match &ty_info.ty {
                Type::Struct(sdef) => {
                    let sym = &compiler.symbols[sdef.sym_id.id as usize];
                    if let Some(Some(ast)) = self.asts.get(0) {
                        if let Some(ast_id) = sym.ast_id {
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
                }
                Type::Enum(edef) => {
                    let sym = &compiler.symbols[edef.sym_id.id as usize];
                    if let Some(Some(ast)) = self.asts.get(0) {
                        if let Some(ast_id) = sym.ast_id {
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
                }
                Type::Alias(adef) => {
                    let sym = &compiler.symbols[adef.sym_id.id as usize];
                    if let Some(Some(ast)) = self.asts.get(0) {
                        if let Some(ast_id) = sym.ast_id {
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
                }
                _ => {}
            }
        }

        // 4. Type and Expr References in AST
        if let Some(Some(ast)) = self.asts.get(0) {
            for item in ast.items() {
                use script_lib::parser::ast::Item;
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
                        collect_type_refs(compiler, &def.spanned_ty_expr, &mut map);
                        for cond in &def.conds {
                            collect_expr_refs(compiler, cond, &mut map, &self.text, &self.interner);
                        }
                    }
                    Item::Struct(s) => {
                        for cond in &s.glob_conds {
                            collect_expr_refs(compiler, cond, &mut map, &self.text, &self.interner);
                        }
                        for field in &s.fields {
                            collect_type_refs(compiler, &field.spanned_ty_expr, &mut map);
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
                            if let Some(ty) = &variant.ty_expr {
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
                }
            }
        }

        self.symbol_map = map;
    }

    pub fn get_entity_at_offset(&self, offset: usize) -> Option<&SemanticEntity> {
        // Find the smallest span that contains the offset, as it's the most specific.
        // This prevents broader expressions (like qualified names) from shadowing
        // their more specific components (like the module or field name).
        self.symbol_map
            .iter()
            .filter(|(span, _)| offset >= span.start && offset <= span.end)
            .min_by_key(|(span, _)| span.end.saturating_sub(span.start))
            .map(|(_, entity)| entity)
    }

    fn extract_diag(err: common::core_error::ConfigLoadError) -> Diagnostic {
        match err {
            common::core_error::ConfigLoadError::Unclosed(diag)
            | common::core_error::ConfigLoadError::Module(diag) => diag,
            common::core_error::ConfigLoadError::IO(io) => Diagnostic::new(
                &std::path::PathBuf::new(),
                io.to_string(),
                None,
                None,
                io.to_string(),
                common::reporter::diagnostic::Area::ConfigLoad,
            ),
        }
    }

    pub fn get_lsp_diagnostics(&self) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let mut lsp_diags = Vec::new();
        let doc_len = self.text.len();

        if let Some(diags) = &self.config_errors {
            analyser::push_diagnostics(&mut lsp_diags, diags, doc_len, &self.text, "chrn-config");
        }
        if let Some(diags) = &self.parse_errors {
            analyser::push_diagnostics(&mut lsp_diags, diags, doc_len, &self.text, "chrn-parser");
        }
        if let Some(diags) = &self.ns_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                doc_len,
                &self.text,
                "chrn-namespace",
            );
        }
        if let Some(diags) = &self.ty_errors {
            analyser::push_diagnostics(&mut lsp_diags, diags, doc_len, &self.text, "chrn-type");
        }

        lsp_diags
    }

    pub fn get_symbol_at_offset(&self, byte_offset: usize) -> Option<(u32, usize, usize)> {
        for st in &self.tokens {
            let span = st.span;
            if byte_offset >= span.start && byte_offset <= span.end {
                if let ScriptToken::Id(id) = st.tok {
                    return Some((id, span.start, span.end));
                }
                return None;
            }
        }
        None
    }

    pub fn get_identifier_at_offset(&self, byte_offset: usize) -> Option<String> {
        self.get_symbol_at_offset(byte_offset)
            .map(|(id, _, _)| self.interner.search(id as usize).to_string())
    }

    pub fn get_definition_location(
        &self,
        entity: &SemanticEntity,
    ) -> Option<(String, SourceSpan, Option<SymbolId>)> {
        let compiler = self.compiler.as_ref()?;
        match entity {
            SemanticEntity::Symbol(sym_id) => {
                let sym = compiler.symbols.get(sym_id.id as usize)?;
                let ast_id = sym.ast_id?;
                let ast = self.asts.get(sym.owner.id)?.as_ref()?;
                let span = ast.get_sym_span(ast_id);
                let metadata = compiler.mods.get(sym.owner.id)?.src_metadata.as_ref()?;
                let path = self.interner.search_path(metadata.path_id.id as usize);
                Some((path.to_string_lossy().to_string(), span, None))
            }
            SemanticEntity::Field {
                owner_sym_id,
                field_idx,
            } => {
                let sym = compiler.symbols.get(owner_sym_id.id as usize)?;
                let ast_id = sym.ast_id?;
                let ast = self.asts.get(sym.owner.id)?.as_ref()?;
                let abs_struct = ast.get_struct(ast_id);
                let field = abs_struct.fields.get(*field_idx)?;
                let metadata = compiler.mods.get(sym.owner.id)?.src_metadata.as_ref()?;
                let path = self.interner.search_path(metadata.path_id.id as usize);
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
                let ast = self.asts.get(sym.owner.id)?.as_ref()?;
                let abs_enum = ast.get_enum(ast_id);
                let variant = abs_enum.variants.get(*variant_idx)?;
                let metadata = compiler.mods.get(sym.owner.id)?.src_metadata.as_ref()?;
                let path = self.interner.search_path(metadata.path_id.id as usize);
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
                let metadata = compiler.mods.get(0)?.src_metadata.as_ref()?;
                let path = self.interner.search_path(metadata.path_id.id as usize);
                Some((
                    path.to_string_lossy().to_string(),
                    *decl_span,
                    *owner_sym_id,
                ))
            }
            SemanticEntity::Module(mod_id) => {
                let metadata = compiler.mods.get(mod_id.id)?.src_metadata.as_ref()?;
                let path = self.interner.search_path(metadata.path_id.id as usize);
                Some((path.to_string_lossy().to_string(), SourceSpan::default(), None))
            }
        }
    }

    /// Check if a given byte offset falls within a comment (single or multi).
    /// Uses binary search for O(log n) performance.
    /// Also checks for single-line comments by looking for // before the cursor on the current line.
    pub fn offset_in_comment(&self, byte_offset: usize) -> bool {
        let idx = self.trivia.partition_point(|t| t.span.start <= byte_offset);
        if idx > 0 {
            let t = &self.trivia[idx - 1];
            if byte_offset <= t.span.end && t.kind.is_comment() {
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

struct CacheInner {
    docs: HashMap<String, (Arc<String>, Arc<RwLock<DocumentState>>)>,
    /// URI → set of module URIs it imports
    imports: HashMap<String, HashSet<String>>,
    /// URI → set of URIs that import it (reverse index)
    dependents: HashMap<String, HashSet<String>>,
}

pub struct DocumentCache {
    inner: RwLock<CacheInner>,
    max_size: usize,
}

impl std::fmt::Debug for DocumentCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentCache")
            .field("max_size", &self.max_size)
            .finish()
    }
}

impl DocumentCache {
    pub fn new(max_size: usize) -> Self {
        DocumentCache {
            inner: RwLock::new(CacheInner {
                docs: HashMap::new(),
                imports: HashMap::new(),
                dependents: HashMap::new(),
            }),
            max_size,
        }
    }

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
            if let Some((cached_text, existing)) = cache.docs.get(uri) {
                if Arc::ptr_eq(cached_text, &text) || **cached_text == *text {
                    return Arc::clone(existing);
                }
            }
        }

        // 2. Perform expensive tokenization OUTSIDE any cache lock
        let mut interner = Intern::init();
        let (tokens, trivia) = Lexer::new(text.as_bytes(), script_start).tokenize(&mut interner);

        // 3. Re-acquire write lock to insert
        let mut cache = self.inner.write();

        // Double check after acquiring write lock in case another thread created it
        if let Some((cached_text, existing)) = cache.docs.get(uri) {
            if Arc::ptr_eq(cached_text, &text) || **cached_text == *text {
                return Arc::clone(existing);
            }
        }

        if cache.docs.len() >= self.max_size {
            let to_remove = cache.docs.len() - self.max_size + 1;
            let keys_to_remove: Vec<String> = cache
                .docs
                .keys()
                .take(to_remove)
                .map(|k| k.to_string())
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

        cache
            .docs
            .insert(uri.to_string(), (text, Arc::clone(&state)));
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
        let mut worklist = vec![uri.to_string()];

        while let Some(current) = worklist.pop() {
            cache.docs.remove(&current);

            if let Some(deps) = cache.dependents.get(&current) {
                for dep in deps {
                    if cache.docs.contains_key(dep.as_str()) {
                        worklist.push(dep.to_string());
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

    pub fn get(&self, uri: &str) -> Option<Arc<RwLock<DocumentState>>> {
        self.inner
            .read()
            .docs
            .get(uri)
            .map(|(_, state)| Arc::clone(state))
    }

    pub fn get_text(&self, uri: &str) -> Option<Arc<String>> {
        self.inner
            .read()
            .docs
            .get(uri)
            .map(|(text, _)| Arc::clone(text))
    }

    pub fn for_each_state<F>(&self, mut f: F)
    where
        F: FnMut(&str, Arc<RwLock<DocumentState>>),
    {
        let cache = self.inner.read();
        for (uri, (_, state)) in &cache.docs {
            f(uri, Arc::clone(state));
        }
    }

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
