use parking_lot::RwLock;
use script_lib::script_compiler::ScriptCompiler;
use std::time::Duration;
use std::{collections::{HashMap, HashSet}, sync::Arc};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tower_lsp::lsp_types::SemanticToken;
use tower_lsp::{Client, LanguageServer, jsonrpc};

use crate::analysis::analyze_and_publish_task;
// use crate::definition::find_in_source;
use crate::state::DocumentCache;
use crate::text::apply_text_change;

// Semantic token support (keyword/string/number highlighting)
use chrn_utils::builtins::BuiltinTypeKind as ChBuiltinTypeKind;
use chrn_utils::id_types::{InternedId, ModuleId, PathId};
use chrn_utils::intern::Intern;
use common::chrn_settings::ChernSettings;
use script_lib::config_loader::ChernConfigLoader;
use script_lib::lexer::Lexer;
use script_lib::modules::Module;
use script_lib::semantic::name_resolver::NamespaceResolver;
use script_lib::semantic::representation::SymbolKind;
use script_lib::semantic::scopes::ScopeType;
use script_lib::semantic::type_resolver::TypeResolver;
use script_lib::semantic::type_resolver::type_context::TypeContext;
use script_lib::token::Token as ScriptToken;
use std::io::Cursor;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticTokenType {
    Keyword,
    String,
    Number,
    Type,
    Function,
    Macro,
    Operator,
    Variable,
    Property,
    Class,
}

impl SemanticTokenType {
    pub fn as_u32(self) -> u32 {
        match self {
            SemanticTokenType::Keyword => 0,
            SemanticTokenType::String => 1,
            SemanticTokenType::Number => 2,
            SemanticTokenType::Type => 3,
            SemanticTokenType::Function => 4,
            SemanticTokenType::Macro => 5,
            SemanticTokenType::Operator => 6,
            SemanticTokenType::Variable => 7,
            SemanticTokenType::Property => 8,
            SemanticTokenType::Class => 9,
        }
    }
}

#[derive(Debug)]
pub struct Backend {
    pub client: Client,
    // store documents text by uri
    pub docs: Arc<RwLock<HashMap<String, String>>>,
    // per-document counter used to debounce rapid change events; incremented on each change
    pub pending_versions: Arc<RwLock<HashMap<String, u64>>>,
    // cache of last published diagnostics to avoid re-sending identical sets
    pub diags_cache: Arc<RwLock<HashMap<String, String>>>,
    // store JoinHandles for in-flight debounce tasks so we can abort them
    pub pending_tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    // document state cache for tokens, AST, and analysis results
    pub doc_cache: Arc<DocumentCache>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            docs: Arc::new(RwLock::new(HashMap::new())),
            pending_versions: Arc::new(RwLock::new(HashMap::new())),
            diags_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_tasks: Arc::new(RwLock::new(HashMap::new())),
            doc_cache: Arc::new(DocumentCache::new(50)),
        }
    }
}

