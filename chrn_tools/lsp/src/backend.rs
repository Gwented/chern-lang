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
//! `pending_versions` stores the current globally unique generation per URI. On
//! `did_change` a new generation is assigned and a 150 ms sleep is awaited before
//! analysis runs.  If another change arrives before the timer fires, the previous
//! task is aborted (`pending_tasks`) and the new one takes over. Open and save
//! analyses are tracked the same way. Analysis results carry the generation they
//! were spawned for; stale results are discarded before cache mutation and in
//! [`crate::analyser::publish_if_current`].

use chrn_utils::source_map::source_span::SourceSpan;
use compilation::config_loader::{ConfigLoader, ConfigLoaderOutput};
use compilation::lexer::token::Token as ScriptToken;
use compilation::lookup::scopes::{self, scopes_concepts};
use compilation::module::module_concepts::ModuleState;
use compilation::parser::ast::ast_concepts::AbstractConfigKind;
use compilation::script_compiler::ScriptCompiler;
use compilation::semantic::hir::hir_concepts::Type;
use compilation::semantic::hir::hir_impls::{
    ConfigMemberMetadataKind, ConfigRoot, ConfigRootMetadataKind, ImplHirKind, ImplMemberKind,
};
use compilation::semantic::hir::hir_symbols::{Symbol, SymbolKind, VariableState};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, GotoDefinitionResponse, LocationLink,
    Position, Range, SemanticToken, Url,
};
use tower_lsp::{Client, LanguageServer, jsonrpc};

use chrn_utils::id_types::{ImplMemberId, InternedId, ModuleId, ScopeId, SymbolId, TypeId};
use lang::config_schemas::{ConfigSchemaKind, get_cfg_schema};

use crate::analyser::analyze_and_publish_task;
use crate::analyser::{dependency_snapshots_are_current, resolve_document_modules};
use crate::state::{DocumentCache, DocumentState, STATE_LOCK_TIMEOUT};
use crate::text::apply_text_change;

