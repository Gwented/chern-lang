//! # backend
//!
//! The [`Backend`] struct is the central hub of the LSP server.  It implements
//! [`tower_lsp::LanguageServer`] and routes every LSP request/notification to the
//! appropriate analysis or formatting helper.
//!
//! ## Lifecycle
//!
//! ```text
//! initialize        — negotiates capabilities with the editor
//! initialized       — no-op (analysis is demand-driven)
//! did_open          — stores text, spawns async analysis task
//! did_change        — applies incremental edits, debounces analysis (150 ms)
//! did_save          — stores new text, spawns analysis task immediately
//! did_close         — evicts text, diagnostics, and analysis state
//! shutdown          — no-op
//! ```
//!
//! ## LSP requests handled
//!
//! | Request                | Helper called                  |
//! |------------------------|--------------------------------|
//! | `textDocument/hover`           | [`crate::hover::compute_hover`]         |
//! | `textDocument/definition`      | [`crate::state::DocumentState::get_definition_location`] |
//! | `textDocument/references`      | [`crate::references::compute_references`] |
//! | `textDocument/rename`          | [`crate::rename::compute_rename`]       |
//! | `textDocument/completion`      | inline in `Backend::completion` |
//! | `textDocument/semanticTokens/full` | inline in `Backend::semantic_tokens_full` |
//!
//! ## Debounce / version invariant
//!
//! `pending_versions` tracks a monotonically increasing counter per URI.  On
//! `did_change` the counter is bumped and a 150 ms sleep is awaited before
//! analysis runs.  If another change arrives before the timer fires, the previous
//! task is aborted (`pending_tasks`) and the new one takes over.  Analysis results
//! carry the version they were spawned for; stale results are discarded in
//! [`crate::analyser::publish_if_current`].

use compilation::lookup::scopes;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::hir_concepts::{Symbol, SymbolKind, Type, VariableState};
use compilation::token::Token as ScriptToken;
use lang::config_loader::ChrnConfigLoader;
use parking_lot::RwLock;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tower_lsp::lsp_types::{CompletionItemKind, SemanticToken};
use tower_lsp::{Client, LanguageServer, jsonrpc};

use crate::analyser::analyze_and_publish_task;
use crate::state::DocumentCache;
use crate::state::DocumentState;
use crate::text::apply_text_change;

// Semantic token support (keyword/string/number highlighting)
use crate::state::SemanticEntity;
use chrn_utils::chrn_settings::ChrnSettings;
use chrn_utils::core_error::ConfigLoadError;
use chrn_utils::intern::Intern;
use lang::types::builtins::BuiltinTypeKind as ChBuiltinTypeKind;
use std::io::Cursor;
use std::path::PathBuf;

/// Publishes config-load diagnostics without awaiting, used in synchronous helpers
/// that cannot be `async`.
fn publish_config_load_error(
    client: Client,
    uri: tower_lsp::lsp_types::Url,
    text: &str,
    err: ConfigLoadError,
) {
    let diags = crate::analyser::config_load_error_to_diagnostics(err, text);
    tokio::spawn(async move {
        client.publish_diagnostics(uri, diags, None).await;
    });
}

/// Semantic token type indices matching the legend declared in [`Backend::initialize`].
///
/// The `as_u32` method returns the index into the `token_types` vector advertised
/// to the client.  **The order must match [`SEMANTIC_TOKENS_LEGEND`](Backend::initialize)**.
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
    EnumMember,
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
            SemanticTokenType::EnumMember => 10,
        }
    }
}

/// The central LSP server state, shared across all request handlers via `Arc`.
///
/// All fields wrapped in `Arc<RwLock<_>>` are shared with async analysis tasks.
#[derive(Debug)]
pub struct Backend {
    pub client: Client,
    /// Raw document texts keyed by URI string, kept in sync with editor state.
    pub docs: Arc<RwLock<HashMap<String, Arc<String>>>>,
    /// Monotonic per-URI change counter; bumped on every `did_change` / `did_open`.
    pub pending_versions: Arc<RwLock<HashMap<String, u64>>>,
    /// JSON-serialised last-published diagnostics per URI; used to suppress
    /// redundant `publishDiagnostics` notifications.
    pub diags_cache: Arc<RwLock<HashMap<String, String>>>,
    /// Handles to in-flight debounce tasks so they can be aborted on newer changes.
    pub pending_tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    /// Document analysis cache: tokens, AST, compiler, symbol map.
    pub doc_cache: Arc<DocumentCache>,
}

