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

use compilation::lexer::token::Token as ScriptToken;
use compilation::lookup::scopes;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::hir_concepts::{
    Symbol, SymbolKind, SymbolOrigin, Type, VariableState,
};
use lang::config_loader::{ConfigLoader, ConfigLoaderOutput};
use parking_lot::RwLock;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tower_lsp::lsp_types::{CompletionItemKind, Position, SemanticToken};
use tower_lsp::{Client, LanguageServer, jsonrpc};

use chrn_utils::id_types::ModuleId;

use crate::analyser::analyze_and_publish_task;
use crate::analyser::resolve_document_modules;
use crate::state::DocumentCache;
use crate::state::DocumentState;
use crate::text::apply_text_change;

// Semantic token support (keyword/string/number highlighting)
use crate::state::SemanticEntity;
use chrn_utils::chrn_config::ChrnConfig;
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
/// to the client.  **The order must match the legend in [`Backend::initialize`](Backend::initialize).**
/// If a variant is added or removed here, the corresponding entry must be added or
/// removed in the `token_types` vec inside `initialize()`, and vice versa.
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
    Regexp,
    Comment,
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
            SemanticTokenType::Regexp => 11,
            SemanticTokenType::Comment => 12,
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
        let chrn_cfg = ChrnConfig::default();

        let mut interner = Intern::init();
        let path_id = interner.intern_path(&path_buf);
        let region = match ConfigLoader::new(
            chrn_utils::id_types::SourceRegionId::new(0),
            Cursor::new(text.as_bytes()),
            path_id,
            &chrn_cfg,
            &interner,
        )
        .load_config()
        {
            ConfigLoaderOutput::Success(region) => region,
            ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                publish_config_load_error(self.client.clone(), uri.clone(), &text, cfg_err);
                broken_region
            }
            ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                publish_config_load_error(self.client.clone(), uri.clone(), &text, cfg_err);
                return None;
            }
        };

        // SAFETY: Pass the real per-URI version, not `0`. The previous
        // hardcoded `0` made the synchronous `get_analyzed_state` path win
        // the version race every time: the async debounced task's
        // `publish_if_current` saw `0 != my_version` and silently dropped
        // every subsequent diagnostics publish — which is exactly why
        // diagnostics "stop working" after the first synchronous request
        // (hover, goto, etc.) on a file.
        let my_version = self.bump_version(&uri_str);

        // Resolve imported modules outside the DocumentState write lock.  This is
        // the same pre-analysis step used by the async analysis task and prevents
        // the deadlock where `ensure_analyzed` held the per-document lock while
        // calling `DocumentCache::get_text`.
        let prepared = resolve_document_modules(
            uri,
            Arc::clone(&text),
            region.script_start,
            region.serial_start,
            &chrn_cfg,
            &self.doc_cache,
            my_version,
        );
        let imported_uris = prepared.resolution.imported_uris.clone();

        let state_arc = self.doc_cache.insert_or_get(
            &uri_str,
            Arc::clone(&text),
            prepared.state,
        );

        {
            let mut state = state_arc.write();
            state.ensure_analyzed(prepared.resolution);
        }

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
        let text = self.get_document_text(uri.as_ref())?;
        self.get_analyzed_state(uri, text)
    }

    /// Atomically increments the version counter for `uri`, returning the new value.
    fn bump_version(&self, uri: &str) -> u64 {
        let mut vers = self.pending_versions.write();
        let v = vers.entry(uri.to_string()).or_insert(0);
        *v = v.wrapping_add(1);
        *v
    }

    /// Applies LSP content changes to a document, updating the docs map.
    /// Returns the new `Arc<String>` on success, or shows an error message
    /// and returns `None` on failure.
    fn apply_content_changes(
        &self,
        params: &tower_lsp::lsp_types::DidChangeTextDocumentParams,
        uri_str: &str,
    ) -> Option<Arc<String>> {
        let mut docs = self.docs.write();
        let existing = docs.remove(uri_str).unwrap_or_default();
        let mut updated = (*existing).clone();
        for change in params.content_changes.iter() {
            match apply_text_change(&updated, change) {
                Ok(next) => updated = next,
                Err(_e) => {
                    docs.insert(uri_str.to_string(), existing);
                    return None;
                }
            }
        }
        let updated_arc = Arc::new(updated);
        docs.insert(uri_str.to_string(), Arc::clone(&updated_arc));
        Some(updated_arc)
    }

    /// Ensures the file where a symbol is defined is analyzed and cached,
    /// enabling cross-module operations (rename, references) to find
    /// occurrences in the definition file even when it hasn't been opened.
    async fn ensure_definition_file_analyzed(&self, def_path_str: &str) {
        let def_path = std::path::Path::new(def_path_str);
        if let Ok(def_uri) = tower_lsp::lsp_types::Url::from_file_path(def_path)
            && let Ok(text) = tokio::fs::read_to_string(def_path).await
        {
            self.get_analyzed_state(&def_uri, Arc::new(text));
        }
    }
}

