use parking_lot::RwLock;
use script_lib::script_compiler::ScriptCompiler;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tower_lsp::lsp_types::SemanticToken;
use tower_lsp::{Client, LanguageServer, jsonrpc};

use crate::analyser::analyze_and_publish_task;
use crate::state::DocumentCache;
use crate::text::apply_text_change;

// Semantic token support (keyword/string/number highlighting)
use crate::state::SemanticEntity;
use chrn_utils::builtins::BuiltinTypeKind as ChBuiltinTypeKind;
use chrn_utils::id_types::PathId;
use chrn_utils::intern::Intern;
use common::chrn_settings::ChrnSettings;
use common::core_error::ConfigLoadError;
use script_lib::config_loader::ChrnConfigLoader;
use script_lib::semantic::representation::{SymbolKind, Type};
use script_lib::token::Token as ScriptToken;
use std::io::Cursor;
use std::path::PathBuf;

fn publish_config_load_error(
    client: &Client,
    uri: &tower_lsp::lsp_types::Url,
    text: &str,
    err: ConfigLoadError,
) {
    use tower_lsp::lsp_types::*;

    let start = Position {
        line: 0,
        character: 0,
    };

    let diag = match err {
        ConfigLoadError::Unclosed(diag) | ConfigLoadError::Module(diag) => {
            let diag_span = diag.span.unwrap_or_default();

            let start_pos = crate::text::offset_to_position(text, diag_span.start);
            let end_pos = crate::text::offset_to_position(text, diag_span.end);

            tower_lsp::lsp_types::Diagnostic {
                range: Range {
                    start: start_pos,
                    end: end_pos,
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("chrn-config".to_string()),
                message: diag.core_msg,
                related_information: None,
                tags: None,
                data: None,
            }
        }
        ConfigLoadError::IO(io) => tower_lsp::lsp_types::Diagnostic {
            range: Range { start, end: start },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("chrn-config".to_string()),
            message: io.to_string(),
            related_information: None,
            tags: None,
            data: None,
        },
    };

    let diags_vec = vec![diag];
    let client_clone = client.clone();
    let uri_clone = uri.clone();
    tokio::spawn(async move {
        client_clone
            .publish_diagnostics(uri_clone, diags_vec, None)
            .await;
    });
}

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
    pub docs: Arc<RwLock<HashMap<String, Arc<String>>>>,
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

    fn get_analyzed_state(
        &self,
        uri: &tower_lsp::lsp_types::Url,
        text: Arc<String>,
    ) -> Option<Arc<RwLock<crate::state::DocumentState>>> {
        let uri_str = uri.to_string();

        // Try to get existing analyzed state first
        if let Some(state_arc) = self.doc_cache.get(&uri_str) {
            let needs_analysis = {
                let state = state_arc.read();
                state.compiler.is_none() || state.text.len() != text.len() || *state.text != *text
            };

            if !needs_analysis {
                return Some(state_arc);
            }
        }

        let path_buf = PathBuf::from(uri.path());
        let settings = ChrnSettings::default();

        let mut interner = Intern::init();
        let path_id = PathId::new(interner.intern_path(&path_buf));
        let metadata = match ChrnConfigLoader::new(
            path_id,
            Cursor::new(text.as_bytes()),
            &settings,
            &mut interner,
        )
        .load_config()
        {
            Ok(m) => m,
            Err(e) => {
                publish_config_load_error(&self.client, uri, &text, e);
                return None;
            }
        };

        let state_arc = self.doc_cache.get_or_create(
            &uri_str,
            Arc::clone(&text),
            metadata.script_start,
            metadata.serial_start,
            0,
        );

        let imported_uris = {
            let mut state = state_arc.write();
            state.ensure_analyzed(&self.doc_cache, &path_buf)
        };

        if !imported_uris.is_empty() {
            self.doc_cache
                .register_dependencies(&uri_str, &imported_uris);
        }

        Some(state_arc)
    }
}