impl Backend {
    /// Creates a new `Backend` with empty state and a default 50-document analysis cache.
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

    /// Returns an analysed [`DocumentState`](crate::state::DocumentState) for `uri`,
    /// running analysis synchronously if needed.
    ///
    /// Preferred over spawning a task when the caller already holds the document text
    /// (e.g. in hover / definition / references / rename handlers that need the state
    /// before they can respond).
    ///
    /// Falls back to publishing a config-load error and returning `None` if the
    /// document header cannot be parsed.
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
                state.compiler.is_none() || *state.text != *text
            };

            if !needs_analysis {
                return Some(state_arc);
            }
        }

        let path_buf = PathBuf::from(uri.path());
        let settings = ChrnSettings::default();

        let mut interner = Intern::init();
        let path_id = interner.intern_path(&path_buf);
        let region = match ChrnConfigLoader::new(
            chrn_utils::id_types::SourceRegionId::new(0),
            Cursor::new(text.as_bytes()),
            path_id,
            &settings,
            &mut interner,
        )
        .load_config()
        {
            Ok(m) => m,
            Err(e) => {
                publish_config_load_error(self.client.clone(), uri.clone(), &text, e);
                return None;
            }
        };

        let state_arc = self.doc_cache.get_or_create(
            &uri_str,
            Arc::clone(&text),
            region.script_start,
            region.serial_start,
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

    /// Looks up the current source text for `uri` from the `docs` map.
    fn get_document_text(&self, uri: &str) -> Option<Arc<String>> {
        self.docs.read().get(uri).map(Arc::clone)
    }

    /// Convenience: retrieves the document text and runs [`get_analyzed_state`](Self::get_analyzed_state).
    fn get_state(&self, uri: &tower_lsp::lsp_types::Url) -> Option<Arc<RwLock<DocumentState>>> {
        let text = self.get_document_text(&uri.to_string())?;
        self.get_analyzed_state(uri, text)
    }

    /// Atomically increments the version counter for `uri`, returning the new value.
    fn bump_version(&self, uri: &str) -> u64 {
        let mut vers = self.pending_versions.write();
        let v = vers.entry(uri.to_string()).or_insert(0);
        *v = v.wrapping_add(1);
        *v
    }

    /// Ensures the file where a symbol is defined is analyzed and cached,
    /// enabling cross-module operations (rename, references) to find
    /// occurrences in the definition file even when it hasn't been opened.
    fn ensure_definition_file_analyzed(
        &self,
        state: &DocumentState,
        byte_offset: usize,
        current_uri: &tower_lsp::lsp_types::Url,
    ) {
        if state.offset_in_comment(byte_offset) {
            return;
        }

        let entity = match state.get_entity_at_offset(byte_offset) {
            Some(e) => e,
            None => return,
        };

        if matches!(
            entity,
            SemanticEntity::Local { .. } | SemanticEntity::Module(_)
        ) {
            return;
        }

        if let Some((def_path_str, _, _)) = state.get_definition_location(entity) {
            let def_path = std::path::Path::new(&def_path_str);
            if def_path == current_uri.path() {
                return;
            }
            if let Ok(def_uri) = tower_lsp::lsp_types::Url::from_file_path(def_path) {
                if let Ok(text) = std::fs::read_to_string(def_path) {
                    self.get_analyzed_state(&def_uri, Arc::new(text));
                }
            }
        }
    }
}