// Semantic token support (keyword/string/number highlighting)
use crate::state::SemanticEntity;
use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::SourceDiagnosticSummary;
use lang::types::builtins::BuiltinTypeKind as ChBuiltinTypeKind;
use std::io::Cursor;

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
#[derive(Clone, Debug)]
pub struct Backend {
    pub client: Client,
    /// Raw document texts keyed by URI string, kept in sync with editor state.
    pub docs: Arc<RwLock<HashMap<String, Arc<String>>>>,
    /// Current globally unique analysis generation for each open URI.
    pub pending_versions: Arc<RwLock<HashMap<String, u64>>>,
    /// Process-wide source of unique analysis generations. Generations never
    /// repeat when a URI is closed and reopened.
    next_version: Arc<AtomicU64>,
    /// Hash of the last-published diagnostics per URI; used to suppress
    /// redundant `publishDiagnostics` notifications.  Only an 8-byte digest is
    /// stored per document rather than the full JSON payload.
    pub diags_cache: Arc<RwLock<HashMap<String, u64>>>,
    /// Latest open, save, or debounced-change analysis task for each URI.
    pub pending_tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    /// Bounds detached blocking compiler jobs when an async task is aborted.
    analysis_slots: Arc<tokio::sync::Semaphore>,
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
            next_version: Arc::new(AtomicU64::new(1)),
            diags_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_tasks: Arc::new(RwLock::new(HashMap::new())),
            analysis_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            doc_cache: Arc::new(DocumentCache::new(50)),
        }
    }

    /// Blocking half of feature-driven analysis. Call only through the bounded
    /// [`Self::get_analyzed_state`] wrapper.
    ///
    /// Preferred over spawning a task when the caller already holds the document text
    /// (e.g. in hover / definition / references / rename handlers that need the state
    /// before they can respond).
    ///
    /// Returns `None` if the document header cannot be parsed or a state lock does
    /// not become available within the interactive request budget.
    fn get_analyzed_state_blocking(
        &self,
        uri: &tower_lsp::lsp_types::Url,
        text: Arc<String>,
        expected_generation: Option<u64>,
    ) -> Option<Arc<RwLock<crate::state::DocumentState>>> {
        let uri_str = uri.to_string();

        // Try to get existing analyzed state first
        if let Some(state_arc) = self.doc_cache.get(&uri_str) {
            let cached_text_matches = self
                .doc_cache
                .get_text(&uri_str)
                .is_some_and(|cached| Arc::ptr_eq(&cached, &text) || *cached == *text);
            if !cached_text_matches {
                // Continue below and replace the stale cache entry.
            } else {
                let needs_analysis = match state_arc.try_read_for(STATE_LOCK_TIMEOUT) {
                    Some(state) => state.compiler.is_none() || *state.text != *text,
                    None => return Some(Arc::clone(&state_arc)),
                };

                if !needs_analysis {
                    return Some(state_arc);
                }
            }
        }

        // Shared with `resolve_document_modules` so both entry points intern the
        // same string: `Url::path` stays percent-encoded, and interning that form
        // here gave the same file two `PathId`s depending on which entry point
        // analyzed it.
        let path_buf = crate::analyser::uri_to_path(uri);
        let mut chrn_cfg = ChrnConfig::default();

        let mut interner = Intern::init();
        let path_id = interner.intern_path(&path_buf);
        let mut cfg_loader_warns = SourceDiagnosticSummary::default();
        // Mirrors `extract_main`: a recovered-but-malformed region leaves the main
        // module in `BrokenRegion`, which later stages treat as "do not re-parse".
        let mut main_state = ModuleState::Loaded;
        let region = match ConfigLoader::new(
            chrn_utils::id_types::SourceRegionId::new(0),
            Cursor::new(text.as_bytes()),
            path_id,
            &chrn_cfg,
        )
        .load_config()
        {
            ConfigLoaderOutput::Success(region, summary) => {
                cfg_loader_warns = summary;
                region
            }
            ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                // Background analysis owns diagnostic publication. Feature-driven
                // analysis only needs a recoverable state.
                _ = cfg_err;
                main_state = ModuleState::BrokenRegion;
                broken_region
            }
            ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                // No region data is recoverable; the loader produced no
                // `script_start` so the default 0 keeps the relative span
                // shift a no-op (the file is treated as if it had no `@def`).
                _ = cfg_err;
                return None;
            }
        };

        // Read the current version WITHOUT bumping it.  This synchronous path
        // never publishes diagnostics itself, so it must not claim ownership of
        // the version counter: bumping it here invalidated the debounced
        // `did_change` task (whose `still_current` check compares versions),
        // which swallowed the diagnostics publish for that change entirely —
        // the editor then kept showing stale diagnostics until the next edit.
        // Peeking instead lets the pending debounced task run, hit the cache
        // this call just populated, and publish the fresh diagnostics exactly
        // once.  (The even older hardcoded `0` had the inverse problem: it
        // made `publish_if_current` in the async task see `0 != my_version`
        // and drop every publish.)
        let my_version = self
            .pending_versions
            .read()
            .get(&uri_str)
            .copied()
            .unwrap_or(0);

        // Resolve imported modules outside the DocumentState write lock.  This is
        // the same pre-analysis step used by the async analysis task and prevents
        // the deadlock where `ensure_analyzed` held the per-document lock while
        // calling `DocumentCache::get_text`.  The interner from the config load
        // is moved in so a second one is not allocated.
        let mut prepared = resolve_document_modules(
            uri,
            Arc::clone(&text),
            region,
            main_state,
            &mut chrn_cfg,
            &self.doc_cache,
            &self.docs,
            my_version,
            interner,
        );

        // Merge config-loader warnings from the Success path into the
        // resolution's config_errors so they are persisted through
        // ensure_analyzed and surface via get_lsp_diagnostics.
        if !cfg_loader_warns.diags.is_empty() {
            prepared
                .resolution
                .config_errors
                .append_summary(&mut cfg_loader_warns);
        }

        let state_arc = self.doc_cache.insert_or_get_when(
            &uri_str,
            Arc::clone(&text),
            prepared.state,
            || {
                if let Some(generation) = expected_generation {
                    let generation_matches = matches!(
                        self.pending_versions.read().get(&uri_str),
                        Some(&current) if current == generation
                    );
                    generation_matches
                        && dependency_snapshots_are_current(
                            &prepared.dependency_snapshots,
                            &self.docs,
                        )
                        && self.docs.read().get(&uri_str).is_some_and(|current| {
                            Arc::ptr_eq(current, &text) || **current == *text
                        })
                } else {
                    !self.docs.read().contains_key(&uri_str)
                        && dependency_snapshots_are_current(
                            &prepared.dependency_snapshots,
                            &self.docs,
                        )
                }
            },
        )?;

        let imported_uris = {
            let mut state = state_arc.try_write_for(STATE_LOCK_TIMEOUT)?;
            state.ensure_analyzed(prepared.resolution)
        };

        if !dependency_snapshots_are_current(&prepared.dependency_snapshots, &self.docs) {
            self.doc_cache.invalidate_if_state(&uri_str, &state_arc);
            return None;
        }

        if let Some(imported_uris) = imported_uris
            && !self.doc_cache.register_dependencies_for_state_when(
                &uri_str,
                &state_arc,
                &imported_uris,
                || dependency_snapshots_are_current(&prepared.dependency_snapshots, &self.docs),
            )
        {
            return None;
        }

        Some(state_arc)
    }

    async fn get_analyzed_state(
        &self,
        uri: &tower_lsp::lsp_types::Url,
        text: Arc<String>,
    ) -> Option<Arc<RwLock<crate::state::DocumentState>>> {
        let expected_generation = self.pending_versions.read().get(uri.as_ref()).copied();
        if let Some(state_arc) = self.doc_cache.get(uri.as_ref()) {
            let cached_text_matches = self
                .doc_cache
                .get_text(uri.as_ref())
                .is_some_and(|cached| Arc::ptr_eq(&cached, &text) || *cached == *text);
            if !cached_text_matches {
                // Continue into bounded analysis and replace the stale entry.
            } else {
                match state_arc.try_read() {
                    Some(state) if state.compiler.is_some() && *state.text == *text => {
                        drop(state);
                        return Some(Arc::clone(&state_arc));
                    }
                    // A background analysis already owns the state. Let the handler's
                    // bounded read decide whether it completes within the request budget.
                    None => return Some(Arc::clone(&state_arc)),
                    Some(_) => {}
                }
            }
        }

        let permit = tokio::time::timeout(
            STATE_LOCK_TIMEOUT,
            Arc::clone(&self.analysis_slots).acquire_owned(),
        )
        .await
        .ok()?
        .ok()?;
        let backend = self.clone();
        let uri = uri.clone();
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            backend.get_analyzed_state_blocking(&uri, text, expected_generation)
        });
        tokio::time::timeout(STATE_LOCK_TIMEOUT, task)
            .await
            .ok()?
            .ok()?
    }

    /// Looks up the current source text for `uri` from the `docs` map.
    fn get_document_text(&self, uri: &str) -> Option<Arc<String>> {
        self.docs.read().get(uri).map(Arc::clone)
    }

    /// Convenience: retrieves the document text and runs [`get_analyzed_state`](Self::get_analyzed_state).
    async fn get_state(
        &self,
        uri: &tower_lsp::lsp_types::Url,
    ) -> Option<Arc<RwLock<DocumentState>>> {
        let text = self.get_document_text(uri.as_ref())?;
        self.get_analyzed_state(uri, text).await
    }

    /// Atomically increments the version counter for `uri`, returning the new value.
    fn bump_version(&self, uri: &str) -> u64 {
        let next = self.next_version.fetch_add(1, Ordering::Relaxed);
        let mut vers = self.pending_versions.write();
        vers.insert(uri.to_string(), next);
        next
    }

    /// Applies LSP content changes to a document, updating the docs map.
    /// Returns the new `Arc<String>` on success, or shows an error message
    /// and returns `None` on failure.
    ///
    /// The previous implementation cloned the entire document up front and
    /// then allocated another full `String` per change event.  This version
    /// borrows the existing text for the first change and only allocates the
    /// strings `apply_text_change` itself produces — one full-document
    /// allocation fewer per `did_change` notification.
    fn apply_content_changes(
        &self,
        params: &tower_lsp::lsp_types::DidChangeTextDocumentParams,
        uri_str: &str,
    ) -> Option<Arc<String>> {
        let mut docs = self.docs.write();
        let existing = docs.remove(uri_str).unwrap_or_default();
        let mut updated: Option<String> = None;
        for change in params.content_changes.iter() {
            let current: &str = updated.as_deref().unwrap_or(&existing);
            match apply_text_change(current, change) {
                Ok(next) => updated = Some(next),
                Err(_e) => {
                    docs.insert(uri_str.to_string(), existing);
                    return None;
                }
            }
        }
        // No change events means the text is unchanged; reuse the existing Arc.
        let updated_arc = match updated {
            Some(u) => Arc::new(u),
            None => existing,
        };
        docs.insert(uri_str.to_string(), Arc::clone(&updated_arc));
        Some(updated_arc)
    }

    /// Ensures the file where a symbol is defined is analyzed and cached,
    /// enabling cross-module operations (rename, references) to find
    /// occurrences in the definition file even when it hasn't been opened.
    async fn ensure_definition_file_analyzed(&self, def_path: &std::path::Path) {
        if let Ok(def_uri) = tower_lsp::lsp_types::Url::from_file_path(def_path)
            && let Ok(text) = tokio::fs::read_to_string(def_path).await
        {
            self.get_analyzed_state(&def_uri, Arc::new(text)).await;
        }
    }

    /// Analyzes the file declaring the entity at `pos` when that file is a
    /// *different* one from `uri`, so the cross-module search that follows can see
    /// it even if the editor never opened it.
    ///
    /// Locals and modules are skipped: locals never cross files, and module
    /// rename/references are unsupported.
    async fn preload_definition_file(
        &self,
        state_arc: &RwLock<DocumentState>,
        uri: &tower_lsp::lsp_types::Url,
        pos: Position,
    ) {
        let def_path = {
            let Some(state) = state_arc.try_read_for(STATE_LOCK_TIMEOUT) else {
                return;
            };
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            if state.offset_in_comment(byte_offset) {
                None
            } else {
                state.get_entity_at_offset(byte_offset).and_then(|e| {
                    if matches!(e, SemanticEntity::Local { .. } | SemanticEntity::Module(_)) {
                        return None;
                    }
                    let (def_path, _, _) = state.definition_site(e)?;
                    if def_path == std::path::Path::new(uri.path()) {
                        return None;
                    }
                    Some(def_path.to_path_buf())
                })
            }
        };

        if let Some(def_path) = def_path {
            self.ensure_definition_file_analyzed(&def_path).await;
        }
    }
}