fn classify_id_token(
    compiler: &ScriptCompiler,
    entity: Option<&SemanticEntity>,
    id: u32,
    next_is_paren: bool,
) -> Option<u32> {
    if let Some(entity) = entity {
        match entity {
            SemanticEntity::Symbol(sym_id) => {
                if let Some(sym) = compiler.symbols.get(sym_id.id as usize) {
                    match sym.kind {
                        SymbolKind::Type(tid) => {
                            let ty = &compiler.types[tid.id as usize].ty;
                            if matches!(ty, Type::Alias(_)) {
                                return Some(SemanticTokenType::Function.as_u32());
                            }
                            return Some(SemanticTokenType::Type.as_u32());
                        }
                        SymbolKind::Val(_) => {
                            if next_is_paren {
                                return Some(SemanticTokenType::Function.as_u32());
                            }
                            return Some(SemanticTokenType::Variable.as_u32());
                        }
                        _ => return Some(SemanticTokenType::Variable.as_u32()),
                    }
                }
            }
            SemanticEntity::Field { .. } | SemanticEntity::Variant { .. } => {
                return Some(SemanticTokenType::Property.as_u32());
            }
            SemanticEntity::Module(_) => return Some(SemanticTokenType::Keyword.as_u32()),
            SemanticEntity::Local { .. } => return Some(SemanticTokenType::Variable.as_u32()),
        }
    }

    if ChBuiltinTypeKind::try_from_interned_id(id).is_some() {
        return Some(SemanticTokenType::Type.as_u32());
    }

    if next_is_paren {
        return Some(SemanticTokenType::Function.as_u32());
    }

    None
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _params: tower_lsp::lsp_types::InitializeParams,
    ) -> jsonrpc::Result<tower_lsp::lsp_types::InitializeResult> {
        let server_capabilities = tower_lsp::lsp_types::ServerCapabilities {
            // Advertise incremental sync so clients (neovim) send ranged edits.
            text_document_sync: Some(tower_lsp::lsp_types::TextDocumentSyncCapability::Kind(
                tower_lsp::lsp_types::TextDocumentSyncKind::INCREMENTAL,
            )),
            hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
            definition_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
            rename_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
            references_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
            // advertise completion support so clients will request completions
            completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                resolve_provider: Some(false),
                trigger_characters: Some(vec!["@".to_string(), "#".to_string(), ".".to_string()]),
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
        let uri_str = params.text_document.uri.to_string();
        let text = Arc::new(params.text_document.text);
        self.docs.write().insert(uri_str.clone(), Arc::clone(&text));

        // bump version
        let version = {
            let mut vers = self.pending_versions.write();
            let v = vers.entry(uri_str.clone()).or_insert(0);
            *v = v.wrapping_add(1);
            *v
        };

        let client = self.client.clone();
        let uri_cloned = params.text_document.uri.clone();
        let dc = self.diags_cache.clone();
        let doc_cache = self.doc_cache.clone();
        let pending_versions = self.pending_versions.clone();
        tokio::spawn(async move {
            analyze_and_publish_task(
                client,
                uri_cloned,
                text,
                dc,
                doc_cache,
                pending_versions,
                version,
            )
            .await
        });
    }

    async fn did_save(&self, params: tower_lsp::lsp_types::DidSaveTextDocumentParams) {
        let uri_str = params.text_document.uri.to_string();
        if let Some(text) = params.text {
            self.docs.write().insert(uri_str.clone(), Arc::new(text));
        }

        let text_opt = {
            let docs = self.docs.read();
            docs.get(&uri_str).cloned()
        };

        if let Some(text) = text_opt {
            // bump version
            let version = {
                let mut vers = self.pending_versions.write();
                let v = vers.entry(uri_str.clone()).or_insert(0);
                *v = v.wrapping_add(1);
                *v
            };

            let client = self.client.clone();
            let uri_cloned = params.text_document.uri.clone();
            let text_cloned = text.clone();

            if let Some(handle) = self.pending_tasks.write().remove(&uri_str) {
                handle.abort();
            }

            let dc = self.diags_cache.clone();
            let doc_cache = self.doc_cache.clone();
            let pending_versions = self.pending_versions.clone();
            tokio::spawn(async move {
                analyze_and_publish_task(
                    client,
                    uri_cloned,
                    text_cloned,
                    dc,
                    doc_cache,
                    pending_versions,
                    version,
                )
                .await
            });
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
        let uri_str = params.text_document.uri.to_string();

        // Apply all content changes in order. If a change has no range, it is a full text replace.
        let mut docs = self.docs.write();
        let existing = docs.remove(&uri_str).unwrap_or_default();
        let mut updated = (*existing).clone();
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
        let updated_arc = Arc::new(updated);
        docs.insert(uri_str.clone(), Arc::clone(&updated_arc));
        let text_opt = Some(updated_arc);
        drop(docs);

        // Invalidate cached document state since content changed
        self.doc_cache.invalidate(&uri_str);

        if let Some(text) = text_opt {
            // Debounce rapid changes: increment a per-doc version and run analysis only
            // if the version hasn't changed for DEBOUNCE_MS.
            const DEBOUNCE_MS: u64 = 150;

            // bump version
            let my_version = {
                let mut vers = self.pending_versions.write();
                let v = vers.entry(uri_str.clone()).or_insert(0);
                *v = v.wrapping_add(1);
                *v
            };

            let client = self.client.clone();
            let pv = self.pending_versions.clone();
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
            let doc_cache_clone = self.doc_cache.clone();
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
                    let doc_cache = doc_cache_clone.clone();
                    let pending_versions = pv.clone();
                    analyze_and_publish_task(
                        client,
                        params.text_document.uri,
                        Arc::clone(&text),
                        dc,
                        doc_cache,
                        pending_versions,
                        my_version,
                    )
                    .await;
                }
                // Attempt to remove our handle from pending_tasks. Use Weak::upgrade so
                // the spawned task does not hold a strong Arc to the pending_tasks map
                // (which would form a reference cycle and leak memory).
                if let Some(pending_tasks_arc) = pending_tasks_weak.upgrade() {
                    let _ = pending_tasks_arc.write().remove(&inner_uri_str);
                }
            });

            // store handle so we can abort it if another edit arrives or doc closes
            self.pending_tasks.write().insert(uri_str, handle);
        }
    }

    async fn hover(
        &self,
        params: tower_lsp::lsp_types::HoverParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = {
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => Arc::clone(t),
                None => return Ok(None),
            }
        };

        let state_arc = match self.get_analyzed_state(&uri, Arc::clone(&text)) {
            Some(s) => s,
            None => return Ok(None),
        };

        // Use a timeout to avoid deadlocking if analysis is stuck holding a write lock
        let arc_for_hover = Arc::clone(&state_arc);
        if let Some(hover_opt) = state_arc
            .try_read_for(Duration::from_millis(500))
            .map(|_guard| {
                // Drop our guard and let compute_hover take its own lock (which should succeed now)
                crate::hover::compute_hover(&uri, pos, arc_for_hover)
            })
        {
            return Ok(hover_opt);
        }
        Ok(None)
    }

    async fn rename(
        &self,
        params: tower_lsp::lsp_types::RenameParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        // Ensure the document is analyzed
        let text = {
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => Arc::clone(t),
                None => return Ok(None),
            }
        };

        if self.get_analyzed_state(&uri, text).is_none() {
            return Ok(None);
        }

        let edit = crate::rename::compute_rename(&uri, pos, new_name, &self.doc_cache);
        Ok(edit)
    }

    async fn references(
        &self,
        params: tower_lsp::lsp_types::ReferenceParams,
    ) -> jsonrpc::Result<Option<Vec<tower_lsp::lsp_types::Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        // Ensure the document is analyzed
        let text = {
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => Arc::clone(t),
                None => return Ok(None),
            }
        };

        if self.get_analyzed_state(&uri, text).is_none() {
            return Ok(None);
        }

        let refs = crate::references::compute_references(&uri, pos, &self.doc_cache);
        Ok(refs)
    }

    async fn semantic_tokens_full(
        &self,
        params: tower_lsp::lsp_types::SemanticTokensParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let text = {
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => Arc::clone(t),
                None => return Ok(None),
            }
        };

        let state_arc = match self.get_analyzed_state(&uri, Arc::clone(&text)) {
            Some(s) => s,
            None => return Ok(None),
        };

        let state = state_arc.read();
        let compiler = match &state.compiler {
            Some(c) => c,
            None => return Ok(None),
        };

        let toks_vec = &state.tokens;

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
            let start_pos = crate::text::offset_to_position(&state.text, span.start);
            let _end_pos = crate::text::offset_to_position(&state.text, span.end.saturating_add(1));
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
                    let entity = state.get_entity_at_offset(span.start);
                    if let Some(ty) = classify_id_token(compiler, entity, id, next_is_paren) {
                        ty
                    } else {
                        continue;
                    }
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
        use tower_lsp::lsp_types::{GotoDefinitionResponse, LocationLink, Range, Url};

        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let text = {
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => Arc::clone(t),
                None => return Ok(None),
            }
        };

        let state_arc = match self.get_analyzed_state(&uri, Arc::clone(&text)) {
            Some(s) => s,
            None => return Ok(None),
        };

        let state = state_arc.read();
        let byte_offset = crate::text::position_to_offset(&state.text, pos);
        let mut links: Vec<LocationLink> = Vec::new();

        let mut def_info = None;
        let entity = state.get_entity_at_offset(byte_offset);
        if let Some(entity) = entity {
            if let Some((def_path, def_span, _)) = state.get_definition_location(entity) {
                def_info = Some((def_path, def_span));
            }
        }

        if let Some((def_path, def_span)) = def_info {
            let target_uri = match Url::from_file_path(&def_path) {
                Ok(u) => u,
                Err(_) => uri.clone(),
            };

            // We need the text of the target file to convert span to position
            let target_text = if def_path == uri.path() {
                Some(Arc::clone(&state.text))
            } else {
                let target_uri_str = target_uri.to_string();
                self.doc_cache
                    .get_text(&target_uri_str)
                    .or_else(|| self.docs.read().get(&target_uri_str).map(Arc::clone))
                    .or_else(|| std::fs::read_to_string(&def_path).ok().map(Arc::new))
            };

            if let Some(t_text) = target_text {
                let start_pos = crate::text::offset_to_position(&t_text, def_span.start);
                let end_pos = crate::text::offset_to_position(&t_text, def_span.end + 1);

                links.push(LocationLink {
                    origin_selection_range: Some(Range {
                        start: pos,
                        end: pos,
                    }),
                    target_uri,
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

        let uri = &params.text_document_position.text_document.uri;
        let text = {
            let docs = self.docs.read();
            match docs.get(&uri.to_string()) {
                Some(t) => Arc::clone(t),
                None => return Ok(None),
            }
        };

        // Use the already-analyzed state to get script boundaries and tokens,
        // avoiding a redundant config load and re-lex.
        let state_arc = match self.get_analyzed_state(uri, Arc::clone(&text)) {
            Some(s) => s,
            None => return Ok(None),
        };

        let state_guard = match state_arc.try_read_for(Duration::from_millis(500)) {
            Some(g) => g,
            None => return Ok(None),
        };
        let state = &*state_guard;

        // find current word prefix using byte offsets
        let byte_off =
            crate::text::position_to_offset(&text, params.text_document_position.position);
        let (start_b, _end_b) = crate::text::find_word_bounds(&text, byte_off);
        let prefix = &text[start_b..byte_off.min(text.len())];

        // Determine the script section boundaries from cached state
        let script_start = state.script_start;
        let serial_start = state.serial_start.unwrap_or(text.len());
        let in_script_section = byte_off >= script_start && byte_off < serial_start;

        // If cursor is outside the script section, return no completions
        if !in_script_section {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        }

        let is_dot_completion = start_b > 0 && text.as_bytes()[start_b - 1] == b'.';
        let mut dot_target_name = None;
        if is_dot_completion {
            let (t_start, t_end) = crate::text::find_word_bounds(&text, start_b - 1);
            if t_start < t_end {
                dot_target_name = Some(&text[t_start..t_end]);
            }
        }

        if let Some(target_name) = dot_target_name {
            let mut items: Vec<CompletionItem> = Vec::new();
            if let Some(target_id) = state.interner.get_interned_id_async(target_name) {
                if let Some(compiler) = &state.compiler {
                    let intern_id = chrn_utils::id_types::InternedId::new(target_id);
                    if let Some(mod_id) = compiler.mod_map.get(&intern_id) {
                        if let Some(module) = compiler.mods.get(mod_id.id as usize) {
                            if mod_id.id == 0 {
                                // Current module: show all symbols except ScopeType::Var
                                for sym in &compiler.symbols {
                                    if sym.owner.id == 0
                                        && sym.scope_type
                                            != script_lib::semantic::scopes::ScopeType::Var
                                    {
                                        let sym_name =
                                            state.interner.search(sym.name_id.id as usize);
                                        if prefix.is_empty() || sym_name.starts_with(prefix) {
                                            let kind = match sym.kind {
                                                script_lib::semantic::representation::SymbolKind::Type(_) => CompletionItemKind::STRUCT,
                                                script_lib::semantic::representation::SymbolKind::Val(_) => CompletionItemKind::VARIABLE,
                                                _ => CompletionItemKind::PROPERTY,
                                            };
                                            items.push(CompletionItem {
                                                label: sym_name.to_string(),
                                                kind: Some(kind),
                                                ..Default::default()
                                            });
                                        }
                                    }
                                }
                            } else {
                                // Other modules: show only exported symbols
                                for sym_id in &module.exports {
                                    if let Some(sym) = compiler.symbols.get(sym_id.id as usize) {
                                        let sym_name =
                                            state.interner.search(sym.name_id.id as usize);
                                        if prefix.is_empty() || sym_name.starts_with(prefix) {
                                            let kind = match sym.kind {
                                                script_lib::semantic::representation::SymbolKind::Type(_) => CompletionItemKind::STRUCT,
                                                script_lib::semantic::representation::SymbolKind::Val(_) => CompletionItemKind::VARIABLE,
                                                _ => CompletionItemKind::PROPERTY,
                                            };
                                            items.push(CompletionItem {
                                                label: sym_name.to_string(),
                                                kind: Some(kind),
                                                ..Default::default()
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return Ok(Some(CompletionResponse::Array(items)));
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
            ("change", CompletionItemKind::KEYWORD),
            ("any", CompletionItemKind::STRUCT),
            ("BigInt", CompletionItemKind::STRUCT),
            ("BigFloat", CompletionItemKind::STRUCT),
            ("bool", CompletionItemKind::STRUCT),
            ("char", CompletionItemKind::STRUCT),
            ("f16", CompletionItemKind::STRUCT),
            ("f32", CompletionItemKind::STRUCT),
            ("f64", CompletionItemKind::STRUCT),
            ("f128", CompletionItemKind::STRUCT),
            ("i16", CompletionItemKind::STRUCT),
            ("i32", CompletionItemKind::STRUCT),
            ("i64", CompletionItemKind::STRUCT),
            ("i8", CompletionItemKind::STRUCT),
            ("i128", CompletionItemKind::STRUCT),
            ("List", CompletionItemKind::STRUCT),
            ("Map", CompletionItemKind::STRUCT),
            ("nil", CompletionItemKind::STRUCT),
            ("Set", CompletionItemKind::STRUCT),
            ("sized", CompletionItemKind::STRUCT),
            ("str", CompletionItemKind::STRUCT),
            ("Tuple", CompletionItemKind::STRUCT),
            ("u16", CompletionItemKind::STRUCT),
            ("u32", CompletionItemKind::STRUCT),
            ("u64", CompletionItemKind::STRUCT),
            ("u8", CompletionItemKind::STRUCT),
            ("u128", CompletionItemKind::STRUCT),
            ("unsized", CompletionItemKind::STRUCT),
            ("true", CompletionItemKind::CONSTANT),
            ("false", CompletionItemKind::CONSTANT),
            ("#warn", CompletionItemKind::VALUE),
            ("#bin", CompletionItemKind::VALUE),
            ("#octal", CompletionItemKind::VALUE),
            ("#scient", CompletionItemKind::VALUE),
            ("#hex", CompletionItemKind::VALUE),
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

        // Add modules from mod_map (using already-analyzed compiler state)
        if let Some(compiler) = &state.compiler {
            for (name_id, _) in &compiler.mod_map {
                // Skip the current module's name to avoid redundant suggestions

                let name = state.interner.search(name_id.id as usize);

                if prefix.is_empty() || name.starts_with(prefix) {
                    items.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::MODULE),
                        ..Default::default()
                    });
                }
            }
        }

        // Reuse pre-computed tokens from the analyzed state instead of re-lexing.
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for st in &state.tokens {
            // Only include identifiers that are within the script section (before serial_start)
            let tok_end = st.span.end;
            if tok_end > serial_start {
                continue;
            }

            if let ScriptToken::Id(id) = st.tok {
                let name = state.interner.search(id as usize);
                if (prefix.is_empty() || name.starts_with(prefix)) && seen.insert(name) {
                    items.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}