/// Returns the [`CompletionItemKind`] that best represents the symbol for completion.
///
/// Used by the completion handler to assign icons to items shown in the editor UI.
fn symbol_completion_kind(compiler: &ScriptCompiler, sym: &Symbol) -> CompletionItemKind {
    match sym.kind {
        SymbolKind::Type(type_id) => match &compiler.types[type_id.id as usize].ty {
            Type::Struct(_) | Type::Enum(_) | Type::TypeDef(_) | Type::BuiltinType(_) => {
                CompletionItemKind::STRUCT
            }
            Type::Alias(_) => CompletionItemKind::FUNCTION,
            Type::Func(func_def) if func_def.is_callable => CompletionItemKind::FUNCTION,
            Type::Func(_) => CompletionItemKind::CONSTANT,
            Type::Unknown | Type::Constrained(_) | Type::Deferred(_) => {
                CompletionItemKind::VARIABLE
            }
        },
        SymbolKind::Variable(var_id) => {
            let var = &compiler.variables[var_id.id as usize];
            let VariableState::Known(val_id) = var.state else {
                return CompletionItemKind::VARIABLE;
            };
            let type_id = compiler.values[val_id.id as usize].type_id;
            match &compiler.types[type_id.id as usize].ty {
                Type::BuiltinType(_) | Type::Struct(_) | Type::TypeDef(_) | Type::Enum(_) => {
                    CompletionItemKind::VARIABLE
                }
                Type::Alias(_) => CompletionItemKind::FUNCTION,
                Type::Func(func_def) if func_def.is_callable => CompletionItemKind::FUNCTION,
                Type::Func(_) => CompletionItemKind::CONSTANT,
                Type::Unknown | Type::Constrained(_) | Type::Deferred(_) => {
                    CompletionItemKind::VARIABLE
                }
            }
        }
        SymbolKind::Module(_) => CompletionItemKind::MODULE,
        SymbolKind::Config(config_id) => todo!(),
        SymbolKind::Directive(_) => CompletionItemKind::KEYWORD,
    }
}