/// Returns the [`CompletionItemKind`] that best represents the symbol for completion.
///
/// Used by the completion handler to assign icons to items shown in the editor UI.
fn symbol_completion_kind(compiler: &ScriptCompiler, sym: &Symbol) -> CompletionItemKind {
    match sym.kind {
        SymbolKind::Type(type_id) => match &compiler.types[type_id].ty {
            Type::Struct(_) | Type::Enum(_) | Type::TypeDef(_) | Type::BuiltinType(_) => {
                CompletionItemKind::STRUCT
            }
            Type::Alias(_) => CompletionItemKind::FUNCTION,
            Type::Func(func_def) if func_def.is_callable => CompletionItemKind::FUNCTION,
            Type::Func(_) => CompletionItemKind::CONSTANT,
            Type::Unknown | Type::Boundaries(_) | Type::Deferred(_) => CompletionItemKind::VARIABLE,
        },
        SymbolKind::Variable(var_id) => {
            let var = &compiler.variables[var_id];
            let VariableState::Known(val_id) = var.state else {
                return CompletionItemKind::VARIABLE;
            };
            let type_id = compiler.values[val_id].type_id;
            match &compiler.types[type_id].ty {
                Type::BuiltinType(_) | Type::Struct(_) | Type::TypeDef(_) | Type::Enum(_) => {
                    CompletionItemKind::VARIABLE
                }
                Type::Alias(_) => CompletionItemKind::FUNCTION,
                Type::Func(func_def) if func_def.is_callable => CompletionItemKind::FUNCTION,
                Type::Func(_) => CompletionItemKind::CONSTANT,
                Type::Unknown | Type::Boundaries(_) | Type::Deferred(_) => {
                    CompletionItemKind::VARIABLE
                }
            }
        }
        SymbolKind::Module(_) => CompletionItemKind::MODULE,
        SymbolKind::Config(_) => CompletionItemKind::CLASS,
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
                // `entity: &SemanticEntity` so `sym_id: &SymbolId` — dereference
                // before passing to `Arena::get`, which takes the id by value.
                if let Some(sym) = compiler.symbols.get(*sym_id) {
                    match sym.kind {
                        SymbolKind::Type(tid) => {
                            let ty = &compiler.types[tid].ty;
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
                                Type::Unknown | Type::Boundaries(_) | Type::Deferred(_) => {
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
                        SymbolKind::Config(_) => {
                            return Some(SemanticTokenType::Class.as_u32());
                        }
                        SymbolKind::Directive(_) => {
                            return Some(SemanticTokenType::Regexp.as_u32());
                        }
                    }
                }
            }
            SemanticEntity::Field { .. }
            | SemanticEntity::Variant { .. }
            | SemanticEntity::ConfigMember { .. }
            | SemanticEntity::ConfigOption { .. } => {
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
                            // **The order must match the local `SemanticTokenType` enum.**
                            // When adding/removing a variant in that enum, update this
                            // vec to match.
                            token_types: vec![
                                tower_lsp::lsp_types::SemanticTokenType::KEYWORD, // 0
                                tower_lsp::lsp_types::SemanticTokenType::STRING,  // 1
                                tower_lsp::lsp_types::SemanticTokenType::NUMBER,  // 2
                                tower_lsp::lsp_types::SemanticTokenType::TYPE,    // 3
                                tower_lsp::lsp_types::SemanticTokenType::FUNCTION, // 4
                                tower_lsp::lsp_types::SemanticTokenType::MACRO,   // 5
                                tower_lsp::lsp_types::SemanticTokenType::OPERATOR, // 6
                                tower_lsp::lsp_types::SemanticTokenType::VARIABLE, // 7
                                tower_lsp::lsp_types::SemanticTokenType::PROPERTY, // 8
                                tower_lsp::lsp_types::SemanticTokenType::CLASS,   // 9
                                tower_lsp::lsp_types::SemanticTokenType::ENUM_MEMBER, // 10
                                tower_lsp::lsp_types::SemanticTokenType::REGEXP,  // 11
                                tower_lsp::lsp_types::SemanticTokenType::COMMENT, // 12
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
        let Some(updated_arc) = self.apply_content_changes(&params, &uri_str) else {
            let _ = self
                .client
                .show_message(
                    tower_lsp::lsp_types::MessageType::ERROR,
                    "Failed to apply text change",
                )
                .await;
            return;
        };

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
                matches!(vers.get(&inner_uri_str), Some(&v) if v == my_version)
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
        let Some(text) = self.get_document_text(uri.as_ref()) else {
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

        let Some(text) = self.get_document_text(uri.as_ref()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };

        let def_path_str = {
            let state = state_arc.read();
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            if state.offset_in_comment(byte_offset) {
                None
            } else {
                state.get_entity_at_offset(byte_offset).and_then(|e| {
                    if matches!(e, SemanticEntity::Local { .. } | SemanticEntity::Module(_)) {
                        return None;
                    }
                    let (def_path, _, _) = state.get_definition_location(e)?;
                    let path = std::path::Path::new(&def_path);
                    if path == uri.path() {
                        return None;
                    }
                    Some(def_path)
                })
            }
        };

        if let Some(ref def_path_str) = def_path_str {
            self.ensure_definition_file_analyzed(def_path_str).await;
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

        let Some(text) = self.get_document_text(uri.as_ref()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };

        let def_path_str = {
            let state = state_arc.read();
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            if state.offset_in_comment(byte_offset) {
                None
            } else {
                state.get_entity_at_offset(byte_offset).and_then(|e| {
                    if matches!(e, SemanticEntity::Local { .. } | SemanticEntity::Module(_)) {
                        return None;
                    }
                    let (def_path, _, _) = state.get_definition_location(e)?;
                    let path = std::path::Path::new(&def_path);
                    if path == uri.path() {
                        return None;
                    }
                    Some(def_path)
                })
            }
        };

        if let Some(ref def_path_str) = def_path_str {
            self.ensure_definition_file_analyzed(def_path_str).await;
        }

        let refs = crate::references::compute_references(&uri, pos, &self.doc_cache);
        Ok(refs)
    }

    async fn semantic_tokens_full(
        &self,
        params: tower_lsp::lsp_types::SemanticTokensParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(text) = self.get_document_text(uri.as_ref()) else {
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

        fn push_semantic_token(
            tokens: &mut Vec<SemanticToken>,
            prev_line: &mut u32,
            prev_start: &mut u32,
            first: &mut bool,
            start_pos: Position,
            length: u32,
            token_type: u32,
        ) {
            let (delta_line, delta_start) = if *first {
                *first = false;
                (start_pos.line, start_pos.character)
            } else if start_pos.line == *prev_line {
                (0, start_pos.character.saturating_sub(*prev_start))
            } else {
                (
                    start_pos.line.saturating_sub(*prev_line),
                    start_pos.character,
                )
            };
            *prev_line = start_pos.line;
            *prev_start = start_pos.character;
            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type,
                token_modifiers_bitset: 0,
            });
        }

        // Interleave tokens and comment trivia in strictly increasing file order.
        //
        // The LSP semantic-tokens protocol uses delta encoding, which means every
        // token's position is expressed relative to the *previous* emitted token.
        // If we emitted all regular tokens first and all comment trivia second,
        // a comment on line 0 would be delta-encoded relative to a token on line 10,
        // producing a nonsensical result (saturating arithmetic would place it on
        // line 10 instead of line 0).  The client would then render the comment
        // at the wrong location.
        //
        // To work around this, we merge the two sources into a single file-order
        // pass using two index pointers.  At each step we pick whichever span
        // comes first in the source, skipping non-comment trivia (whitespace,
        // newlines, tabs) which carry no semantic meaning.
        let mut tok_idx = 0;
        let mut triv_idx = 0;

        loop {
            let emit_comment = triv_idx < state.trivia.len()
                && (tok_idx >= toks_vec.len()
                    || state.trivia[triv_idx].span.start <= toks_vec[tok_idx].span.start);

            if emit_comment {
                let triv = &state.trivia[triv_idx];
                triv_idx += 1;
                if !triv.kind.is_comment() {
                    continue;
                }
                let start_pos =
                    crate::text::offset_to_position(&state.text, triv.span.start as usize);
                let length = triv.span.end.saturating_sub(triv.span.start);

                push_semantic_token(
                    &mut tokens,
                    &mut prev_line,
                    &mut prev_start,
                    &mut first,
                    start_pos,
                    length,
                    SemanticTokenType::Comment.as_u32(),
                );
            } else if tok_idx < toks_vec.len() {
                let st = &toks_vec[tok_idx];
                tok_idx += 1;
                let span = st.span;
                let start_pos = crate::text::offset_to_position(&state.text, span.start as usize);
                let length = span.end.saturating_sub(span.start);

                let token_type: u32 = match st.tok {
                    ScriptToken::Def | ScriptToken::End => SemanticTokenType::Macro.as_u32(),
                    ScriptToken::Keyword(kw) if kw.is_sect() => SemanticTokenType::Class.as_u32(),
                    ScriptToken::Keyword(_) => SemanticTokenType::Keyword.as_u32(),
                    ScriptToken::Str(_) | ScriptToken::Char(_) => {
                        SemanticTokenType::String.as_u32()
                    }
                    ScriptToken::BoolLiteral(_) => SemanticTokenType::String.as_u32(),
                    ScriptToken::Integer(_, _) | ScriptToken::Float(_, _) => {
                        SemanticTokenType::Number.as_u32()
                    }
                    ScriptToken::Id(id) => {
                        let next_is_paren = tok_idx < toks_vec.len()
                            && matches!(toks_vec[tok_idx].tok, ScriptToken::OParen);
                        let entity = state.get_entity_at_offset(span.start as usize);
                        if let Some(ty) = classify_id_token(compiler, entity, id.id, next_is_paren)
                        {
                            ty
                        } else {
                            continue;
                        }
                    }
                    ScriptToken::At => SemanticTokenType::Macro.as_u32(),
                    ScriptToken::HashSymbol => SemanticTokenType::Regexp.as_u32(),
                    ScriptToken::Assign
                    | ScriptToken::EqualTo
                    | ScriptToken::Walrus
                    | ScriptToken::Comma
                    | ScriptToken::DotRangeInclusive
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

                push_semantic_token(
                    &mut tokens,
                    &mut prev_line,
                    &mut prev_start,
                    &mut first,
                    start_pos,
                    length,
                    token_type,
                );
            } else {
                break;
            }
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

        let Some(text) = self.get_document_text(uri.as_ref()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text) else {
            return Ok(None);
        };

        let def_path_str = {
            let state = state_arc.read();
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            if state.offset_in_comment(byte_offset) {
                None
            } else {
                state.get_entity_at_offset(byte_offset).and_then(|entity| {
                    state
                        .get_definition_location(entity)
                        .map(|(dp, ds, _)| (dp, ds))
                })
            }
        };

        if def_path_str.is_none() {
            return Ok(None);
        }

        let mut links: Vec<LocationLink> = Vec::new();

        if let Some((def_path, def_span)) = def_path_str {
            let target_uri = match Url::from_file_path(&def_path) {
                Ok(u) => u,
                Err(_) => uri.clone(),
            };

            // We need the text of the target file to convert span to position
            let target_text = if def_path == uri.path() {
                let state = state_arc.read();
                Some(Arc::clone(&state.text))
            } else {
                let target_uri_str = target_uri.to_string();
                let from_cache = self
                    .doc_cache
                    .get_text(&target_uri_str)
                    .or_else(|| self.docs.read().get(&target_uri_str).map(Arc::clone));
                match from_cache {
                    Some(t) => Some(t),
                    None => tokio::fs::read_to_string(&def_path)
                        .await
                        .ok()
                        .map(Arc::new),
                }
            };

            if let Some(t_text) = target_text {
                let start_pos = crate::text::offset_to_position(&t_text, def_span.start as usize);
                let end_pos = crate::text::offset_to_position(&t_text, def_span.end as usize);

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

        // Single `:` should not trigger completions; only `::` (static access) should.
        // Without this guard, typing the first colon of `::` fires the trigger character
        // and falls through to the normal path, showing irrelevant results.
        if byte_off > 0
            && state.text.as_bytes()[byte_off - 1] == b':'
            && !(byte_off >= 2 && state.text.as_bytes()[byte_off - 2] == b':')
        {
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
            if let Some(target_id) = state.interner.try_search_str(target_name)
                && let Some(compiler) = &state.compiler
                && let Some(module) = compiler.mods.iter().find(|m| m.name_id == target_id)
            {
                if module.mod_id.id == 0 {
                    // Current module: show all symbols except ScopeType::Var.
                    // `Arena` does not implement `IntoIterator`; iterate over its
                    // inner `items` vector.
                    for sym in &compiler.symbols.items {
                        if (matches!(sym.sym_origin, SymbolOrigin::Module(mid) if mid.id == 0)
                            || matches!(sym.sym_origin, SymbolOrigin::Compiler))
                            && sym.scope_origin != scopes::ScopeType::Var
                            && !matches!(sym.kind, SymbolKind::Directive(_))
                        {
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
                        // `sym_id: &SymbolId` (from iterating over `Vec<SymbolId>`),
                        // dereference before calling `Arena::get`.
                        if let Some(sym) = compiler.symbols.get(*sym_id) {
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
        if let Some(compiler) = &state.compiler
            && let Some(core_mod) = compiler
                .mods
                // `core_mod_id` is already a `ModuleId` — pass it directly.
                .get(compiler.intrinsic_registry.core_mod_id)
        {
            for sym_id in &core_mod.exports {
                // `sym_id: &SymbolId` — dereference for the typed `Arena::get` call.
                if let Some(sym) = compiler.symbols.get(*sym_id) {
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

        // Add compiler-origin directives (e.g. #warn, #ignore, #scient, etc.)
        // Read dynamically from the compiler symbol registry instead of hard-coding.
        if let Some(compiler) = &state.compiler {
            // `Arena` is not an iterator; iterate over the inner `items` vec.
            for sym in &compiler.symbols.items {
                if matches!(sym.kind, SymbolKind::Directive(_)) {
                    let name = state.interner.search(sym.name_id);
                    let label = format!("#{}", name);
                    if prefix.is_empty() || label.starts_with(prefix) {
                        items.push(CompletionItem {
                            label,
                            kind: Some(symbol_completion_kind(compiler, sym)),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        // Add all modules (using already-analyzed compiler state)
        if let Some(compiler) = &state.compiler {
            // Index the `Arena` with a typed `ModuleId` (the only impl the
            // primary `Index` for `Arena` provides).
            let current_module = &compiler.mods[ModuleId::new(0)];
            // `Arena` itself is not iterable; iterate over the inner `items` vec.
            for module in &compiler.mods.items {
                let name = state.interner.search(module.name_id);

                let is_self = module.mod_id.id == 0;
                let is_imported = current_module
                    .imports
                    .iter()
                    .any(|i| i.name_id == module.name_id || i.alias_id == Some(module.name_id));

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
