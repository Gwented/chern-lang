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
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::analyser;

use chrn_utils::id_types::{InternedId, ModuleId, PathId};
use chrn_utils::intern::Intern;
use common::chrn_settings::ChrnSettings;
use common::reporter::diagnostic::Diagnostic;

pub struct DocumentState {
    pub text: Arc<String>,
    pub tokens: Vec<SpannedToken>,
    pub interner: Intern,
    pub script_start: usize,
    pub serial_start: Option<usize>,
    pub compiler: Option<ScriptCompiler>,
    pub asts: Vec<script_lib::parser::ast::AstInfo>,
    pub parse_errors: Option<Vec<Diagnostic>>,
    pub ns_errors: Option<Vec<Diagnostic>>,
    pub ty_errors: Option<Vec<Diagnostic>>,
    pub has_parse_errors: bool,
    pub has_ns_errors: bool,
    pub has_ty_errors: bool,
    pub member_ids: HashSet<u32>,
    pub version: u64,
}

impl DocumentState {
    pub fn new(
        text: Arc<String>,
        script_start: usize,
        serial_start: Option<usize>,
        version: u64,
    ) -> Self {
        let mut interner = Intern::init();
        let toks = Lexer::new(text.as_bytes(), script_start).tokenize(&mut interner);

        DocumentState {
            text,
            tokens: toks,
            interner,
            script_start,
            serial_start,
            compiler: None,
            asts: Vec::new(),
            parse_errors: None,
            ns_errors: None,
            ty_errors: None,
            has_parse_errors: false,
            has_ns_errors: false,
            has_ty_errors: false,
            member_ids: HashSet::new(),
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
            .file_name()
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
            Err(_) => (None, Vec::new()),
        };

        let main_mod = Module::new(
            name_id,
            path_id,
            ModuleId::new(0),
            main_imports,
            ModuleMetadata::new(
                self.text.as_bytes().to_vec(),
                self.script_start,
                self.serial_start,
            ),
        );

        let mut mod_map = HashMap::new();
        mod_map.insert(name_id, ModuleId::new(0));

        let mut seen = std::collections::HashSet::new();
        seen.insert(path_id);

        let mut other_mods = Vec::new();
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
            .filter_map(|m| {
                let p = self.interner.search_path(m.path_id.id as usize);
                tower_lsp::lsp_types::Url::from_file_path(p)
                    .ok()
                    .map(|u| u.to_string())
            })
            .collect();

        let mut all_mods = Vec::with_capacity(other_mods.len() + 1);
        all_mods.push(main_mod);
        all_mods.append(&mut other_mods);

        let mut compiler = ScriptCompiler::new(bind, mod_map, all_mods);

        let mut all_asts = Vec::new();

        for mod_idx in 0..compiler.mods.len() {
            let module = &compiler.mods[mod_idx];

            let parse_result = if mod_idx == 0 {
                // Reuse pre-computed tokens for main module
                script_lib::parser::parse(&settings, &module, &self.tokens, &self.interner)
            } else {
                let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
                    .tokenize(&mut self.interner);
                script_lib::parser::parse(&settings, &module, &toks, &self.interner)
            };

            let (ast_info, parse_errors) = match parse_result {
                Ok(ast_info) => (ast_info, None),
                Err((partial_ast, err)) => (partial_ast, Some(err)),
            };

            if mod_idx == 0 {
                self.has_parse_errors = parse_errors.is_some();

                if let Some(diags) = parse_errors {
                    self.parse_errors = Some(diags);
                } else {
                    self.parse_errors = None;
                }
            }

            all_asts.push(ast_info);
        }

        // Namespace resolution for all modules
        for mod_idx in 0..compiler.mods.len() {
            let mut ns_resolver = NamespaceResolver::new(
                &settings,
                &all_asts[mod_idx],
                &self.interner,
                ModuleId::new(mod_idx),
                &mut compiler,
            );

            if let Err(ns_diags) = ns_resolver.resolve() {
                if mod_idx == 0 {
                    self.ns_errors = Some(ns_diags);
                    self.has_ns_errors = true;
                }
            }
        }

        if !self.has_parse_errors && !self.has_ns_errors {
            let mut ty_ctx = TypeContext::new();
            for mod_idx in 0..compiler.mods.len() {
                let mut type_resolver = TypeResolver::new(
                    &settings,
                    &all_asts[mod_idx],
                    ModuleId::new(mod_idx),
                    &mut ty_ctx,
                    &self.interner,
                    &mut compiler,
                );

                if let Err(ty_diags) = type_resolver.resolve() {
                    if mod_idx == 0 {
                        self.ty_errors = Some(ty_diags);
                        self.has_ty_errors = true;
                    }
                }
            }

            let mut member_map: HashSet<u32> = HashSet::new();
            for ty_info in compiler.types.iter() {
                match &ty_info.ty {
                    script_lib::semantic::representation::Type::Struct(sdef) => {
                        for fld in sdef.fields.iter() {
                            member_map.insert(fld.name_id.id);
                        }
                    }
                    script_lib::semantic::representation::Type::Enum(edef) => {
                        for v in edef.variants.iter() {
                            member_map.insert(v.name_id.id);
                        }
                    }
                    _ => {}
                }
            }

            self.member_ids = member_map;
        }

        self.asts = all_asts;
        self.compiler = Some(compiler);

        imported_uris
    }