/// Maps a token to a semantic token type index, used to colour-highlight identifiers.
///
/// Returns `None` for identifiers that should not receive semantic highlighting
/// (e.g. unresolved names when no entity is found and the next token is not `(`)
/// so the client falls back to syntactic colouring.
///
/// # Parameters
/// * `compiler`      — Needed to inspect symbol and type information.
/// * `entity`        — The [`SemanticEntity`] at the token offset, if any.
/// * `id`            — The raw interned ID of the identifier token.
/// * `next_is_paren` — Whether the next token is `(`, indicating a call expression.
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
                            match ty {
                                Type::BuiltinType(_)
                                | Type::TypeDef(_)
                                | Type::Struct(_)
                                | Type::Enum(_) => {
                                    return Some(SemanticTokenType::Type.as_u32());
                                }
                                Type::Alias(_) => {
                                    return Some(SemanticTokenType::Function.as_u32());
                                }
                                Type::Func(func_def) if func_def.is_callable => {
                                    return Some(SemanticTokenType::Function.as_u32());
                                }
                                Type::Func(_) => {
                                    return Some(SemanticTokenType::String.as_u32());
                                }
                                Type::Unknown | Type::Constrained(_) | Type::Deferred(_) => {
                                    return Some(SemanticTokenType::Type.as_u32());
                                }
                            }
                        }
                        SymbolKind::Variable(_) => {
                            if next_is_paren {
                                return Some(SemanticTokenType::Function.as_u32());
                            }
                            return Some(SemanticTokenType::Variable.as_u32());
                        }
                        SymbolKind::Module(_) => {
                            return Some(SemanticTokenType::Variable.as_u32());
                        }
                        SymbolKind::Config(cfg_id) => todo!(),
                        SymbolKind::Directive(_) => {
                            return Some(SemanticTokenType::Class.as_u32());
                        }
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
            completion_provider: Some(tower_lsp::lsp_types::CompletionOptions {
                resolve_provider: Some(false),
                trigger_characters: Some(vec![
                    "@".to_string(),
                    "#".to_string(),
                    ".".to_string(),
                    ":".to_string(),
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

        let version = self.bump_version(&uri_str);
        let pending_versions = self.pending_versions.clone();

        let client = self.client.clone();
        let dc = self.diags_cache.clone();
        let doc_cache = self.doc_cache.clone();
        tokio::spawn(async move {
            analyze_and_publish_task(
                client,
                params.text_document.uri,
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

        let text: Arc<String> = if let Some(t) = params.text {
            let text = Arc::new(t);
            self.docs.write().insert(uri_str.clone(), Arc::clone(&text));
            text
        } else if let Some(t) = self.docs.read().get(&uri_str).map(Arc::clone) {
            t
        } else {
            return;
        };

        let version = self.bump_version(&uri_str);
        let pending_versions = self.pending_versions.clone();

        let client = self.client.clone();
        let dc = self.diags_cache.clone();
        let doc_cache = self.doc_cache.clone();

        if let Some(handle) = self.pending_tasks.write().remove(&uri_str) {
            handle.abort();
        }

        tokio::spawn(async move {
            analyze_and_publish_task(
                client,
                params.text_document.uri,
                text,
                dc,
                doc_cache,
                pending_versions,
                version,
            )
            .await
        });
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
                    let _ = self.client.show_message(
                        tower_lsp::lsp_types::MessageType::ERROR,
                        format!("Failed to apply text change: {}", e),
                    );
                    return;
                }
            }
        }
        let updated_arc = Arc::new(updated);
        docs.insert(uri_str.clone(), Arc::clone(&updated_arc));
        drop(docs);

        self.doc_cache.invalidate(&uri_str);

        const DEBOUNCE_MS: u64 = 150;

        let my_version = self.bump_version(&uri_str);

        let client = self.client.clone();
        let pv = self.pending_versions.clone();
        let dc = self.diags_cache.clone();

        if let Some(prev) = self.pending_tasks.write().remove(&uri_str) {
            prev.abort();
        }

        let pending_tasks_weak = Arc::downgrade(&self.pending_tasks);
        let inner_uri_str = uri_str.clone();
        let doc_cache = self.doc_cache.clone();
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(DEBOUNCE_MS)).await;

            let still_current = {
                let vers = pv.read();
                match vers.get(&inner_uri_str) {
                    Some(&v) if v == my_version => true,
                    _ => false,
                }
            };

            if still_current {
                analyze_and_publish_task(
                    client,
                    params.text_document.uri,
                    updated_arc,
                    dc,
                    doc_cache,
                    pv,
                    my_version,
                )
                .await;
            }

            if let Some(pending_tasks_arc) = pending_tasks_weak.upgrade() {
                let _ = pending_tasks_arc.write().remove(&inner_uri_str);
            }
        });

        self.pending_tasks.write().insert(uri_str, handle);
    }

    async fn hover(
        &self,
        params: tower_lsp::lsp_types::HoverParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(text) = self.get_document_text(&uri.to_string()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };
        let state = match state_arc.try_read_for(Duration::from_millis(500)) {
            Some(guard) => guard,
            None => return Ok(None),
        };
        Ok(crate::hover::compute_hover(&uri, pos, &state))
    }

    async fn rename(
        &self,
        params: tower_lsp::lsp_types::RenameParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        let Some(text) = self.get_document_text(&uri.to_string()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };

        {
            let state = state_arc.read();
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            self.ensure_definition_file_analyzed(&state, byte_offset, &uri);
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

        let Some(text) = self.get_document_text(&uri.to_string()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };

        {
            let state = state_arc.read();
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            self.ensure_definition_file_analyzed(&state, byte_offset, &uri);
        }

        let refs = crate::references::compute_references(&uri, pos, &self.doc_cache);
        Ok(refs)
    }

    async fn semantic_tokens_full(
        &self,
        params: tower_lsp::lsp_types::SemanticTokensParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(text) = self.get_document_text(&uri.to_string()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
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
            let start_pos = crate::text::offset_to_position(&state.text, span.start as usize);
            let _end_pos =
                crate::text::offset_to_position(&state.text, span.end.saturating_add(1) as usize);
            // compute length in chars from start_pos to end_pos roughly using bytes difference
            let length = span.end.saturating_add(1).saturating_sub(span.start);

            // Map script token variants to our semantic token legend indices.
            let token_type: u32 = match st.tok {
                ScriptToken::Def | ScriptToken::End => SemanticTokenType::Macro.as_u32(),
                ScriptToken::Keyword(kw) if kw.is_sect() => SemanticTokenType::Class.as_u32(),
                ScriptToken::Keyword(_) => SemanticTokenType::Keyword.as_u32(),
                ScriptToken::Str(_) | ScriptToken::Char(_) => SemanticTokenType::String.as_u32(),
                ScriptToken::BoolLiteral(_) => SemanticTokenType::String.as_u32(),
                ScriptToken::Integer(_, _) | ScriptToken::Float(_, _) => {
                    SemanticTokenType::Number.as_u32()
                }
                ScriptToken::Id(id) => {
                    let next_is_paren = i + 1 < toks_vec.len()
                        && matches!(toks_vec[i + 1].tok, ScriptToken::OParen);
                    let entity = state.get_entity_at_offset(span.start as usize);
                    if let Some(ty) = classify_id_token(compiler, entity, id.id, next_is_paren) {
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
                | ScriptToken::SlimArrow
                | ScriptToken::StaticAccess => SemanticTokenType::Operator.as_u32(),
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

        let Some(text) = self.get_document_text(&uri.to_string()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };

        let state = state_arc.read();
        let byte_offset = crate::text::position_to_offset(&state.text, pos);

        if state.offset_in_comment(byte_offset) {
            return Ok(None);
        }

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
                let start_pos = crate::text::offset_to_position(&t_text, def_span.start as usize);
                let end_pos = crate::text::offset_to_position(&t_text, (def_span.end + 1) as usize);

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
        let Some(state_arc) = self.get_state(uri) else {
            return Ok(None);
        };

        let state_guard = match state_arc.try_read_for(Duration::from_millis(500)) {
            Some(g) => g,
            None => return Ok(None),
        };
        let state = &*state_guard;

        let byte_off =
            crate::text::position_to_offset(&state.text, params.text_document_position.position);
        let (start_b, _end_b) = crate::text::find_word_bounds(&state.text, byte_off);
        let prefix = &state.text[start_b..byte_off.min(state.text.len())];

        // Determine the script section boundaries from cached state
        let script_start = state.script_start;
        let serial_start = state.serial_start.unwrap_or(state.text.len());
        let in_script_section = byte_off >= script_start && byte_off < serial_start;

        // If cursor is outside the script section, return no completions
        if !in_script_section || state.offset_in_comment(byte_off) {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        }

        let is_dot_completion = start_b > 0 && state.text.as_bytes()[start_b - 1] == b'.';
        let mut dot_target_name = None;
        if is_dot_completion {
            let (t_start, t_end) = crate::text::find_word_bounds(&state.text, start_b - 1);
            if t_start < t_end {
                dot_target_name = Some(&state.text[t_start..t_end]);
            }
        }

        let is_static_access_completion = byte_off >= 2
            && state.text.as_bytes()[byte_off - 2] == b':'
            && state.text.as_bytes()[byte_off - 1] == b':';
        let mut static_target_name = None;
        if is_static_access_completion {
            let (t_start, t_end) = crate::text::find_word_bounds(&state.text, byte_off - 2);
            if t_start < t_end {
                static_target_name = Some(&state.text[t_start..t_end]);
            }
        }

        let target_name = dot_target_name.or(static_target_name);

        if let Some(target_name) = target_name {
            let mut items: Vec<CompletionItem> = Vec::new();
            if let Some(target_id) = state.interner.try_search_str(target_name) {
                if let Some(compiler) = &state.compiler {
                    if let Some(module) = compiler.mods.iter().find(|m| m.name_id == target_id) {
                        if module.mod_id.id == 0 {
                            // Current module: show all symbols except ScopeType::Var
                            for sym in &compiler.symbols {
                                if (matches!(sym.sym_origin, compilation::semantic::hir::hir_concepts::SymbolOrigin::Module(mid) if mid.id == 0) || matches!(sym.sym_origin, compilation::semantic::hir::hir_concepts::SymbolOrigin::Compiler)) && sym.scope_origin != scopes::ScopeType::Var {
                                    let sym_name = state.interner.search(sym.name_id);
                                    if prefix.is_empty() || sym_name.starts_with(prefix) {
                                        let kind = symbol_completion_kind(compiler, sym);
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
                                    let sym_name = state.interner.search(sym.name_id);
                                    if prefix.is_empty() || sym_name.starts_with(prefix) {
                                        let kind = symbol_completion_kind(compiler, sym);
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
            return Ok(Some(CompletionResponse::Array(items)));
        }

        // Language keywords and argument annotations (intrinsic types/functions come from core module exports)
        const SUGGESTIONS: &[(&str, CompletionItemKind)] = &[
            ("@def", CompletionItemKind::SNIPPET),
            ("@end", CompletionItemKind::SNIPPET),
            ("bind", CompletionItemKind::KEYWORD),
            ("import", CompletionItemKind::KEYWORD),
            ("export", CompletionItemKind::KEYWORD),
            ("alias", CompletionItemKind::KEYWORD),
            ("let", CompletionItemKind::KEYWORD),
            ("in", CompletionItemKind::KEYWORD),
            ("as", CompletionItemKind::KEYWORD),
            ("var->", CompletionItemKind::KEYWORD),
            ("nest->", CompletionItemKind::KEYWORD),
            ("complex->", CompletionItemKind::KEYWORD),
            ("override->", CompletionItemKind::KEYWORD),
            ("struct", CompletionItemKind::KEYWORD),
            ("enum", CompletionItemKind::KEYWORD),
            ("change", CompletionItemKind::KEYWORD),
            ("List", CompletionItemKind::STRUCT),
            ("Set", CompletionItemKind::STRUCT),
            ("Map", CompletionItemKind::STRUCT),
            ("Tuple", CompletionItemKind::STRUCT),
            ("true", CompletionItemKind::CONSTANT),
            ("false", CompletionItemKind::CONSTANT),
            ("#warn", CompletionItemKind::CONSTRUCTOR),
            ("#bin", CompletionItemKind::CONSTRUCTOR),
            ("#octal", CompletionItemKind::CONSTRUCTOR),
            ("#scient", CompletionItemKind::CONSTRUCTOR),
            ("#hex", CompletionItemKind::CONSTRUCTOR),
            ("#ignore", CompletionItemKind::CONSTRUCTOR),
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

        //TODO: Should auto-complete any module that has a src of "None"
        // Add core library exports (types, functions, and constants from the core module)
        if let Some(compiler) = &state.compiler {
            if let Some(core_mod) = compiler
                .mods
                .get(compiler.intrinsic_registry.core_mod_id.id)
            {
                for sym_id in &core_mod.exports {
                    if let Some(sym) = compiler.symbols.get(sym_id.id as usize) {
                        let name = state.interner.search(sym.name_id);
                        if prefix.is_empty() || name.starts_with(prefix) {
                            let kind = symbol_completion_kind(compiler, sym);
                            items.push(CompletionItem {
                                label: name.to_string(),
                                kind: Some(kind),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        // Add all modules (using already-analyzed compiler state)
        if let Some(compiler) = &state.compiler {
            let current_module = &compiler.mods[0];
            for module in &compiler.mods {
                let name = state.interner.search(module.name_id);

                let is_self = module.mod_id.id == 0;
                let is_imported = current_module.imports.iter().any(|i| {
                    i.name_id == module.name_id || i.alias_id.map_or(false, |a| a == module.name_id)
                });

                if !is_imported && !is_self {
                    continue;
                }

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
            if (tok_end as usize) > serial_start {
                continue;
            }

            if let ScriptToken::Id(id) = st.tok {
                let name = state.interner.search(id);
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