/// Returns the [`CompletionItemKind`] that best represents the symbol for completion.
///
/// Used by the completion handler to assign icons to items shown in the editor UI.
fn symbol_completion_kind(compiler: &ScriptCompiler, sym: &Symbol) -> CompletionItemKind {
    match sym.kind {
        SymbolKind::Type(type_id) => match &compiler.types[type_id].ty {
            Type::Struct(_) | Type::Enum(_) | Type::TypeDef(_) | Type::BuiltinTypeInfo(_) => {
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
                Type::BuiltinTypeInfo(_) | Type::Struct(_) | Type::TypeDef(_) | Type::Enum(_) => {
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
        SymbolKind::Namespace => match sym.associated_scope.expect("Should have namespace") {
            scopes_concepts::AssociatedScopeKind::Module(_) => CompletionItemKind::MODULE,
            scopes_concepts::AssociatedScopeKind::Scope(_) => CompletionItemKind::VARIABLE,
        },
        SymbolKind::Directive(_) => CompletionItemKind::KEYWORD,
        // Core exposes extern names as terminal type symbols even though it does
        // not yet attach a `TypeId` to them.
        SymbolKind::ExternType(_) => CompletionItemKind::CLASS,
    }
}

/// Collects symbols owned by `mod_id`'s qualified namespace.
///
/// Core's scope is injected into user modules for unqualified lexical lookup. A
/// qualified lookup uses `NamespaceOnly` and excludes it, so completion must do
/// the same or `main::i8` is advertised even though the compiler rejects it.
fn reachable_module_symbols(compiler: &ScriptCompiler, mod_id: ModuleId) -> Vec<SymbolId> {
    let mut symbols = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for scope_id in &compiler.mods[mod_id].scopes {
        if compiler.scopes[*scope_id].scope.scope_type == scopes_concepts::ScopeType::Core {
            continue;
        }
        for (_, sym_id) in compiler.scopes[*scope_id].scope.table.iter_interned() {
            if seen.insert(sym_id) {
                symbols.push(sym_id);
            }
        }
    }
    symbols
}

/// Resolves a visible symbol by interned name from the main module and returns its
/// namespace scope when the symbol carries one.
///
/// Used by static-access completion so `i32::` finds the built-in type's namespace
/// even though built-in types are not modules.
fn namespace_scope_of_visible_symbol(
    compiler: &ScriptCompiler,
    name_id: InternedId,
) -> Option<ScopeId> {
    for scope_type in [
        scopes_concepts::ScopeType::Var,
        scopes_concepts::ScopeType::Neutral,
    ] {
        let sym_id = scopes::find_sym_id(
            compiler,
            scopes_concepts::AssociatedScopeKind::Module(ModuleId::new(0)),
            name_id,
            scope_type,
            scopes_concepts::ScopeLookupPattern::NoRestrictions,
            scopes_concepts::ScopeLookupPreferenceFlags::none(),
        )?
        .found_sym_id;
        let sym = &compiler.syms[sym_id];
        if let Some(scopes_concepts::AssociatedScopeKind::Scope(scope_id)) = sym.associated_scope {
            return Some(scope_id);
        }
    }
    None
}

/// Resolves a compiler-provided namespace by name. Intrinsic namespaces such as
/// `JAVA::types::java` are not reachable from the ordinary module scope tables,
/// but they are valid inside override configuration paths.
fn namespace_scope_of_intrinsic_symbol(
    compiler: &ScriptCompiler,
    name_id: InternedId,
) -> Option<ScopeId> {
    compiler.syms.items.iter().find_map(|sym| {
        if sym.name_id != name_id || !matches!(sym.kind, SymbolKind::Namespace) {
            return None;
        }
        let Some(scopes_concepts::AssociatedScopeKind::Scope(scope_id)) = sym.associated_scope
        else {
            return None;
        };
        compiler.scopes[scope_id]
            .scope
            .is_intrinsic
            .then_some(scope_id)
    })
}

#[derive(Clone, Copy)]
enum CompletionNamespace {
    Module(ModuleId),
    Scope(ScopeId),
}

fn completion_namespace_for_symbol(sym: &Symbol) -> Option<CompletionNamespace> {
    match sym.associated_scope? {
        scopes_concepts::AssociatedScopeKind::Module(mod_id) => {
            Some(CompletionNamespace::Module(mod_id))
        }
        scopes_concepts::AssociatedScopeKind::Scope(scope_id) => {
            Some(CompletionNamespace::Scope(scope_id))
        }
    }
}

/// Extracts every namespace segment before the completion prefix from lexer
/// tokens. Walking tokens keeps comments and whitespace out of the path while
/// retaining the full `a::b::` chain instead of only the nearest identifier.
fn static_access_target_path(state: &DocumentState, prefix_start: usize) -> Vec<InternedId> {
    let relative_start = prefix_start.saturating_sub(state.script_start) as u32;
    let mut index = state
        .tokens
        .partition_point(|token| token.span.end <= relative_start);
    let mut reversed = Vec::new();

    loop {
        let Some(access) = index.checked_sub(1).and_then(|i| state.tokens.get(i)) else {
            break;
        };
        if !matches!(access.tok, ScriptToken::StaticAccess) {
            break;
        }
        index -= 1;

        let Some(segment) = index.checked_sub(1).and_then(|i| state.tokens.get(i)) else {
            break;
        };
        let ScriptToken::Id(name_id) = segment.tok else {
            break;
        };
        reversed.push(name_id);
        index -= 1;
    }

    reversed.reverse();
    reversed
}

fn module_symbol_for_completion(
    compiler: &ScriptCompiler,
    mod_id: ModuleId,
    name_id: InternedId,
) -> Option<SymbolId> {
    if mod_id == ModuleId::new(0) {
        return [
            scopes_concepts::ScopeType::Var,
            scopes_concepts::ScopeType::Neutral,
        ]
        .into_iter()
        .find_map(|scope_type| {
            scopes::find_sym_id(
                compiler,
                scopes_concepts::AssociatedScopeKind::Module(mod_id),
                name_id,
                scope_type,
                scopes_concepts::ScopeLookupPattern::NamespaceOnly,
                scopes_concepts::ScopeLookupPreferenceFlags::none(),
            )
            .map(|found| found.found_sym_id)
        });
    }

    compiler.mods[mod_id]
        .exports
        .iter()
        .copied()
        .find(|sym_id| compiler.syms[*sym_id].name_id == name_id)
}

fn resolve_completion_namespace_path(
    compiler: &ScriptCompiler,
    path: &[InternedId],
    allow_intrinsic_root: bool,
) -> Option<CompletionNamespace> {
    let (&head, tail) = path.split_first()?;
    let mut cursor = if let Some(mod_id) = crate::state::visible_module(compiler, head) {
        CompletionNamespace::Module(mod_id)
    } else if let Some(scope_id) = namespace_scope_of_visible_symbol(compiler, head).or_else(|| {
        allow_intrinsic_root
            .then(|| namespace_scope_of_intrinsic_symbol(compiler, head))
            .flatten()
    }) {
        CompletionNamespace::Scope(scope_id)
    } else {
        return None;
    };

    for &name_id in tail {
        let sym_id = match cursor {
            CompletionNamespace::Module(mod_id) => {
                module_symbol_for_completion(compiler, mod_id, name_id)?
            }
            CompletionNamespace::Scope(scope_id) => scopes::find_sym_id(
                compiler,
                scopes_concepts::AssociatedScopeKind::Scope(scope_id),
                name_id,
                scopes_concepts::ScopeType::Complex,
                scopes_concepts::ScopeLookupPattern::NamespaceOnly,
                scopes_concepts::ScopeLookupPreferenceFlags::none(),
            )
            .map(|found| found.found_sym_id)?,
        };
        cursor = completion_namespace_for_symbol(&compiler.syms[sym_id])?;
    }

    Some(cursor)
}

pub(crate) struct ConfigCompletionCandidate {
    pub(crate) open: u32,
    pub(crate) close: u32,
    pub(crate) name_start: u32,
    pub(crate) type_id: Option<TypeId>,
    pub(crate) scope_id: Option<ScopeId>,
    pub(crate) is_root: bool,
    pub(crate) configured_options: Vec<InternedId>,
    pub(crate) configured_members: Vec<InternedId>,
}

fn config_namespace_scope_id(
    state: &DocumentState,
    compiler: &ScriptCompiler,
    name_span: SourceSpan,
) -> Option<ScopeId> {
    state.symbol_map.iter().find_map(|(span, entity)| {
        let SemanticEntity::Symbol(sym_id) = entity else {
            return None;
        };
        let sym = &compiler.syms[*sym_id];
        (*span == name_span && matches!(sym.kind, SymbolKind::Namespace))
            .then(|| match sym.associated_scope {
                Some(scopes_concepts::AssociatedScopeKind::Scope(scope_id)) => Some(scope_id),
                _ => None,
            })
            .flatten()
    })
}

/// Map config delimiters to their closing brace. Shorthand arrows share their
/// enclosing block's close; child braces still establish their own scope.
/// Lexer tokens exclude delimiters inside strings and comments.
fn config_delimiter_pairs(state: &DocumentState) -> HashMap<u32, u32> {
    let mut stack = Vec::new();
    let mut pairs = HashMap::new();

    for token in &state.tokens {
        match token.tok {
            ScriptToken::OCurlyBracket | ScriptToken::NotSlimArrow => {
                stack.push((token.span.start, token.tok));
            }
            ScriptToken::CCurlyBracket => {
                while let Some((open, delimiter)) = stack.pop() {
                    pairs.insert(open, token.span.start);
                    if delimiter == ScriptToken::OCurlyBracket {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    pairs
}

fn config_block_bounds(
    state: &DocumentState,
    pairs: &HashMap<u32, u32>,
    name_end: u32,
) -> Option<(u32, u32)> {
    // The delimiter immediately follows the name in the lexer stream, including
    // when comments or whitespace intervene. Do not borrow a later child's brace.
    let from = state
        .tokens
        .partition_point(|token| token.span.start < name_end);
    let delimiter = state.tokens.get(from)?;
    let open = delimiter.span.start;
    let close = match delimiter.tok {
        ScriptToken::OCurlyBracket | ScriptToken::NotSlimArrow => pairs.get(&open).copied(),
        _ => return None,
    };

    // An incomplete config block has no closing token yet.  Treating the document end as its
    // boundary keeps completion useful while the user is typing the block.
    let close = close.unwrap_or_else(|| {
        (state.serial_start.unwrap_or(state.text.len()) - state.script_start) as u32
    });

    Some((open, close))
}

fn config_type_info(
    compiler: &ScriptCompiler,
    mut type_id: TypeId,
) -> Option<(Vec<(InternedId, CompletionItemKind)>, ConfigSchemaKind)> {
    for _ in 0..chrn_utils::MAX_LOOPS {
        match &compiler.types[type_id].ty {
            Type::Struct(struct_def) => {
                let members = struct_def
                    .fields
                    .iter()
                    .filter_map(|member_id| match &compiler.sym_members[*member_id] {
                        compilation::semantic::hir::hir_symbols::MemberSymbolKind::Field(field) => {
                            Some((field.name_id, CompletionItemKind::FIELD))
                        }
                        compilation::semantic::hir::hir_symbols::MemberSymbolKind::Variant(_) => {
                            None
                        }
                    })
                    .collect();
                return Some((members, ConfigSchemaKind::Struct));
            }
            Type::Enum(enum_def) => {
                let members = enum_def
                    .variants
                    .iter()
                    .filter_map(|member_id| match &compiler.sym_members[*member_id] {
                        compilation::semantic::hir::hir_symbols::MemberSymbolKind::Variant(
                            variant,
                        ) => Some((variant.name_id, CompletionItemKind::ENUM_MEMBER)),
                        compilation::semantic::hir::hir_symbols::MemberSymbolKind::Field(_) => None,
                    })
                    .collect();
                return Some((members, ConfigSchemaKind::Enum));
            }
            Type::TypeDef(type_def) => type_id = type_def.type_id,
            Type::Deferred(inner) => type_id = *inner,
            Type::BuiltinTypeInfo(_)
            | Type::Func(_)
            | Type::Alias(_)
            | Type::Boundaries(_)
            | Type::Unknown => return None,
        }
    }

    None
}

fn configured_option_names(
    compiler: &ScriptCompiler,
    option_ids: &[ImplMemberId],
) -> Vec<InternedId> {
    option_ids
        .iter()
        .filter_map(|member_id| match &compiler.impl_membs[*member_id] {
            ImplMemberKind::OptAssignmentRoot(option) => Some(option.name_id),
            ImplMemberKind::OptAssignmentMember(option) => Some(option.name_id),
            //TODO: `MultiTypeAssignment` assigns types, not config options, and is
            //unfinished in core.
            ImplMemberKind::ConfigMember(_)
            | ImplMemberKind::MultiTypeAssignment(_)
            | ImplMemberKind::Unknown { .. } => None,
        })
        .collect()
}

fn config_member_type_id(
    member: &compilation::semantic::hir::hir_impls::ConfigMember,
) -> Option<TypeId> {
    match &member.meta {
        ConfigMemberMetadataKind::Complex(meta) => meta.linked_memb_type_id,
        ConfigMemberMetadataKind::Override(_) => None,
    }
}

fn config_root_type_id(compiler: &ScriptCompiler, cfg_root: &ConfigRoot) -> Option<TypeId> {
    let sym_id = cfg_root.linked_sym_id?;
    match &compiler.syms[sym_id].kind {
        SymbolKind::Type(type_id) => Some(*type_id),
        _ => None,
    }
}

fn config_candidate_for_member(
    compiler: &ScriptCompiler,
    member_id: ImplMemberId,
    state: &DocumentState,
    pairs: &HashMap<u32, u32>,
    candidates: &mut Vec<ConfigCompletionCandidate>,
) {
    let ImplMemberKind::ConfigMember(member) = &compiler.impl_membs[member_id] else {
        return;
    };

    if let Some((open, close)) = config_block_bounds(state, pairs, member.common.name_span.end) {
        let configured_members = member
            .cfg_members
            .iter()
            .filter_map(|child_id| match &compiler.impl_membs[*child_id] {
                ImplMemberKind::ConfigMember(child) => Some(child.common.name_id),
                ImplMemberKind::OptAssignmentRoot(_)
                | ImplMemberKind::OptAssignmentMember(_)
                | ImplMemberKind::MultiTypeAssignment(_)
                | ImplMemberKind::Unknown { .. } => None,
            })
            .collect();

        candidates.push(ConfigCompletionCandidate {
            open,
            close,
            name_start: member.common.name_span.start,
            type_id: config_member_type_id(member),
            scope_id: config_namespace_scope_id(state, compiler, member.common.name_span),
            is_root: false,
            configured_options: configured_option_names(compiler, &member.ast_stmts),
            configured_members,
        });
    }

    for &child_id in &member.cfg_members {
        config_candidate_for_member(compiler, child_id, state, pairs, candidates);
    }
}

/// The source span of the name a config block is written under.
///
/// A root names a path (`mod::Type`), so its span runs from the first segment to the
/// last; a member names a single identifier.  `None` only for an empty root path,
/// which a parse error can leave behind.
fn cfg_kind_name_span(kind: &AbstractConfigKind) -> Option<SourceSpan> {
    match kind {
        AbstractConfigKind::Root(path, _) => {
            let first = path.first()?;
            let last = path.last()?;
            Some(SourceSpan::new(
                first.span.region_id,
                first.span.start,
                last.span.end,
            ))
        }
        AbstractConfigKind::Member(sp_name_id, _) => Some(sp_name_id.span),
    }
}

fn cursor_in_override_config(
    state: &DocumentState,
    compiler: &ScriptCompiler,
    byte_off: usize,
) -> bool {
    let Some(ast) = state.asts.first().and_then(Option::as_ref) else {
        return false;
    };
    let Some(main_units) = state.compilation_syms.first().and_then(Option::as_ref) else {
        return false;
    };
    let pairs = config_delimiter_pairs(state);
    let relative_cursor = byte_off.saturating_sub(state.script_start) as u32;

    fn contains_override(
        cfg: &compilation::parser::ast::ast_concepts::AbstractConfig,
        state: &DocumentState,
        pairs: &HashMap<u32, u32>,
        relative_cursor: u32,
    ) -> bool {
        let Some(name_span) = cfg_kind_name_span(&cfg.kind) else {
            return false;
        };
        let Some((open, close)) = config_block_bounds(state, pairs, name_span.end) else {
            return false;
        };
        if !(open <= relative_cursor && relative_cursor < close) {
            return false;
        }

        let is_override = matches!(
            &cfg.kind,
            AbstractConfigKind::Root(_, ConfigRootMetadataKind::Override)
                | AbstractConfigKind::Member(
                    _,
                    compilation::parser::ast::ast_concepts::AstConfigMemberMetadataKind::Override(
                        _,
                    ),
                )
        );
        is_override
            || cfg
                .cfg_members
                .iter()
                .any(|child| contains_override(child, state, pairs, relative_cursor))
    }

    main_units.iter().any(|unit| {
        let compilation::semantic::compilation_unit::CompilationUnit::Impl(impl_id) = unit else {
            return false;
        };
        let ImplHirKind::Config(_) = compiler.impls[*impl_id].kind;
        let Some(ast_id) = compiler.impls[*impl_id].ast_id else {
            return false;
        };
        contains_override(ast.get_cfg_root(ast_id), state, &pairs, relative_cursor)
    })
}

fn config_completion_candidate(
    state: &DocumentState,
    compiler: &ScriptCompiler,
    byte_off: usize,
) -> Option<ConfigCompletionCandidate> {
    let pairs = config_delimiter_pairs(state);
    let relative_cursor = byte_off.saturating_sub(state.script_start) as u32;
    let ast = state.asts.first()?.as_ref()?;
    let main_units = state.compilation_syms.first()?.as_ref()?;
    let mut candidates = Vec::new();

    for unit in main_units {
        let compilation::semantic::compilation_unit::CompilationUnit::Impl(impl_id) = unit else {
            continue;
        };
        let impl_hir = &compiler.impls[*impl_id];
        if impl_hir.scope_origin != scopes_concepts::ScopeType::Complex {
            continue;
        }

        let ImplHirKind::Config(cfg_root_id) = &impl_hir.kind;
        let cfg_root = &compiler.cfgs[*cfg_root_id];
        let Some(ast_id) = impl_hir.ast_id else {
            continue;
        };
        let AbstractConfigKind::Root(_, root_meta) = &ast.get_cfg_root(ast_id).kind else {
            continue;
        };
        let Some(root_name_span) = cfg_kind_name_span(&ast.get_cfg_root(ast_id).kind) else {
            continue;
        };
        let Some((open, close)) = config_block_bounds(state, &pairs, root_name_span.end) else {
            continue;
        };

        candidates.push(ConfigCompletionCandidate {
            open,
            close,
            name_start: root_name_span.start,
            type_id: matches!(root_meta, ConfigRootMetadataKind::Complex)
                .then(|| config_root_type_id(compiler, cfg_root))
                .flatten(),
            scope_id: matches!(root_meta, ConfigRootMetadataKind::Override)
                .then(|| {
                    cfg_root.linked_sym_id.and_then(|sym_id| {
                        match compiler.syms[sym_id].associated_scope {
                            Some(scopes_concepts::AssociatedScopeKind::Scope(scope_id)) => {
                                Some(scope_id)
                            }
                            _ => None,
                        }
                    })
                })
                .flatten(),
            is_root: true,
            configured_options: configured_option_names(compiler, &cfg_root.stmts),
            configured_members: cfg_root
                .common
                .cfg_membs
                .iter()
                .filter_map(|member_id| match &compiler.impl_membs[*member_id] {
                    ImplMemberKind::ConfigMember(member) => Some(member.common.name_id),
                    ImplMemberKind::OptAssignmentRoot(_)
                    | ImplMemberKind::OptAssignmentMember(_)
                    | ImplMemberKind::MultiTypeAssignment(_)
                    | ImplMemberKind::Unknown { .. } => None,
                })
                .collect(),
        });

        for &member_id in &cfg_root.common.cfg_membs {
            config_candidate_for_member(compiler, member_id, state, &pairs, &mut candidates);
        }
    }

    candidates
        .into_iter()
        .filter(|candidate| candidate.open <= relative_cursor && relative_cursor < candidate.close)
        .max_by_key(|candidate| (candidate.open, candidate.name_start))
}

fn completion_follows_override(state: &DocumentState, prefix_start: usize) -> bool {
    let relative_start = prefix_start.saturating_sub(state.script_start) as u32;
    let before_prefix = state
        .tokens
        .partition_point(|token| token.span.end <= relative_start);
    let previous = state.tokens[..before_prefix]
        .iter()
        .rfind(|token| !matches!(token.tok, ScriptToken::EOF));

    previous.is_some_and(|token| {
        matches!(
            token.tok,
            ScriptToken::Keyword(lang::keywords::Keyword::Override)
        )
    })
}

/// Resolve the namespace immediately preceding an override shorthand arrow.
///
/// Use tokens instead of resolved config HIR because an invalid child after the
/// cursor can prevent core from retaining that config member. The namespace
/// transition at `override JAVA=>` is still unambiguous in the source.
fn override_arrow_namespace_scope(
    state: &DocumentState,
    compiler: &ScriptCompiler,
    prefix_start: usize,
) -> Option<ScopeId> {
    let relative_start = prefix_start.saturating_sub(state.script_start) as u32;
    let before_prefix = state
        .tokens
        .partition_point(|token| token.span.end <= relative_start);
    let tokens = state
        .tokens
        .get(before_prefix.checked_sub(3)?..before_prefix)?;

    let [override_token, namespace_token, arrow_token] = tokens else {
        return None;
    };
    if !matches!(
        override_token.tok,
        ScriptToken::Keyword(lang::keywords::Keyword::Override)
    ) || !matches!(arrow_token.tok, ScriptToken::NotSlimArrow)
    {
        return None;
    }
    let ScriptToken::Id(namespace_name_id) = namespace_token.tok else {
        return None;
    };

    namespace_scope_of_intrinsic_symbol(compiler, namespace_name_id)
}

fn override_root_completion_items(
    state: &DocumentState,
    compiler: &ScriptCompiler,
    prefix: &str,
) -> Vec<CompletionItem> {
    let roots: Vec<_> = if let Some(scope_id) = compiler.intrinsic_registry.complex_scope_id {
        compiler.scopes[scope_id]
            .scope
            .table
            .iter_interned()
            .map(|(_, sym_id)| compiler.syms[sym_id].name_id)
            .collect()
    } else {
        // An unfinished `override ` has no resolved config root, so core has not
        // materialized its intrinsic scope yet. These names are stable interner
        // preloads and are the roots core will install once the target parses.
        vec![
            InternedId::new(chrn_utils::intern::INTERNED_JAVA_UPPER),
            InternedId::new(chrn_utils::intern::INTERNED_RUST_UPPER),
        ]
    };

    roots
        .into_iter()
        .filter_map(|name_id| {
            let name = state.interner.search(name_id);
            (prefix.is_empty() || name.starts_with(prefix)).then(|| CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::VARIABLE),
                ..Default::default()
            })
        })
        .collect()
}

pub(crate) fn config_completion_items(
    state: &DocumentState,
    compiler: &ScriptCompiler,
    candidate: ConfigCompletionCandidate,
    prefix: &str,
) -> Vec<CompletionItem> {
    if let Some(scope_id) = candidate.scope_id {
        return compiler.scopes[scope_id]
            .scope
            .table
            .iter_interned()
            .filter_map(|(_, sym_id)| {
                let sym = &compiler.syms[sym_id];
                let name = state.interner.search(sym.name_id);
                (prefix.is_empty() || name.starts_with(prefix)).then(|| CompletionItem {
                    label: name.to_string(),
                    kind: Some(symbol_completion_kind(compiler, sym)),
                    ..Default::default()
                })
            })
            .collect();
    }

    let (members, root_schema_kind) = match candidate.type_id {
        Some(type_id) => match config_type_info(compiler, type_id) {
            Some(info) => info,
            None if !candidate.is_root => (Vec::new(), ConfigSchemaKind::Member),
            None => return Vec::new(),
        },
        None if !candidate.is_root => (Vec::new(), ConfigSchemaKind::Member),
        None => return Vec::new(),
    };

    let schema_kind = if candidate.is_root {
        root_schema_kind
    } else {
        ConfigSchemaKind::Member
    };
    let configured_members = candidate.configured_members;
    let configured_options = candidate.configured_options;
    let mut items = Vec::new();

    for (name_id, kind) in members {
        if configured_members.contains(&name_id) {
            continue;
        }
        let name = state.interner.search(name_id);
        if prefix.is_empty() || name.starts_with(prefix) {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(kind),
                ..Default::default()
            });
        }
    }

    for option in get_cfg_schema(schema_kind).opt_schema {
        if configured_options.contains(&option.name_id) {
            continue;
        }
        let name = state.interner.search(option.name_id);
        if prefix.is_empty() || name.starts_with(prefix) {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                ..Default::default()
            });
        }
    }

    items
}

/// Accumulates semantic tokens in the delta encoding the LSP protocol requires:
/// each token's position is expressed relative to the previously emitted one.
///
/// Owning the running `(prev_line, prev_start, first)` state here keeps it out of
/// the emit loop, which previously passed all three plus the output vector as
/// `&mut` arguments to a seven-parameter helper on every push.
#[derive(Default)]
struct DeltaTokens {
    tokens: Vec<SemanticToken>,
    prev_line: u32,
    prev_start: u32,
    /// `false` until the first token is pushed; the first token is absolute.
    started: bool,
}

impl DeltaTokens {
    /// Appends a token at `start_pos`, delta-encoded against the previous one.
    ///
    /// `start_pos` must not precede the previously pushed token.
    fn push(&mut self, start_pos: Position, length: u32, token_type: u32) {
        let (delta_line, delta_start) = if !self.started {
            self.started = true;
            (start_pos.line, start_pos.character)
        } else if start_pos.line == self.prev_line {
            (0, start_pos.character.saturating_sub(self.prev_start))
        } else {
            (
                start_pos.line.saturating_sub(self.prev_line),
                start_pos.character,
            )
        };
        self.prev_line = start_pos.line;
        self.prev_start = start_pos.character;
        self.tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });
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
                let sym = &compiler.syms[*sym_id];
                match sym.kind {
                    SymbolKind::Type(tid) => {
                        let ty = &compiler.types[tid].ty;
                        match ty {
                            Type::BuiltinTypeInfo(_)
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
                    SymbolKind::Namespace => match sym.associated_scope {
                        Some(scopes_concepts::AssociatedScopeKind::Module(_)) => {
                            return Some(SemanticTokenType::Keyword.as_u32());
                        }
                        Some(scopes_concepts::AssociatedScopeKind::Scope(scope_id)) => {
                            let token_type = if compiler.scopes[scope_id].scope.is_intrinsic {
                                SemanticTokenType::Class
                            } else {
                                SemanticTokenType::Variable
                            };
                            return Some(token_type.as_u32());
                        }
                        None => return None,
                    },
                    SymbolKind::Directive(_) => {
                        return Some(SemanticTokenType::Regexp.as_u32());
                    }
                    SymbolKind::ExternType(_) => {
                        return Some(SemanticTokenType::Type.as_u32());
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
        let open_docs = Arc::clone(&self.docs);
        let analysis_slots = Arc::clone(&self.analysis_slots);
        if let Some(previous) = self.pending_tasks.write().remove(&uri_str) {
            previous.abort();
        }
        self.doc_cache.invalidate(&uri_str);
        let handle = tokio::spawn(async move {
            analyze_and_publish_task(
                client,
                params.text_document.uri,
                text,
                dc,
                doc_cache,
                pending_versions,
                open_docs,
                analysis_slots,
                version,
            )
            .await
        });
        self.pending_tasks.write().insert(uri_str, handle);
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
        let open_docs = Arc::clone(&self.docs);
        let analysis_slots = Arc::clone(&self.analysis_slots);

        if let Some(handle) = self.pending_tasks.write().remove(&uri_str) {
            handle.abort();
        }

        self.doc_cache.invalidate(&uri_str);
        let handle = tokio::spawn(async move {
            analyze_and_publish_task(
                client,
                params.text_document.uri,
                text,
                dc,
                doc_cache,
                pending_versions,
                open_docs,
                analysis_slots,
                version,
            )
            .await
        });
        self.pending_tasks.write().insert(uri_str, handle);
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        // Remove the document on close to free memory and avoid stale state
        let uri = params.text_document.uri.to_string();
        self.docs.write().remove(&uri);
        // Remove the current generation so every surviving task becomes stale.
        self.pending_versions.write().remove(&uri);
        // remove cached diagnostics for this doc
        self.diags_cache.write().remove(&uri);
        // Abort and remove any pending analysis task for this document.
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
            _ = self
                .client
                .show_message(
                    tower_lsp::lsp_types::MessageType::ERROR,
                    "Failed to apply text change",
                )
                .await;
            return;
        };

        const DEBOUNCE_MS: u64 = 150;

        let my_version = self.bump_version(&uri_str);
        self.doc_cache.invalidate(&uri_str);

        let client = self.client.clone();
        let pv = self.pending_versions.clone();
        let dc = self.diags_cache.clone();
        let analysis_slots = Arc::clone(&self.analysis_slots);

        if let Some(prev) = self.pending_tasks.write().remove(&uri_str) {
            prev.abort();
        }

        let inner_uri_str = uri_str.clone();
        let doc_cache = self.doc_cache.clone();
        let open_docs = Arc::clone(&self.docs);
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
                    open_docs,
                    analysis_slots,
                    my_version,
                )
                .await;
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
        let Some(state_arc) = self.get_analyzed_state(&uri, text).await else {
            return Ok(None);
        };
        let state = match state_arc.try_read_for(STATE_LOCK_TIMEOUT) {
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
        let Some(state_arc) = self.get_analyzed_state(&uri, text).await else {
            return Ok(None);
        };

        self.preload_definition_file(&state_arc, &uri, pos).await;

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
        let Some(state_arc) = self.get_analyzed_state(&uri, text).await else {
            return Ok(None);
        };

        self.preload_definition_file(&state_arc, &uri, pos).await;

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
        let Some(state_arc) = self.get_analyzed_state(&uri, text).await else {
            return Ok(None);
        };

        let Some(state) = state_arc.try_read_for(STATE_LOCK_TIMEOUT) else {
            return Ok(None);
        };
        let compiler = match &state.compiler {
            Some(c) => c,
            None => return Ok(None),
        };

        let toks_vec = &state.tokens;

        let mut out = DeltaTokens::default();

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
        //
        // Both token and trivia spans are relative to the region's `src_bytes`;
        // the comparison below therefore operates in that relative space.  When
        // emitting LSP positions, we add `script_start` to shift the offsets
        // back into the absolute file coordinate system the client expects.
        //
        // Because that merge yields non-decreasing offsets, a single
        // `PositionCursor` can convert them all in one pass over the document;
        // calling `offset_to_position` per token rescanned the whole file each
        // time, which is quadratic in the file size.
        let script_start = state.script_start;
        let mut cursor = crate::text::PositionCursor::new(&state.text);
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
                let abs_start =
                    crate::text::rel_to_abs_offset(triv.span.start, script_start) as usize;
                let start_pos = cursor.position_at(abs_start);
                let length = triv.span.end.saturating_sub(triv.span.start);

                out.push(start_pos, length, SemanticTokenType::Comment.as_u32());
            } else if tok_idx < toks_vec.len() {
                let st = &toks_vec[tok_idx];
                tok_idx += 1;
                let span = st.span;
                let abs_start = crate::text::rel_to_abs_offset(span.start, script_start) as usize;
                let start_pos = cursor.position_at(abs_start);
                let length = span.end.saturating_sub(span.start);

                let token_type: u32 = match st.tok {
                    ScriptToken::Def | ScriptToken::End => SemanticTokenType::Macro.as_u32(),
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
                        let entity = state.get_entity_at_offset(abs_start);
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
                    | ScriptToken::NotSlimArrow
                    | ScriptToken::StaticAccess => SemanticTokenType::Operator.as_u32(),
                    _ => continue,
                };

                out.push(start_pos, length, token_type);
            } else {
                break;
            }
        }

        let sem_toks = tower_lsp::lsp_types::SemanticTokens {
            result_id: None,
            data: out.tokens,
        };

        Ok(Some(tower_lsp::lsp_types::SemanticTokensResult::Tokens(
            sem_toks,
        )))
    }

    async fn goto_definition(
        &self,
        params: tower_lsp::lsp_types::GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let Some(text) = self.get_document_text(uri.as_ref()) else {
            return Ok(None);
        };
        let Some(state_arc) = self.get_analyzed_state(&uri, text).await else {
            return Ok(None);
        };

        // `(def_path, def_span, def_script_start)` — the script start of the
        // target region is needed to convert the relative `def_span` (which
        // is relative to the target region's `src_bytes`) into an absolute
        // file byte offset usable as an LSP `Position`.
        let def_target = {
            let Some(state) = state_arc.try_read_for(STATE_LOCK_TIMEOUT) else {
                return Ok(None);
            };
            let byte_offset = crate::text::position_to_offset(&state.text, pos);
            if state.offset_in_comment(byte_offset) {
                None
            } else {
                state.get_entity_at_offset(byte_offset).and_then(|entity| {
                    let (def_path, def_span, _) = state.get_definition_location(entity)?;
                    let def_script_start = state.region_arena[def_span.region_id].script_start;
                    Some((def_path, def_span, def_script_start))
                })
            }
        };

        let Some((def_path, def_span, def_script_start)) = def_target else {
            return Ok(None);
        };

        let target_uri = Url::from_file_path(&def_path).unwrap_or_else(|_| uri.clone());

        // The target file's text is needed to turn the span into a position.
        let target_text = if def_path == uri.path() {
            state_arc
                .try_read_for(STATE_LOCK_TIMEOUT)
                .map(|state| Arc::clone(&state.text))
        } else {
            let target_uri_str = target_uri.to_string();
            match self
                .doc_cache
                .get_text(&target_uri_str)
                .or_else(|| self.docs.read().get(&target_uri_str).map(Arc::clone))
            {
                Some(t) => Some(t),
                None => tokio::fs::read_to_string(&def_path)
                    .await
                    .ok()
                    .map(Arc::new),
            }
        };

        let Some(t_text) = target_text else {
            return Ok(None);
        };

        // `def_span` is relative to the target region's `src_bytes`; add the target
        // region's `script_start` to put it in absolute file coordinates before
        // converting to an LSP `Position`.
        let abs_start = crate::text::rel_to_abs_offset(def_span.start, def_script_start) as usize;
        let abs_end = crate::text::rel_to_abs_offset(def_span.end, def_script_start) as usize;
        let target_range = Range {
            start: crate::text::offset_to_position(&t_text, abs_start),
            end: crate::text::offset_to_position(&t_text, abs_end),
        };

        Ok(Some(GotoDefinitionResponse::Link(vec![LocationLink {
            origin_selection_range: Some(Range {
                start: pos,
                end: pos,
            }),
            target_uri,
            target_range,
            target_selection_range: target_range,
        }])))
    }

    async fn completion(
        &self,
        params: tower_lsp::lsp_types::CompletionParams,
    ) -> jsonrpc::Result<Option<tower_lsp::lsp_types::CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let Some(state_arc) = self.get_state(uri).await else {
            return Ok(None);
        };

        let state_guard = match state_arc.try_read_for(STATE_LOCK_TIMEOUT) {
            Some(g) => g,
            None => return Ok(None),
        };
        let state = &*state_guard;

        let byte_off =
            crate::text::position_to_offset(&state.text, params.text_document_position.position);
        let (mut start_b, _end_b) = crate::text::find_word_bounds(&state.text, byte_off);
        // Word bounds include `>` for section names such as `nest->`. A config
        // arrow is a delimiter, including when a prefix directly follows it.
        if start_b > 0
            && start_b < byte_off
            && state.text.as_bytes().get(start_b - 1..start_b + 1) == Some(b"=>")
        {
            start_b += 1;
        }
        let prefix = &state.text[start_b..byte_off.min(state.text.len())];

        // `>` triggers automatically only as the second character of `=>`.
        if params
            .context
            .as_ref()
            .and_then(|context| context.trigger_character.as_deref())
            == Some(">")
            && !state.text[..byte_off].ends_with("=>")
        {
            return Ok(Some(CompletionResponse::Array(Vec::new())));
        }

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

        // Static access is detected at the word start so a partially typed member
        // (`i32::MA`) still triggers it, not just an empty token after `::`.
        let is_static_access_completion = start_b >= 2
            && state.text.as_bytes()[start_b - 1] == b':'
            && state.text.as_bytes()[start_b - 2] == b':';
        if is_static_access_completion {
            let mut items = Vec::new();
            if let Some(compiler) = &state.compiler {
                let path = static_access_target_path(state, start_b);
                let allow_intrinsic_root = cursor_in_override_config(state, compiler, byte_off);
                if let Some(namespace) =
                    resolve_completion_namespace_path(compiler, &path, allow_intrinsic_root)
                {
                    let mut push_symbol = |sym_id: SymbolId| {
                        let sym = &compiler.syms[sym_id];
                        let name = state.interner.search(sym.name_id);
                        if prefix.is_empty() || name.starts_with(prefix) {
                            items.push(CompletionItem {
                                label: name.to_string(),
                                kind: Some(symbol_completion_kind(compiler, sym)),
                                ..Default::default()
                            });
                        }
                    };

                    match namespace {
                        CompletionNamespace::Module(mod_id) if mod_id == ModuleId::new(0) => {
                            for sym_id in reachable_module_symbols(compiler, mod_id) {
                                let sym = &compiler.syms[sym_id];
                                if sym.scope_origin == scopes_concepts::ScopeType::Var
                                    || matches!(sym.kind, SymbolKind::Directive(_))
                                {
                                    continue;
                                }
                                push_symbol(sym_id);
                            }
                        }
                        CompletionNamespace::Module(mod_id) => {
                            for &sym_id in &compiler.mods[mod_id].exports {
                                push_symbol(sym_id);
                            }
                        }
                        CompletionNamespace::Scope(scope_id) => {
                            for (_, sym_id) in compiler.scopes[scope_id].scope.table.iter_interned()
                            {
                                push_symbol(sym_id);
                            }
                        }
                    }
                }
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }

        let target_name = dot_target_name;

        if let Some(target_name) = target_name {
            let mut items: Vec<CompletionItem> = Vec::new();
            if let Some(target_id) = state.interner.try_search_str(target_name)
                && let Some(compiler) = &state.compiler
            {
                if let Some(mod_id) = crate::state::visible_module(compiler, target_id) {
                    let module = &compiler.mods[mod_id];
                    if module.mod_id.id == 0 {
                        // Current module: everything reachable through the module's own scope
                        // tables plus the injected core scope. This mirrors real scope lookup,
                        // so compiler-internal namespace members such as `i8::MAX` (which live
                        // in builtin-type namespace scopes) are never offered here.
                        for sym_id in reachable_module_symbols(compiler, module.mod_id) {
                            let sym = &compiler.syms[sym_id];
                            if sym.scope_origin == scopes_concepts::ScopeType::Var
                                || matches!(sym.kind, SymbolKind::Directive(_))
                            {
                                continue;
                            }
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
                    } else {
                        // Other modules: show only exported symbols
                        for sym_id in &module.exports {
                            let sym = &compiler.syms[*sym_id];
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
                } else if let Some(scope_id) =
                    namespace_scope_of_visible_symbol(compiler, target_id).or_else(|| {
                        cursor_in_override_config(state, compiler, byte_off)
                            .then(|| namespace_scope_of_intrinsic_symbol(compiler, target_id))
                            .flatten()
                    })
                {
                    // Not a module: a namespace-bearing symbol such as a built-in type
                    // or an intrinsic override namespace. Its members live in the
                    // associated scope rather than in a module export list.
                    let ns_scope = &compiler.scopes[scope_id];
                    for (_, member_id) in ns_scope.scope.table.iter_interned() {
                        let member = &compiler.syms[member_id];
                        let member_name = state.interner.search(member.name_id);
                        if prefix.is_empty() || member_name.starts_with(prefix) {
                            let kind = symbol_completion_kind(compiler, member);
                            items.push(CompletionItem {
                                label: member_name.to_string(),
                                kind: Some(kind),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }

        // `override` selects a compiler-provided platform namespace. Handle this
        // before the enclosing config candidate can offer schema members/options.
        if let Some(compiler) = &state.compiler
            && completion_follows_override(state, start_b)
        {
            let items = override_root_completion_items(state, compiler, prefix);
            return Ok(Some(CompletionResponse::Array(items)));
        }

        // `override JAVA=>` is a direct namespace transition. Resolve it from
        // tokens so invalid text after the cursor cannot make completion fall
        // back to the enclosing complex config member.
        if let Some(compiler) = &state.compiler
            && let Some(scope_id) = override_arrow_namespace_scope(state, compiler, start_b)
        {
            let items = compiler.scopes[scope_id]
                .scope
                .table
                .iter_interned()
                .filter_map(|(_, sym_id)| {
                    let sym = &compiler.syms[sym_id];
                    let name = state.interner.search(sym.name_id);
                    (prefix.is_empty() || name.starts_with(prefix)).then(|| CompletionItem {
                        label: name.to_string(),
                        kind: Some(symbol_completion_kind(compiler, sym)),
                        ..Default::default()
                    })
                })
                .collect();
            return Ok(Some(CompletionResponse::Array(items)));
        }

        // Config blocks have a type-dependent namespace.  Handle them before the general
        // in-scope completion path so a `complex->` block only offers members of its current
        // struct/enum (plus the options valid for that config schema).
        if let Some(compiler) = &state.compiler
            && let Some(candidate) = config_completion_candidate(state, compiler, byte_off)
        {
            let items = config_completion_items(state, compiler, candidate, prefix);
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
            ("override", CompletionItemKind::KEYWORD),
            ("struct", CompletionItemKind::KEYWORD),
            ("enum", CompletionItemKind::KEYWORD),
            ("change", CompletionItemKind::KEYWORD),
            ("for", CompletionItemKind::KEYWORD),
            ("List", CompletionItemKind::STRUCT),
            ("Set", CompletionItemKind::STRUCT),
            ("Map", CompletionItemKind::STRUCT),
            ("Tuple", CompletionItemKind::STRUCT),
            ("true", CompletionItemKind::CONSTANT),
            ("false", CompletionItemKind::CONSTANT),
        ];

        let mut items: Vec<CompletionItem> = Vec::new();
        // Every source below filters on the same prefix and builds the same item.
        let mut push_item = |label: String, kind: CompletionItemKind| {
            if prefix.is_empty() || label.starts_with(prefix) {
                items.push(CompletionItem {
                    label,
                    kind: Some(kind),
                    ..Default::default()
                });
            }
        };

        for (label, kind) in SUGGESTIONS {
            push_item(label.to_string(), *kind);
        }

        if let Some(compiler) = &state.compiler {
            //TODO: Should auto-complete any module that has a src of "None"
            // Core library exports: types, functions, and constants.
            let core_mod = &compiler.mods[compiler.intrinsic_registry.core_mod_id];
            for sym_id in &core_mod.exports {
                let sym = &compiler.syms[*sym_id];
                push_item(
                    state.interner.search(sym.name_id).to_string(),
                    symbol_completion_kind(compiler, sym),
                );
            }

            // Compiler-origin directives (`#warn`, `#ignore`, `#scient`, …), read from
            // the symbol registry rather than hard-coded.
            // `Arena` is not an iterator; iterate over the inner `items` vec.
            for sym in &compiler.syms.items {
                if matches!(sym.kind, SymbolKind::Directive(_)) {
                    let name = state.interner.search(sym.name_id);
                    push_item(format!("#{}", name), symbol_completion_kind(compiler, sym));
                }
            }

            // The current module and everything it imports.
            // Index the `Arena` with a typed `ModuleId` (the only impl the primary
            // `Index` for `Arena` provides).
            let current_module = &compiler.mods[ModuleId::new(0)];
            for module in &compiler.mods.items {
                let is_self = module.mod_id.id == 0;
                let is_imported = current_module.imports.iter().any(|i| {
                    i.name_id == module.name_id
                        || i.sp_alias_id.as_ref().map(|sp| sp.inner) == Some(module.name_id)
                });

                if is_self || is_imported {
                    push_item(
                        state.interner.search(module.name_id).to_string(),
                        CompletionItemKind::MODULE,
                    );
                }
            }
        }

        // Reuse pre-computed tokens from the analyzed state instead of re-lexing.
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for st in &state.tokens {
            // Only include identifiers that are within the script section (before
            // `serial_start`).  `st.span.end` is relative to the region's
            // `src_bytes`, so add `script_start` to compare against the absolute
            // `serial_start`.
            let tok_end_abs = crate::text::rel_to_abs_offset(st.span.end, script_start);
            if (tok_end_abs as usize) > serial_start {
                continue;
            }

            if let ScriptToken::Id(id) = st.tok {
                let name = state.interner.search(id);
                if seen.insert(name) {
                    push_item(name.to_string(), CompletionItemKind::VARIABLE);
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}