    pub fn get_lsp_diagnostics(&self) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let mut lsp_diags = Vec::new();
        let doc_len = self.text.len();

        if let Some(diags) = &self.parse_errors {
            analyser::push_diagnostics(&mut lsp_diags, diags, doc_len, &self.text, "chern-parser");
        }
        if let Some(diags) = &self.ns_errors {
            analyser::push_diagnostics(
                &mut lsp_diags,
                diags,
                doc_len,
                &self.text,
                "chern-namespace",
            );
        }
        if let Some(diags) = &self.ty_errors {
            analyser::push_diagnostics(&mut lsp_diags, diags, doc_len, &self.text, "chern-type");
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

    pub fn find_definition_of_symbol(
        &self,
        interned_id: u32,
    ) -> Option<(String, common::span::Span)> {
        let compiler = self.compiler.as_ref()?;
        let interned = InternedId::new(interned_id);

        // Try to find the symbol in the main module first
        if let Some(sym_id) =
            compiler.mods[0].get_sym_id(interned, script_lib::semantic::scopes::ScopeType::Var)
        {
            if let Some(sym) = compiler.symbols.get(&sym_id) {
                let module = &compiler.mods[sym.owner.id];
                let ast_info = &self.asts.get(sym.owner.id)?;
                let span = ast_info.get_sym_span(sym.ast_id);

                let path = self.interner.search_path(module.path_id.id as usize);
                return Some((path.to_string_lossy().to_string(), span));
            }
        }
        None
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
        let mut cache = self.inner.write();

        if let Some((_, existing)) = cache.docs.get(uri) {
            let existing_state = existing.read();
            // Compare Arcs first (cheap), then content if needed
            if Arc::ptr_eq(&existing_state.text, &text) || *existing_state.text == *text {
                return Arc::clone(existing);
            }
        }

        if cache.docs.len() >= self.max_size {
            // Efficient eviction without cloning all keys
            let to_remove = cache.docs.len() - self.max_size + 1;
            let keys_to_remove: Vec<String> = cache
                .docs
                .keys()
                .take(to_remove)
                .map(|k| k.to_string())
                .collect();
            for key in keys_to_remove {
                cache.docs.remove(&key);
            }
        }

        let state = Arc::new(RwLock::new(DocumentState::new(
            Arc::clone(&text),
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
                    // Only add if the doc entry still exists (avoids re-visiting)
                    if cache.docs.contains_key(dep.as_str()) {
                        worklist.push(dep.to_string());
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
