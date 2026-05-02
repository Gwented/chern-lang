use parking_lot::RwLock;
use script_lib::lexer::Lexer;
use script_lib::modules::Module;
use script_lib::modules::ModuleMetadata;
use script_lib::script_compiler::ScriptCompiler;
use script_lib::semantic::name_resolver::NamespaceResolver;
use script_lib::semantic::type_resolver::type_context::TypeContext;
use script_lib::semantic::type_resolver::TypeResolver;
use script_lib::token::SpannedToken;
use script_lib::token::Token as ScriptToken;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use chrn_utils::id_types::{InternedId, ModuleId, PathId};
use chrn_utils::intern::Intern;
use common::chrn_settings::ChernSettings;
use common::reporter::diagnostic::Diagnostic;

pub struct DocumentState {
    pub text: String,
    pub tokens: Vec<SpannedToken>,
    pub interner: Intern,
    pub script_start: usize,
    pub serial_start: Option<usize>,
    pub compiler: Option<ScriptCompiler>,
    pub parse_errors: Option<Vec<Diagnostic>>,
    pub ns_errors: Option<Vec<Diagnostic>>,
    pub has_parse_errors: bool,
    pub has_ns_errors: bool,
    pub member_ids: HashSet<u32>,
}

impl DocumentState {
    pub fn new(text: String, script_start: usize, serial_start: Option<usize>) -> Self {
        let mut interner = Intern::init();
        let toks = Lexer::new(text.as_bytes(), script_start).tokenize(&mut interner);

        DocumentState {
            text,
            tokens: toks,
            interner,
            script_start,
            serial_start,
            compiler: None,
            parse_errors: None,
            ns_errors: None,
            has_parse_errors: false,
            has_ns_errors: false,
            member_ids: HashSet::new(),
        }
    }

    pub fn ensure_analyzed(&mut self) {
        if self.compiler.is_some() {
            return;
        }

        let settings = ChernSettings::default();
        let path_buf = PathBuf::from("<cached>");

        let name = "main".to_string();
        let name_id = InternedId::new(self.interner.intern(&name));
        let path_id = PathId::new(self.interner.intern_path(&path_buf));
        let module = Module::new(
            name_id,
            path_id,
            ModuleId::new(0),
            Vec::new(),
            ModuleMetadata::new(
                self.text.as_bytes().to_vec(),
                self.script_start,
                self.serial_start,
            ),
        );

        let mut mod_map = HashMap::new();
        mod_map.insert(name_id, ModuleId::new(0));
        let mut compiler = ScriptCompiler::new(None, mod_map, vec![module]);

        let toks_vec: Vec<_> = self.tokens.clone();
        let parse_result =
            script_lib::parser::parse(&settings, &compiler.mods[0], &toks_vec, &self.interner);

        let (ast_info, parse_errors) = match parse_result {
            Ok(ast_info) => (ast_info, None),
            Err((partial_ast, err)) => (partial_ast, Some(err)),
        };

        self.has_parse_errors = parse_errors.is_some();

        if let Some(err) = parse_errors {
            if let common::core_error::ScriptError::Parser(diags)
            | common::core_error::ScriptError::Semantic(diags) = err
            {
                self.parse_errors = Some(diags);
            } else {
                self.parse_errors = None;
            }
        }

        let mut ns_resolver = NamespaceResolver::new(
            &settings,
            &ast_info,
            &self.interner,
            ModuleId::new(0),
            &mut compiler,
        );

        if let Err(ns_diags) = ns_resolver.resolve() {
            self.ns_errors = Some(ns_diags);
            self.has_ns_errors = true;
        }

        if !self.has_parse_errors && !self.has_ns_errors {
            let mut ty_ctx = TypeContext::new();
            let mut type_resolver = TypeResolver::new(
                &settings,
                &ast_info,
                ModuleId::new(0),
                &mut ty_ctx,
                &self.interner,
                &mut compiler,
            );

            let _ = type_resolver.resolve();

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

        self.compiler = Some(compiler);
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
}

pub struct DocumentCache {
    inner: RwLock<HashMap<String, Arc<RwLock<DocumentState>>>>,
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
            inner: RwLock::new(HashMap::new()),
            max_size,
        }
    }

    pub fn get_or_create(
        &self,
        uri: &str,
        text: String,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> Arc<RwLock<DocumentState>> {
        let mut cache = self.inner.write();

        if let Some(existing) = cache.get(uri) {
            let existing_state = existing.read();
            if existing_state.text == text {
                return Arc::clone(existing);
            }
        }

        if cache.len() >= self.max_size {
            let to_remove = cache.len() - self.max_size + 1;
            let keys: Vec<_> = cache.keys().take(to_remove).cloned().collect();
            for key in keys {
                cache.remove(&key);
            }
        }

        let state = Arc::new(RwLock::new(DocumentState::new(
            text,
            script_start,
            serial_start,
        )));
        cache.insert(uri.to_string(), Arc::clone(&state));
        state
    }

    pub fn invalidate(&self, uri: &str) {
        self.inner.write().remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<Arc<RwLock<DocumentState>>> {
        self.inner.read().get(uri).map(Arc::clone)
    }

    pub fn clear(&self) {
        self.inner.write().clear();
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new(50)
    }
}