// Helper to classify identifier tokens into semantic token kinds.
fn classify_id_token(
    compiler: &ScriptCompiler,
    id: u32,
    has_type_info: bool,
    next_is_paren: bool,
    member_ids: Option<&HashSet<u32>>,
) -> u32 {
    // If this identifier is a struct field or enum variant name, classify as Property
    if let Some(mset) = member_ids {
        if mset.contains(&id) {
            return SemanticTokenType::Property.as_u32();
        }
    }
    // Builtin type names are interned to well-known ids; check them first.
    if ChBuiltinTypeKind::try_from_interned_id(id).is_some() {
        return SemanticTokenType::Type.as_u32();
    }

    if next_is_paren {
        return SemanticTokenType::Function.as_u32();
    }

    //TODO: A bit more specific eventually
    if has_type_info {
        let interned = InternedId::new(id);
        if let Some(sym_id) = compiler.mods[0].get_sym_id(interned, ScopeType::Var) {
            if let Some(sym) = compiler.symbols.get(&sym_id) {
                match sym.kind {
                    SymbolKind::Type(_) => return SemanticTokenType::Type.as_u32(),
                    // A SymbolKind::Val represents a value (variable). Even if the value
                    // has a type annotation (struct/enum/typedef/etc.), we should not
                    // highlight the identifier as a type. Only symbols that are
                    // SymbolKind::Type (type declarations) or builtin type names are
                    // considered types for highlighting.
                    SymbolKind::Val(_) => return SemanticTokenType::Variable.as_u32(),
                    _ => return SemanticTokenType::Variable.as_u32(),
                }
            }
        }
    }

    SemanticTokenType::Variable.as_u32()
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: tower_lsp::lsp_types::InitializeParams,
    ) -> jsonrpc::Result<tower_lsp::lsp_types::InitializeResult> {
        let server_capabilities = tower_lsp::lsp_types::ServerCapabilities {
            // Advertise incremental sync so clients (neovim) send ranged edits.
            text_document_sync: Some(tower_lsp::lsp_types::TextDocumentSyncCapability::Kind(
                tower_lsp::lsp_types::TextDocumentSyncKind::INCREMENTAL,
            )),
            hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
            // advertise completion support so clients will request completions
            completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                resolve_provider: Some(false),
                trigger_characters: Some(vec![
                    "@".to_string(),
                    "#".to_string(),
                    ":".to_string(),
                    "-".to_string(),
                    ">".to_string(),
                ]),
                ..Default::default()
            }),
            // advertise semantic tokens for full documents (keywords/strings/numbers)
            semantic_tokens_provider: Some(
                tower_lsp::lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(
                    tower_lsp::lsp_types::SemanticTokensOptions {
                        legend: tower_lsp::lsp_types::SemanticTokensLegend {
                            // Provide a richer set of token kinds so clients can color
                            // keywords, types, functions, macros, operators and variables
                            // differently.
                            // Ok
                            token_types: vec![
                                tower_lsp::lsp_types::SemanticTokenType::KEYWORD, // 0
                                tower_lsp::lsp_types::SemanticTokenType::STRING,
                                tower_lsp::lsp_types::SemanticTokenType::NUMBER, // 2
                                tower_lsp::lsp_types::SemanticTokenType::TYPE,
                                tower_lsp::lsp_types::SemanticTokenType::FUNCTION, // 4
                                tower_lsp::lsp_types::SemanticTokenType::MACRO,
                                tower_lsp::lsp_types::SemanticTokenType::OPERATOR, // 6
                                tower_lsp::lsp_types::SemanticTokenType::VARIABLE,
                                tower_lsp::lsp_types::SemanticTokenType::PROPERTY,
                                tower_lsp::lsp_types::SemanticTokenType::CLASS, // was nothing
                            ],
                            token_modifiers: vec![],
                        },
                        range: None,
                        full: Some(tower_lsp::lsp_types::SemanticTokensFullOptions::Bool(true)),
                        ..Default::default()
                    },
                ),
            ),
            ..Default::default()
        };

        Ok(tower_lsp::lsp_types::InitializeResult {
            capabilities: server_capabilities,
            server_info: None,
        })
    }

    async fn initialized(&self, _: tower_lsp::lsp_types::InitializedParams) {}

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let text = params.text_document.text;
        self.docs.write().insert(uri, text);
    }

    async fn did_save(&self, params: tower_lsp::lsp_types::DidSaveTextDocumentParams) {
        // Ensure we have the latest saved text if the client provided it
        if let Some(text) = params.text.as_ref() {
            let uri = params.text_document.uri.to_string();
            self.docs.write().insert(uri, text.clone());
        }
        // run analysis on save (if uri available)
        let uri = params.text_document.uri.clone();
        // Avoid holding the lock across await by cloning text into local
        if let Some(text) = params.text.as_ref() {
            let client = self.client.clone();
            let uri_cloned = uri.clone();
            let text_cloned = text.clone();
            // On explicit save, run analysis immediately (no debounce). Abort any pending debounce task
            // to avoid duplicate work and keep task count bounded.
            if let Some(handle) = self.pending_tasks.write().remove(&uri.to_string()) {
                handle.abort();
            }
            let dc = self.diags_cache.clone();
            tokio::spawn(async move {
                analyze_and_publish_task(client, uri_cloned, text_cloned, dc).await
            });
        } else {
            let maybe_text = {
                let docs = self.docs.read();
                docs.get(&uri.to_string()).cloned()
            };

            if let Some(text) = maybe_text {
                let client = self.client.clone();
                let uri_cloned = uri.clone();
                let text_cloned = text.clone();
                // On save but without provided text, still run immediately. Abort any pending debounce task.
                if let Some(handle) = self.pending_tasks.write().remove(&uri.to_string()) {
                    handle.abort();
                }
                let dc = self.diags_cache.clone();
                tokio::spawn(async move {
                    analyze_and_publish_task(client, uri_cloned, text_cloned, dc).await
                });
            }
        }
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        // Remove the document on close to free memory and avoid stale state
        let uri = params.text_document.uri.to_string();
        self.docs.write().remove(&uri);
        // remove any pending debounce/version info for this doc
        self.pending_versions.write().remove(&uri);
        // remove cached diagnostics for this doc
        self.diags_cache.write().remove(&uri);
        // abort and remove any pending debounce task for this document
        if let Some(handle) = self.pending_tasks.write().remove(&uri) {
            handle.abort();
        }
        // invalidate document state cache
        self.doc_cache.invalidate(&uri);
    }

    async fn did_change(&self, params: tower_lsp::lsp_types::DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        // Apply all content changes in order. If a change has no range, it is a full text replace.
        let mut docs = self.docs.write();
        let existing = docs.remove(&uri.to_string()).unwrap_or_default();
        let mut updated = existing;
        for change in params.content_changes.into_iter() {
            match apply_text_change(&updated, &change) {
                Ok(next) => updated = next,
                Err(e) => {
                    // report error back to client via window/showMessage
                    let _ = self.client.show_message(
                        tower_lsp::lsp_types::MessageType::ERROR,
                        format!("Failed to apply text change: {}", e),
                    );
                    // keep the previous content to avoid corrupting buffer
                    return;
                }
            }
        }
        docs.insert(uri.to_string(), updated);
        // clone the current text while holding the lock then drop lock before awaiting
        let maybe_text = docs.get(&uri.to_string()).cloned();
        drop(docs);

        // Invalidate cached document state since content changed
        self.doc_cache.invalidate(&uri.to_string());

        if let Some(text) = maybe_text {
            // Debounce rapid changes: increment a per-doc version and run analysis only
            // if the version hasn't changed for DEBOUNCE_MS.
            const DEBOUNCE_MS: u64 = 150;
            let uri_str = uri.to_string();

            // bump version
            let my_version = {
                let mut vers = self.pending_versions.write();
                let v = vers.entry(uri_str.clone()).or_insert(0);
                *v = v.wrapping_add(1);
                *v
            };

            let client = self.client.clone();
            let pv = self.pending_versions.clone();
            let uri_cloned = uri.clone();
            let text_cloned = text.clone();
            // clone diag cache so the spawned task doesn't borrow `self`
            let dc = self.diags_cache.clone();

            // Abort any previously scheduled debounce task for this uri to avoid accumulating
            // sleeping tasks when edits are frequent.
            if let Some(prev) = self.pending_tasks.write().remove(&uri_str) {
                prev.abort();
            }

            // Use a Weak reference to pending_tasks inside the spawned task to avoid
            // forming a strong reference cycle: Arc -> HashMap -> JoinHandle -> Arc.
            // The Weak won't keep the Arc alive if the rest of the server drops it.
            let pending_tasks_weak = Arc::downgrade(&self.pending_tasks);
            let inner_uri_str = uri_str.clone();
            let handle = tokio::spawn(async move {
                sleep(Duration::from_millis(DEBOUNCE_MS)).await;

                // check version
                let still_current = {
                    let vers = pv.read();
                    match vers.get(&inner_uri_str) {
                        Some(&v) if v == my_version => true,
                        _ => false,
                    }
                };

                if still_current {
                    analyze_and_publish_task(client, uri_cloned, text_cloned, dc).await;
                }
                // Attempt to remove our handle from pending_tasks. Use Weak::upgrade so
                // the spawned task does not hold a strong Arc to the pending_tasks map
                // (which would form a reference cycle and leak memory).
                if let Some(pending_tasks_arc) = pending_tasks_weak.upgrade() {
                    let _ = pending_tasks_arc.write().remove(&inner_uri_str);
                }
            });

            // store handle so we can abort it if another edit arrives or doc closes
            self.pending_tasks.write().insert(uri_str.clone(), handle);
        }
    }

    async fn hover(
        &self,
        params: tower_lsp::lsp_types::HoverParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.docs.read();
        let text = match docs.get(&uri.to_string()) {
            Some(t) => t.clone(),
            None => return Ok(None),
        };

        // Delegate hover computation to hover module
        let hover_opt = crate::hover::compute_hover(&text, &uri.to_string(), pos);
        Ok(hover_opt)
    }

    async fn semantic_tokens_full(
        &self,
        params: tower_lsp::lsp_types::SemanticTokensParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let docs = self.docs.read();
        let text = match docs.get(&uri.to_string()) {
            Some(t) => t.clone(),
            None => return Ok(None),
        };

        // Lex the document
        let path_buf = PathBuf::from(uri.path());
        let src_bytes = text.as_bytes().to_vec();
        let settings = ChernSettings::default();

        let metadata = match ChernConfigLoader::new(
            path_buf.as_path(),
            Cursor::new(src_bytes.clone()),
            &settings,
        )
        .load_config()
        {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        // Build a minimal in-memory compiler pipeline so we can consult
        // the resolved symbols/types for semantic token classification.
        let mut interner = Intern::init();

        let name = path_buf
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("<unnamed>")
            .to_string();
        let name_id = InternedId::new(interner.intern(&name));
        let path_id = PathId::new(interner.intern_path(&path_buf));
        let module = Module::new(name_id, path_id, ModuleId::new(0), Vec::new(), metadata);

        let mut mod_map = HashMap::new();
        mod_map.insert(name_id, ModuleId::new(0));
        let mut compiler = ScriptCompiler::new(None, mod_map, vec![module]);

        let toks = Lexer::new(
            &compiler.mods[0].metadata.src_bytes,
            compiler.mods[0].metadata.script_start,
        )
        .tokenize(&mut interner);

        // We'll iterate with indices so we can peek the next token when needed
        let toks_vec = toks;

        // Attempt to parse & resolve namespace/types. Failures are ignored and
        // we'll still produce tokens without type-specialization.
        let mut has_type_info = false;
        // Collect struct field / enum variant name ids so we can mark them as Property
        let mut maybe_member_ids: Option<HashSet<u32>> = None;
        if let Ok(ast_info) =
            script_lib::parser::parse(&settings, &compiler.mods[0], &toks_vec, &interner)
        {
            let mut ns_resolver = NamespaceResolver::new(
                &settings,
                &ast_info,
                &interner,
                ModuleId::new(0),
                &mut compiler,
            );

            if ns_resolver.resolve().is_ok() {
                let mut ty_ctx = TypeContext::new();
                let mut type_resolver = TypeResolver::new(
                    &settings,
                    &ast_info,
                    ModuleId::new(0),
                    &mut ty_ctx,
                    &interner,
                    &mut compiler,
                );

                if type_resolver.resolve().is_ok() {
                    has_type_info = true;
                    // Build a set of member name ids (fields and enum variants)
                    let mut mset: HashSet<u32> = HashSet::new();
                    for ty_info in compiler.types.iter() {
                        match &ty_info.ty {
                            script_lib::semantic::representation::Type::Struct(sdef) => {
                                for fld in sdef.fields.iter() {
                                    mset.insert(fld.name_id.id);
                                }
                            }
                            script_lib::semantic::representation::Type::Enum(edef) => {
                                for v in edef.variants.iter() {
                                    mset.insert(v.name_id.id);
                                }
                            }
                            _ => {}
                        }
                    }
                    maybe_member_ids = Some(mset);
                }
            }
        }

        let mut tokens: Vec<SemanticToken> = Vec::new();

        // keep previous token position for delta encoding
        let mut prev_line: u32 = 0;
        let mut prev_start: u32 = 0;
        let mut first = true;

        // Highlighting
        for i in 0..toks_vec.len() {
            let st = &toks_vec[i];
            let span = st.span;
            // convert start byte to position
            let start_pos = crate::text::offset_to_position(&text, span.start);
            let end_pos = crate::text::offset_to_position(&text, span.end.saturating_add(1));
            // compute length in chars from start_pos to end_pos roughly using bytes difference
            let length = (span.end.saturating_add(1).saturating_sub(span.start)) as u32;

            // Map script token variants to our semantic token legend indices.
            let token_type: u32 = match st.tok {
                ScriptToken::Def | ScriptToken::End => SemanticTokenType::Macro.as_u32(),
                ScriptToken::Keyword(kw) if kw.is_sect() => SemanticTokenType::Class.as_u32(),
                ScriptToken::Keyword(_) => SemanticTokenType::Keyword.as_u32(),
                ScriptToken::Str(_) => SemanticTokenType::String.as_u32(),
                ScriptToken::BoolLiteral(_) => SemanticTokenType::String.as_u32(),
                ScriptToken::Integer(_, _) | ScriptToken::Float(_, _) => {
                    SemanticTokenType::Number.as_u32()
                }
                ScriptToken::Id(id) => {
                    let next_is_paren = i + 1 < toks_vec.len()
                        && matches!(toks_vec[i + 1].tok, ScriptToken::OParen);
                    classify_id_token(&compiler, id, has_type_info, next_is_paren, maybe_member_ids.as_ref())
                }
                ScriptToken::At => SemanticTokenType::Macro.as_u32(),
                ScriptToken::HashSymbol => SemanticTokenType::Operator.as_u32(),
                // punctuation and operators -> OPERATOR
                ScriptToken::Assign
                | ScriptToken::EqualTo
                | ScriptToken::Walrus
                | ScriptToken::Comma
                | ScriptToken::DotRange
                | ScriptToken::Slash
                | ScriptToken::Percent
                | ScriptToken::Plus
                | ScriptToken::Asterisk
                | ScriptToken::Hyphen
                | ScriptToken::GreaterOrEq
                | ScriptToken::LessOrEq
                | ScriptToken::NotEq
                | ScriptToken::Ampersand
                | ScriptToken::And
                | ScriptToken::Or
                | ScriptToken::Caret
                | ScriptToken::ExclamationPoint
                | ScriptToken::Tilde
                | ScriptToken::VerticalBar
                | ScriptToken::Dot
                | ScriptToken::OParen
                | ScriptToken::CParen
                | ScriptToken::OBracket
                | ScriptToken::CBracket
                | ScriptToken::OCurlyBracket
                | ScriptToken::CCurlyBracket
                | ScriptToken::OAngleBracket
                | ScriptToken::CAngleBracket
                | ScriptToken::QuestionMark
                | ScriptToken::Colon
                | ScriptToken::SlimArrow => SemanticTokenType::Operator.as_u32(),
                _ => continue,
            };

            let delta_line: u32;
            let delta_start: u32;

            if first {
                delta_line = start_pos.line;
                delta_start = start_pos.character;
                first = false;
            } else {
                if start_pos.line == prev_line {
                    delta_line = 0;
                    delta_start = start_pos.character.saturating_sub(prev_start);
                } else {
                    delta_line = start_pos.line.saturating_sub(prev_line);
                    delta_start = start_pos.character;
                }
            }

            prev_line = start_pos.line;
            prev_start = start_pos.character;

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset: 0,
            });
        }

        let sem_toks = tower_lsp::lsp_types::SemanticTokens {
            result_id: None,
            data: tokens,
        };

        Ok(Some(tower_lsp::lsp_types::SemanticTokensResult::Tokens(
            sem_toks,
        )))
    }

    async fn goto_definition(
        &self,
        params: tower_lsp::lsp_types::GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::GotoDefinitionResponse>> {
        use tower_lsp::lsp_types::{GotoDefinitionResponse, LocationLink, Range};

        let text = {
            let uri = params
                .text_document_position_params
                .text_document
                .uri
                .clone();
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };

        let pos = params.text_document_position_params.position;

        let path_buf = PathBuf::from(
            params
                .text_document_position_params
                .text_document
                .uri
                .path(),
        );
        let src_bytes = text.as_bytes().to_vec();
        let settings = ChernSettings::default();

        let metadata = match ChernConfigLoader::new(
            path_buf.as_path(),
            Cursor::new(src_bytes.clone()),
            &settings,
        )
        .load_config()
        {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        let mut interner = Intern::init();
        let toks = Lexer::new(text.as_bytes(), metadata.script_start).tokenize(&mut interner);

        let byte_offset = crate::text::position_to_offset(&text, pos);

        let mut found_name: Option<String> = None;

        for st in &toks {
            let span = st.span;
            if byte_offset >= span.start && byte_offset <= span.end {
                match &st.tok {
                    ScriptToken::Id(id) => {
                        found_name = Some(interner.search(*id as usize).to_string());
                        break;
                    }
                    _ => {}
                }
                break;
            }
        }

        let name = match found_name {
            Some(n) => n,
            None => return Ok(None),
        };

        let mut links: Vec<LocationLink> = Vec::new();

        if let Some(def_range) = crate::definition::find_in_source(&text, &name) {
            let start_pos = crate::text::offset_to_position(&text, def_range.0);
            let end_pos = crate::text::offset_to_position(&text, def_range.1);

            if def_range.0 < text.len() {
                links.push(LocationLink {
                    origin_selection_range: Some(Range {
                        start: pos,
                        end: pos,
                    }),
                    target_uri: params
                        .text_document_position_params
                        .text_document
                        .uri
                        .clone(),
                    target_range: Range {
                        start: start_pos,
                        end: end_pos,
                    },
                    target_selection_range: Range {
                        start: start_pos,
                        end: end_pos,
                    },
                });
            }
        }

        if links.is_empty() {
            Ok(None)
        } else {
            Ok(Some(GotoDefinitionResponse::Link(links)))
        }
    }

    async fn completion(
        &self,
        params: tower_lsp::lsp_types::CompletionParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::CompletionResponse>> {
        use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse};

        // Simple completion: combine a curated set of language tokens with
        // identifiers discovered in the current document.
        let uri = params.text_document_position.text_document.uri.clone();
        let pos = params.text_document_position.position;

        let docs = self.docs.read();
        let text = match docs.get(&uri.to_string()) {
            Some(t) => t.clone(),
            None => return Ok(None),
        };

        // find current word prefix using byte offsets
        let byte_off = crate::text::position_to_offset(&text, pos);
        let (start_b, _end_b) = crate::text::find_word_bounds(&text, byte_off);
        let prefix = &text[start_b..byte_off.min(text.len())];

        // Load metadata to determine script/serial boundaries
        let path_buf = PathBuf::from(uri.path());
        let src_bytes = text.as_bytes().to_vec();
        let settings = ChernSettings::default();

        let metadata = match ChernConfigLoader::new(
            path_buf.as_path(),
            Cursor::new(src_bytes.clone()),
            &settings,
        )
        .load_config()
        {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };

        // Determine the script section boundaries
        let script_start = metadata.script_start;
        let serial_start = metadata.serial_start.unwrap_or(text.len());
        let in_script_section = byte_off >= script_start && byte_off < serial_start;

        // If cursor is outside the script section, return no completions
        if !in_script_section {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        }

        // Suggestions for keywords, types, etc
        const SUGGESTIONS: &[(&str, CompletionItemKind)] = &[
            ("@def", CompletionItemKind::SNIPPET),
            ("@end", CompletionItemKind::SNIPPET),
            ("bind", CompletionItemKind::KEYWORD),
            ("import", CompletionItemKind::KEYWORD),
            ("export", CompletionItemKind::KEYWORD),
            ("alias", CompletionItemKind::KEYWORD),
            ("let", CompletionItemKind::KEYWORD),
            ("as", CompletionItemKind::KEYWORD),
            ("var->", CompletionItemKind::KEYWORD),
            ("nest->", CompletionItemKind::KEYWORD),
            ("complex->", CompletionItemKind::KEYWORD),
            ("override->", CompletionItemKind::KEYWORD),
            ("struct", CompletionItemKind::KEYWORD),
            ("enum", CompletionItemKind::KEYWORD),
            ("List", CompletionItemKind::STRUCT),
            ("Map", CompletionItemKind::STRUCT),
            ("Set", CompletionItemKind::STRUCT),
            ("Tuple", CompletionItemKind::STRUCT),
            ("true", CompletionItemKind::CONSTANT),
            ("false", CompletionItemKind::CONSTANT),
            //TODO: Not exactly a keyword
            ("#warn", CompletionItemKind::VALUE),
            ("#ignore", CompletionItemKind::VALUE),
        ];

        let mut items: Vec<CompletionItem> = Vec::new();

        for (label, kind) in SUGGESTIONS.iter() {
            if prefix.is_empty() || label.starts_with(prefix) {
                items.push(CompletionItem {
                    label: label.to_string(),
                    kind: Some(*kind),
                    ..Default::default()
                });
            }
        }

        // Add identifiers from the document (lexed only in script section).
        // De-duplicate using a map.
        let mut interner = Intern::init();
        let toks = Lexer::new(text.as_bytes(), metadata.script_start).tokenize(&mut interner);

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for st in toks.into_iter() {
            // Only include identifiers that are within the script section (before serial_start)
            let tok_end = st.span.end;
            if tok_end > serial_start {
                continue;
            }

            if let ScriptToken::Id(id) = st.tok {
                let name = interner.search(id as usize);
                if (prefix.is_empty() || name.starts_with(prefix)) && seen.insert(name.into()) {
                    items.push(CompletionItem {
                        label: name.into(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}
