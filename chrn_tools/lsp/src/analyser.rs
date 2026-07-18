//! # analyser
//!
//! Provides the async document-analysis pipeline and helper utilities for
//! converting core compiler diagnostics into LSP [`Diagnostic`] objects.
//!
//! ## Responsibilities
//!
//! * [`analyze_and_publish_task`] — the primary entry point called from
//!   [`crate::backend::Backend`] every time a document changes.  It orchestrates:
//!   1. Config loading (locating `@def`/`@end` boundaries via `ChrnConfigLoader`)
//!   2. [`DocumentCache`](crate::state::DocumentCache) lookup / creation
//!   3. Full semantic analysis via [`DocumentState::ensure_analyzed`](crate::state::DocumentState::ensure_analyzed)
//!   4. Diagnostic publication (deduplicated, version-gated)
//!
//! * [`config_load_error_to_diagnostics`] — converts a [`ConfigLoadError`] into LSP
//!   diagnostics so editors can underline the problematic region in the config header.
//!
//! * [`push_diagnostics`] — converts a slice of core [`SourceDiagnostic`] values into
//!   LSP diagnostics and appends them to an existing list.
//!
//! * [`resolve_modules_lsp`] — recursively resolves imported modules, using the
//!   open-document cache first and falling back to disk.  Accumulates any import
//!   errors into the caller-supplied diagnostics vector.
//!
//! ## Version / debounce invariant
//!
//! Each document carries a monotonically increasing `version` counter (stored in
//! `pending_versions`).  [`analyze_and_publish_task`] will silently discard its
//! results if a newer version has been enqueued by the time it finishes, preventing
//! stale diagnostics from overwriting fresh ones.
//!
//! ## Diagnostic cache
//!
//! `diags_cache` stores a hash of the last-published diagnostic list per document.
//! A new publish is skipped when the serialised form hashes to the cached value,
//! avoiding unnecessary LSP notifications for no-op edits while keeping the
//! per-entry memory cost to 8 bytes instead of the full JSON payload.

use chrn_utils::source_map::source_diagnostic::DiagnosticLevel;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
use compilation::lexer::Lexer;
use compilation::modules::Bind;
use compilation::modules::ImportKind;
use compilation::modules::Module;
use compilation::modules::ModuleState;
use lang::config_loader::{ConfigLoader, ConfigLoaderOutput};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::Client;
use tower_lsp::lsp_types;

use chrn_utils::arena::Arena;
use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::core_error::{self, ConfigLoadError};
use chrn_utils::id_types::{ModuleId, PathId, SourceRegionId};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
use chrn_utils::source_map::source_region::SourceRegion;
use compilation::modules::mod_finder::ModuleFinder;
use std::io::Cursor;
use tower_lsp::lsp_types::Url;

use crate::state::DocumentCache;
use crate::state::DocumentState;

const MAX_DIAGS_CACHE_SIZE: usize = 100;

/// Evicts entries from the diagnostics cache if it has reached [`MAX_DIAGS_CACHE_SIZE`].
///
/// When the cache is full, ten extra entries are removed so the eviction does not
/// happen on every subsequent insert.  The eviction order is unspecified (HashMap
/// iteration order).
fn evict_cache_if_needed(cache: &mut HashMap<String, u64>) {
    if cache.len() >= MAX_DIAGS_CACHE_SIZE {
        let to_remove = cache.len() - MAX_DIAGS_CACHE_SIZE + 10;
        let keys_to_remove: Vec<String> = cache
            .keys()
            .take(to_remove)
            .map(|k| k.to_string())
            .collect();
        for key in keys_to_remove {
            cache.remove(&key);
        }
    }
}

/// Converts a [`ConfigLoadError`](chrn_utils::core_error::ConfigLoadError) into a list of LSP
/// [`Diagnostic`](tower_lsp::lsp_types::Diagnostic) values that the editor can display.
///
/// # Parameters
/// * `err`  — The error returned by [`ChrnConfigLoader::load_config`].
/// * `text` — The raw source text of the document, used to convert byte offsets to
///   LSP line/character positions.
///
/// # Behaviour
/// * A `ConfigLoadError::Diagnostic` diagnostic is expanded into one primary diagnostic
///   (using the `primary` annotation span if present) plus one additional diagnostic
///   per secondary annotation, note, and help message.
/// * A `ConfigLoadError::IO` error is reported at position `(0, 0)` because no span
///   information is available.
///
/// # Spanning
///
/// The diagnostic spans are produced by `ConfigLoader` and are **relative** to
/// the region's `src_bytes`; `script_start` is added to each one to convert
/// them to absolute file positions before being passed to
/// [`crate::text::offset_to_position`].
pub(crate) fn config_load_error_to_diagnostics(
    err: chrn_utils::core_error::ConfigLoadError,
    text: &str,
    script_start: usize,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let source = "chrn-config";
    let start = lsp_types::Position {
        line: 0,
        character: 0,
    };

    match err {
        chrn_utils::core_error::ConfigLoadError::Diagnostic(diag) => {
            let severity = match diag.level {
                DiagnosticLevel::Error => lsp_types::DiagnosticSeverity::ERROR,
                DiagnosticLevel::Warn => lsp_types::DiagnosticSeverity::WARNING,
                DiagnosticLevel::Note => lsp_types::DiagnosticSeverity::INFORMATION,
                DiagnosticLevel::Help => lsp_types::DiagnosticSeverity::HINT,
            };

            let (start_pos, end_pos) = if let Some(annotation) = diag
                .annotations
                .iter()
                .find(|a| matches!(a.kind, AnnotationKind::Primary))
                .or_else(|| diag.annotations.first())
            {
                let abs_s =
                    crate::text::rel_to_abs_offset(annotation.span.start, script_start) as usize;
                let abs_e =
                    crate::text::rel_to_abs_offset(annotation.span.end, script_start) as usize;
                let s_pos = crate::text::offset_to_position(text, abs_s);
                let e_pos = crate::text::offset_to_position(text, abs_e);
                (s_pos, e_pos)
            } else {
                (start, start)
            };

            let mut result = vec![tower_lsp::lsp_types::Diagnostic {
                range: lsp_types::Range {
                    start: start_pos,
                    end: end_pos,
                },
                severity: Some(severity),
                source: Some(source.to_string()),
                message: diag.core_msg,
                ..Default::default()
            }];

            for annotation in &diag.annotations {
                let msg = match &annotation.label {
                    Some(label) => label.clone(),
                    None => match annotation.kind {
                        AnnotationKind::Primary => continue,
                        AnnotationKind::Secondary => "related to this".to_string(),
                        AnnotationKind::Note => "note: ".to_string(),
                        AnnotationKind::Help => "help: ".to_string(),
                    },
                };

                let abs_ann_start =
                    crate::text::rel_to_abs_offset(annotation.span.start, script_start) as usize;
                let abs_ann_end =
                    crate::text::rel_to_abs_offset(annotation.span.end, script_start) as usize;
                let ann_start = crate::text::offset_to_position(text, abs_ann_start);
                let ann_end = crate::text::offset_to_position(text, abs_ann_end);
                let ann_sev = match annotation.kind {
                    AnnotationKind::Primary => severity,
                    AnnotationKind::Secondary => lsp_types::DiagnosticSeverity::WARNING,
                    AnnotationKind::Note => lsp_types::DiagnosticSeverity::INFORMATION,
                    AnnotationKind::Help => lsp_types::DiagnosticSeverity::HINT,
                };
                result.push(tower_lsp::lsp_types::Diagnostic {
                    range: lsp_types::Range {
                        start: ann_start,
                        end: ann_end,
                    },
                    severity: Some(ann_sev),
                    source: Some(source.to_string()),
                    message: msg,
                    ..Default::default()
                });
            }

            for note in &diag.notes {
                result.push(tower_lsp::lsp_types::Diagnostic {
                    range: lsp_types::Range {
                        start: start_pos,
                        end: end_pos,
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::INFORMATION),
                    source: Some(source.to_string()),
                    message: note.clone(),
                    ..Default::default()
                });
            }

            for help_msg in &diag.help {
                result.push(tower_lsp::lsp_types::Diagnostic {
                    range: lsp_types::Range {
                        start: start_pos,
                        end: end_pos,
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::HINT),
                    source: Some(source.to_string()),
                    message: help_msg.clone(),
                    ..Default::default()
                });
            }

            result
        }
        chrn_utils::core_error::ConfigLoadError::IO(io) => vec![tower_lsp::lsp_types::Diagnostic {
            range: lsp_types::Range { start, end: start },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            source: Some(source.to_string()),
            message: io.to_string(),
            ..Default::default()
        }],
    }
}

/// Module-resolution results gathered outside the [`DocumentState`] write lock.
///
/// Keeping this work out of `ensure_analyzed` breaks the lock-order inversion that
/// caused deadlocks: `ensure_analyzed` used to hold the per-document write lock while
/// calling back into `DocumentCache::get_text`.
pub(crate) struct ModuleResolution {
    /// `bind` declaration from the config header, if any.
    pub bind: Option<Bind>,
    /// Main module region (id 0).
    pub main_region: SourceRegion,
    /// Main module descriptor.
    pub main_mod: Module,
    /// Imported module descriptors; indexed by `ModuleId::id - 1`.
    pub sub_mods: Vec<Option<Module>>,
    /// Imported module regions; ids are `1..=sub_mods.len()`.
    pub sub_regions: Vec<SourceRegion>,
    /// Config/import diagnostics collected during resolution.
    pub config_errors: Option<Vec<SourceDiagnostic>>,
    /// URI strings of every imported module, for dependency registration.
    pub imported_uris: Vec<String>,
}

/// A document whose lexical data and imported modules have been resolved, but whose
/// compiler pipeline has not yet run.
pub(crate) struct PreparedDocument {
    pub state: DocumentState,
    pub resolution: ModuleResolution,
}

/// Resolves all imported modules for `text` and builds a pre-analysis [`DocumentState`].
///
/// This function performs all work that needs to touch [`DocumentCache`] (for in-memory
/// copies of imported files) or disk.  It is intentionally synchronous and does NOT
/// acquire any `DocumentState` lock, so it can safely call `DocumentCache::get_text`
/// without risking the deadlock described in [`ModuleResolution`].
///
/// The `interner` is the one that was already used for the initial config load of
/// `main_region`, guaranteeing that `main_region.path_id` is valid in the same
/// interner the resulting `DocumentState` owns.  (Previously a second interner was
/// created here, which only worked because both interners happened to assign id 0
/// to the first path interned.)
pub(crate) fn resolve_document_modules(
    uri: &Url,
    text: Arc<String>,
    main_region: SourceRegion,
    chrn_cfg: &ChrnConfig,
    doc_cache: &DocumentCache,
    version: u64,
    mut interner: Intern,
) -> PreparedDocument {
    let path_buf = uri
        .to_file_path()
        .unwrap_or_else(|_| PathBuf::from(uri.path()));
    let path_id = interner.intern_path(&path_buf);

    // The lexer is given the *relative* `src_bytes` (the script section only) and
    // the *absolute* `script_start` (the byte position in the file where the
    // script section starts). Token spans are produced relative to `src_bytes`,
    // which is what the parser/compiler expect. The LSP later converts these
    // relative spans to absolute file positions using `script_start` whenever it
    // needs to surface them as LSP `Position`s.
    let (tokens, trivia) = Lexer::new(
        SourceRegionId::new(0),
        &main_region.src_bytes,
        main_region.script_start,
    )
    .tokenize(&mut interner);

    let name = path_buf
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("<unnamed>")
        .to_string();
    let name_id = interner.intern(&name);

    let mut reserved_mod_ids: Vec<(PathId, ModuleId)> = vec![(path_id, ModuleId::new(0))];

    let (bind, main_imports, finder_diags) = ModuleFinder::new(
        &main_region.src_bytes,
        chrn_cfg,
        &mut reserved_mod_ids,
        &main_region,
        main_region.script_start,
        main_region.serial_start,
    )
    .collect_imports(&mut interner);

    let mut config_errors = if finder_diags.is_empty() {
        None
    } else {
        Some(finder_diags)
    };

    let main_mod = Module::new(
        name_id,
        ModuleState::Loading,
        ModuleId::new(0),
        main_imports,
        Some(SourceRegionId::new(0)),
    );

    let mut seen: Vec<PathId> = vec![path_id];
    let mut sub_mods = Vec::with_capacity(main_mod.imports.len());
    let mut sub_regions: Vec<SourceRegion> = Vec::new();
    let mut sub_diags = Vec::new();

    resolve_modules_lsp(
        &mut reserved_mod_ids,
        &mut seen,
        &mut sub_mods,
        &mut sub_regions,
        &main_mod,
        chrn_cfg,
        &mut interner,
        doc_cache,
        &mut sub_diags,
        path_id,
    );

    if !sub_diags.is_empty() {
        match &mut config_errors {
            Some(existing) => existing.append(&mut sub_diags),
            None => config_errors = Some(sub_diags),
        }
    }

    let imported_uris: Vec<String> = sub_mods
        .iter()
        .filter_map(|mod_opt| {
            let m = mod_opt.as_ref()?;
            let region_id = m.region_id?;
            let region = sub_regions.get(region_id.id as usize - 1)?;
            let p = interner.search_path(region.path_id);
            Url::from_file_path(p).ok().map(|u| u.to_string())
        })
        .collect();

    let state = DocumentState::new(
        Arc::clone(&text),
        tokens,
        trivia,
        interner,
        main_region.script_start,
        main_region.serial_start,
        version,
    );

    PreparedDocument {
        state,
        resolution: ModuleResolution {
            bind,
            main_region,
            main_mod,
            sub_mods,
            sub_regions,
            config_errors,
            imported_uris,
        },
    }
}

/// Async task that analyses a document and publishes diagnostics to the LSP client.
///
/// This is the primary entry point for the analysis pipeline.  It is spawned as a
/// Tokio task by [`crate::backend::Backend`] on `did_open`, `did_save`, and (after a
/// debounce period) on `did_change`.
///
/// # Parameters
/// * `client`          — The tower-lsp client handle used to call `publish_diagnostics`.
/// * `uri`             — The document URI being analysed.
/// * `text`            — The current source text.
/// * `diags_cache`     — Shared JSON cache of last-published diagnostics per URI;
///   prevents redundant notifications.
/// * `doc_cache`       — Shared analysis cache; provides tokenisation and semantic
///   analysis results.
/// * `pending_versions`— Monotonic per-URI version counter used to discard stale results.
/// * `version`         — The version token assigned to this particular analysis run.
///
/// # Analysis steps
/// 1. Runs `ChrnConfigLoader` to identify script boundaries.  If this fails, the
///    config errors are published immediately and the task returns early.
/// 2. Resolves imported modules and tokenises the document **outside** any
///    `DocumentState` lock, using `DocumentCache` and disk as needed.
/// 3. Inserts the prepared document into `DocumentCache`.
/// 4. Calls `DocumentState::ensure_analyzed` to run parsing, name resolution,
///    type-checking, and symbol-map construction.
/// 5. Registers cross-module dependency edges in the cache.
/// 6. Publishes diagnostics via `publish_if_current`, which checks that the version
///    still matches before sending.
pub async fn analyze_and_publish_task(
    client: Client,
    uri: Url,
    text: Arc<String>,
    diags_cache: Arc<RwLock<HashMap<String, u64>>>,
    doc_cache: Arc<DocumentCache>,
    pending_versions: Arc<RwLock<HashMap<String, u64>>>,
    version: u64,
) {
    let chrn_cfg = ChrnConfig::default();

    let path_buf = uri
        .to_file_path()
        .unwrap_or_else(|_| PathBuf::from(uri.path()));

    // 1. Initial config load to find boundaries.
    //
    // A single `Intern` is used for the whole analysis: the `path_id` carried by
    // the region is interned in the same interner that later stages (module
    // resolution, parsing, diagnostics) read from, so the ids always line up.
    // The interner is then moved into the `DocumentState` instead of allocating
    // a second, throwaway interner per analysis run.
    let mut interner = Intern::init();
    let path_id = interner.intern_path(&path_buf);

    // Config-load diagnostics produced on the recoverable `Broken` path.  They
    // are folded into the final publish at the end of the task rather than
    // being published immediately: `publish_diagnostics` *replaces* the whole
    // diagnostic set for a URI, so the previous early publish was wiped out by
    // the pipeline publish that followed, making config-load errors vanish
    // from the editor.
    let mut pre_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
    let region = match ConfigLoader::new(
        SourceRegionId::new(0),
        Cursor::new(text.as_bytes()),
        path_id,
        &chrn_cfg,
        &interner,
    )
    .load_config()
    {
        ConfigLoaderOutput::Success(region) => region,
        ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
            // The broken region still carries the `script_start` discovered so
            // far (may be 0 if no `@def` was found), which is the offset the
            // diagnostic spans need to be shifted by to land in absolute file
            // coordinates.
            pre_diags =
                config_load_error_to_diagnostics(cfg_err, &text, broken_region.script_start);
            broken_region
        }
        ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
            // The loader was unable to recover any region data, so the script
            // start defaults to 0.  Diagnostic spans produced up to that point
            // (e.g. unclosed multi-line comments) are still relative to the
            // start of the file, so this noop shift is the right default.
            let diags = config_load_error_to_diagnostics(cfg_err, &text, 0);
            publish_if_current(
                &client,
                &uri,
                diags,
                &diags_cache,
                &pending_versions,
                version,
            )
            .await;
            return;
        }
    };

    // 2. Resolve imported modules and build a pre-analysis state **without**
    //    holding any DocumentState lock.  This breaks the previous deadlock cycle
    //    where ensure_analyzed held the per-document write lock while calling
    //    DocumentCache::get_text.  The interner is moved in here.
    let prepared = resolve_document_modules(
        &uri,
        Arc::clone(&text),
        region,
        &chrn_cfg,
        &doc_cache,
        version,
        interner,
    );

    // 3. Insert the prepared state into the cache.  If the same text is already
    //    cached, the existing state is reused.
    let state_arc = doc_cache.insert_or_get(uri.as_ref(), Arc::clone(&text), prepared.state);

    // 4. Run the compiler pipeline while holding only the per-document lock.
    //    `ensure_analyzed` returns the imported module URIs (moved out of the
    //    resolution), or an empty vec when the state was already analyzed.
    let imported_uris = {
        let mut state = state_arc.write();
        state.ensure_analyzed(prepared.resolution)
    };

    if !imported_uris.is_empty() {
        doc_cache.register_dependencies(uri.as_ref(), &imported_uris);
    }

    // 5. Get diagnostics and publish if still current.  Config-load diagnostics
    //    from the `Broken` path are prepended so they survive this (replacing)
    //    publish.
    let mut lsp_diags = pre_diags;
    lsp_diags.extend(state_arc.read().get_lsp_diagnostics());
    publish_if_current(
        &client,
        &uri,
        lsp_diags,
        &diags_cache,
        &pending_versions,
        version,
    )
    .await;
}

/// Publishes `lsp_diags` to the client only when `version` is still the latest version
/// for `uri` and the diagnostic list has actually changed.
///
/// # Version check
/// Reads `pending_versions` under a short-lived lock.  If the stored version differs
/// from `version`, this means a newer analysis task was spawned and these results are
/// stale — they are silently discarded.
///
/// # Deduplication
/// Serialises `lsp_diags` to JSON and stores only a 64-bit hash of the payload in
/// `diags_cache`.  If the hashes are equal the publish is skipped.  Keeping the
/// hash rather than the full JSON string means the cache holds 8 bytes per
/// document instead of a (potentially large) diagnostic payload per document.
/// On serialisation failure the diagnostics are always sent (fail-open).
async fn publish_if_current(
    client: &Client,
    uri: &Url,
    lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic>,
    diags_cache: &RwLock<HashMap<String, u64>>,
    pending_versions: &RwLock<HashMap<String, u64>>,
    version: u64,
) {
    // Check version
    {
        let vers = pending_versions.read();
        if let Some(&v) = vers.get(uri.as_ref())
            && v != version
        {
            return; // Newer version exists, discard these results
        }
    }

    // Cache check
    if let Ok(serialized) = serde_json::to_string(&lsp_diags) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serialized.hash(&mut hasher);
        let digest = hasher.finish();
        // The JSON string is dropped here; only the digest persists in the cache.
        let key = uri.to_string();
        let should_send = {
            let mut cache = diags_cache.write();
            evict_cache_if_needed(&mut cache);
            match cache.get(&key) {
                Some(prev) if *prev == digest => false,
                _ => {
                    cache.insert(key, digest);
                    true
                }
            }
        };

        if should_send {
            client
                .publish_diagnostics(uri.clone(), lsp_diags, None)
                .await;
        }
    } else {
        client
            .publish_diagnostics(uri.clone(), lsp_diags, None)
            .await;
    }
}

/// Resolves the source text and `script_start` for a [`SourceDiagnostic`] by
/// looking up the [`SourceRegion`](chrn_utils::source_map::source_region::SourceRegion)
/// whose `path_id` matches the diagnostic's `path_id`.
///
/// Returns `(text, doc_len, script_start)` where:
/// * `text` is the raw source bytes of the matching region, decoded as UTF-8.
/// * `doc_len` is `text.len()`.
/// * `script_start` is the absolute file byte position of the region's start,
///   which is added to relative diagnostic spans to put them in absolute
///   file coordinates before being surfaced to the LSP client.
///
/// Falls back to `(fallback_text, fallback_doc_len, 0)` if no matching region is
/// found. This is the case for diagnostics emitted by the compiler intrinsics
/// (which never correspond to a user file) or for diagnostics whose region has
/// been evicted from the arena.
fn resolve_diag_text<'a>(
    arena: &'a Arena<SourceRegion, SourceRegionId>,
    diag: &SourceDiagnostic,
    fallback_text: &'a str,
    fallback_doc_len: usize,
) -> (&'a str, usize, usize) {
    // SAFETY: The arena is built from the same `Intern` instance that produced
    // the diagnostic's `path_id` (see `DocumentState::ensure_analyzed`).
    // Therefore `region.path_id == diag.path_id` is a correct comparison.
    if let Some(region) = arena.items.iter().find(|r| r.path_id == diag.path_id) {
        let text = std::str::from_utf8(&region.src_bytes).unwrap_or(fallback_text);
        return (text, region.src_bytes.len(), region.script_start);
    }
    (fallback_text, fallback_doc_len, 0)
}

/// Converts a slice of core [`SourceDiagnostic`] values and appends the resulting LSP
/// diagnostics to `lsp_diags`.
///
/// For each core diagnostic the function emits:
/// * One primary diagnostic at the primary annotation span (or `(0,0)` if absent).
/// * One additional diagnostic per secondary annotation with its span.
/// * One `INFORMATION`-severity diagnostic per note (at the primary span).
/// * One `HINT`-severity diagnostic per help message (at the primary span).
///
/// # Region resolution
///
/// Each `SourceDiagnostic` carries a `path_id` identifying which file the
/// diagnostic came from. The byte spans in its annotations are offsets into
/// *that* file's bytes, not the main document's. To produce a correct LSP
/// range, we look up the matching [`SourceRegion`](chrn_utils::source_map::source_region::SourceRegion)
/// in `arena` and convert the span against that region's text.
///
/// Diagnostics whose `path_id` does not match any region (e.g. compiler-intrinsic
/// diagnostics, or regions that have been evicted) fall back to `fallback_text`.
///
/// # Parameters
/// * `lsp_diags`        — Output vector; diagnostics are appended, not replaced.
/// * `diags`            — Core diagnostics produced by parsing / name-resolution / type-checking.
/// * `arena`            — Region arena for the document being analyzed; used to resolve
///   the correct source text per diagnostic.
/// * `fallback_text`    — Main document text, used when no matching region is found.
/// * `fallback_doc_len` — Length of the main document, used to clamp spans safely.
/// * `source`           — Value for the LSP `source` field (e.g. `"chrn-parser"`).
pub(crate) fn push_diagnostics(
    lsp_diags: &mut Vec<tower_lsp::lsp_types::Diagnostic>,
    diags: &[SourceDiagnostic],
    arena: &Arena<SourceRegion, SourceRegionId>,
    fallback_text: &str,
    fallback_doc_len: usize,
    source: &str,
) {
    for core_diag in diags {
        // Resolve the correct source text for THIS diagnostic. A diagnostic
        // originating in an imported module has spans in that module's bytes,
        // not the main document's, so we must look up the matching region.
        //
        // The returned `script_start` is added to the (relative) diagnostic
        // spans to put them in absolute file coordinates.  After that shift,
        // `fallback_text` (the whole document) can be used to convert byte
        // offsets into LSP `Position`s that line up with what the editor shows.
        let (rel_text, doc_len, script_start) =
            resolve_diag_text(arena, core_diag, fallback_text, fallback_doc_len);

        // Whether the resolved region is the main document (where
        // `fallback_text` matches the region's bytes) or a sub-module, the
        // resulting LSP positions must be in the absolute file coordinate
        // system.  We always use `fallback_text` for the final `Position`
        // conversion so the line/column reflects the whole document the
        // editor is showing, with `script_start` shifting the byte offset.
        let text = fallback_text;
        let effective_doc_len = fallback_doc_len;

        let severity = match core_diag.level {
            DiagnosticLevel::Error => lsp_types::DiagnosticSeverity::ERROR,
            DiagnosticLevel::Warn => lsp_types::DiagnosticSeverity::WARNING,
            DiagnosticLevel::Note => lsp_types::DiagnosticSeverity::INFORMATION,
            DiagnosticLevel::Help => lsp_types::DiagnosticSeverity::HINT,
        };

        let (start_byte, end_byte) = if let Some(annotation) = core_diag
            .annotations
            .iter()
            .find(|a| matches!(a.kind, AnnotationKind::Primary))
            .or_else(|| core_diag.annotations.first())
        {
            // `annotation.span` is relative to the region's `src_bytes`; shift
            // to absolute file coordinates and clamp to the document length.
            let s = (crate::text::rel_to_abs_offset(annotation.span.start, script_start) as usize)
                .min(effective_doc_len);
            let e = (crate::text::rel_to_abs_offset(annotation.span.end, script_start) as usize)
                .min(effective_doc_len);
            (s, e)
        } else {
            (0, 0)
        };

        let start_pos = crate::text::offset_to_position(text, start_byte);
        let end_pos = crate::text::offset_to_position(text, end_byte);

        lsp_diags.push(tower_lsp::lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: start_pos,
                end: end_pos,
            },
            severity: Some(severity),
            source: Some(source.to_string()),
            message: core_diag.core_msg.clone(),
            ..Default::default()
        });

        for annotation in &core_diag.annotations {
            let msg = match &annotation.label {
                Some(label) => label.clone(),
                None => match annotation.kind {
                    AnnotationKind::Primary => continue,
                    AnnotationKind::Secondary => "".to_string(),
                    AnnotationKind::Note => "note: ".to_string(),
                    AnnotationKind::Help => "help: ".to_string(),
                },
            };

            // The annotation span is relative to the region's `src_bytes`;
            // shift to absolute file coordinates (using the resolved
            // `script_start`) and clamp to the document length.
            //
            // When the region is the main document, `script_start` may be 0
            // (no `@def`) or the position of `@` (with `@def`); in both
            // cases the shift lands the byte offset in absolute coordinates
            // against the full `fallback_text`.
            let ann_start = (crate::text::rel_to_abs_offset(annotation.span.start, script_start)
                as usize)
                .min(effective_doc_len);
            let ann_end = (crate::text::rel_to_abs_offset(annotation.span.end, script_start)
                as usize)
                .min(effective_doc_len);
            let ann_sev = match annotation.kind {
                AnnotationKind::Primary => severity,
                AnnotationKind::Secondary | AnnotationKind::Help => {
                    lsp_types::DiagnosticSeverity::HINT
                }
                AnnotationKind::Note => lsp_types::DiagnosticSeverity::INFORMATION,
            };
            // `rel_text` and `doc_len` are still part of this scope in case
            // future variants of the diagnostic pipeline need to look up
            // other text-shaped fields by relative offset.
            let _ = (rel_text, doc_len);
            lsp_diags.push(tower_lsp::lsp_types::Diagnostic {
                range: lsp_types::Range {
                    start: crate::text::offset_to_position(text, ann_start),
                    end: crate::text::offset_to_position(text, ann_end),
                },
                severity: Some(ann_sev),
                source: Some(source.to_string()),
                message: msg,
                ..Default::default()
            });
        }

        for note in &core_diag.notes {
            lsp_diags.push(tower_lsp::lsp_types::Diagnostic {
                range: lsp_types::Range {
                    start: start_pos,
                    end: end_pos,
                },
                severity: Some(lsp_types::DiagnosticSeverity::INFORMATION),
                source: Some(source.to_string()),
                message: note.clone(),
                ..Default::default()
            });
        }

        for help_msg in &core_diag.help {
            lsp_diags.push(tower_lsp::lsp_types::Diagnostic {
                range: lsp_types::Range {
                    start: start_pos,
                    end: end_pos,
                },
                severity: Some(lsp_types::DiagnosticSeverity::HINT),
                source: Some(source.to_string()),
                message: help_msg.clone(),
                ..Default::default()
            });
        }
    }
}

/// Recursively resolves all imports declared in `prev_mod`, building the flat module
/// list required by [`ScriptCompiler`](compilation::script_compiler::ScriptCompiler).
///
/// The function walks each `ImportKind::Source` import, attempts to obtain the source
/// bytes from the open-document cache (in-memory) or from disk (fallback), parses the
/// imported file, and calls itself recursively for that file's own imports.
///
/// # Parameters
/// * `reserved_mod_ids` — Global registry mapping file paths to pre-assigned module
///   IDs.  Updated as new imports are discovered during traversal.
/// * `seen`             — Guard set of already-visited path IDs to break import cycles.
/// * `modules`          — Output slot array indexed by `ModuleId - 1`; entries are
///   `None` until a module is successfully loaded.
/// * `sub_regions`      — Output vector of imported module source regions.  Region
///   ids are `1 + index` because id `0` is reserved for the main document region.
/// * `prev_mod`         — The module whose imports are to be resolved in this call.
/// * `settings`         — Global compiler settings forwarded to `ChrnConfigLoader` and
///   `ModuleFinder`.
/// * `interner`         — Shared string/path interner for all modules being resolved.
/// * `doc_cache`        — Cache queried for in-memory document text before falling
///   back to disk I/O.
/// * `diags`            — Accumulator for any import-related diagnostics (path errors,
///   IO errors, parse errors in imported files).
/// * `current_path_id`  — The [`PathId`] of `prev_mod` (the module containing the
///   `import` statement).  Import errors that point at the import path span must use
///   this `path_id` so their spans are resolved against the correct region when they
///   are later surfaced through [`push_diagnostics`].
///
/// # Errors
/// All errors are appended to `diags` rather than returned.  The function always
/// attempts to continue resolving remaining siblings after an error.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_modules_lsp(
    reserved_mod_ids: &mut Vec<(PathId, ModuleId)>,
    seen: &mut Vec<PathId>,
    modules: &mut Vec<Option<Module>>,
    sub_regions: &mut Vec<SourceRegion>,
    prev_mod: &Module,
    settings: &ChrnConfig,
    interner: &mut Intern,
    doc_cache: &DocumentCache,
    diags: &mut Vec<SourceDiagnostic>,
    current_path_id: PathId,
) {
    for import in &prev_mod.imports {
        let ImportKind::Source(path_id, path_span) = import.kind else {
            continue;
        };

        if seen.contains(&path_id) {
            continue;
        }

        seen.push(path_id);

        let current_mod_id = reserved_mod_ids
            .iter()
            .find(|(p_id, _)| *p_id == path_id)
            .map(|(_, m_id)| *m_id)
            .expect("Previous registration failed");

        let path_owned = interner.search_path(path_id).to_path_buf();
        let path = path_owned.as_path();

        // Try to get from doc_cache first
        let uri = Url::from_file_path(path).unwrap();
        // Keep the cached `Arc<String>` alive in a local so the reader can borrow
        // from it directly.  Previously the bytes were copied into a fresh
        // `Vec<u8>` (`text.as_bytes().to_vec()`) on *every* analysis of *every*
        // importing document, just to satisfy the boxed-reader type.  The box now
        // borrows from `cached_text`, which outlives the `ConfigLoader` below.
        let cached_text = doc_cache.get_text(uri.as_ref());
        let source_res: Result<Box<dyn std::io::Read + '_>, ConfigLoadError> =
            match &cached_text {
                Some(text) => Ok(Box::new(Cursor::new(text.as_bytes()))),
                None => {
                    // Fallback to disk
                    match std::fs::File::open(path) {
                        Ok(_) if path.is_dir() => {
                            let core_msg =
                                format!("The path \"{}\" is a directory", path.display());
                            let src_diag = SourceDiagnostic::builder(
                                DiagnosticLevel::Error,
                                core_msg,
                                current_path_id,
                            )
                            .add_annotation(
                                path_span,
                                AnnotationKind::Primary,
                                "Caused by this import".to_string().into(),
                            )
                            .build();
                            Err(ConfigLoadError::Diagnostic(src_diag))
                        }
                        Ok(f) => Ok(Box::new(f) as Box<dyn std::io::Read + '_>),
                        Err(e) => {
                            let core_msg = core_error::form_string_from_io_err(&e, path)
                                .unwrap_or(e.to_string());
                            let src_diag = SourceDiagnostic::builder(
                                DiagnosticLevel::Error,
                                core_msg,
                                current_path_id,
                            )
                            .add_annotation(
                                path_span,
                                AnnotationKind::Primary,
                                "Caused by this import".to_string().into(),
                            )
                            .build();
                            Err(ConfigLoadError::Diagnostic(src_diag))
                        }
                    }
                }
            };

        let src = match source_res {
            Ok(s) => s,
            Err(ConfigLoadError::Diagnostic(diag)) => {
                diags.push(diag);
                continue;
            }
            Err(ConfigLoadError::IO(e)) => {
                let core_msg = format!("IO error: {}", e);
                let src_diag =
                    SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, current_path_id)
                        .add_annotation(path_span, AnnotationKind::Primary, None)
                        .build();
                diags.push(src_diag);
                continue;
            }
        };

        // `ConfigLoader::new` requires the region's id up front.  Sub-regions are
        // stored in `sub_regions` with ids `1 + index` because id 0 is the main
        // document region.
        let sub_region_id = SourceRegionId::new((sub_regions.len() + 1) as u32);

        let sub_region = match ConfigLoader::new(sub_region_id, src, path_id, settings, interner)
            .load_config()
        {
            ConfigLoaderOutput::Success(region) => region,
            ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                match cfg_err {
                    ConfigLoadError::Diagnostic(diag) => {
                        diags.push(diag);
                    }
                    ConfigLoadError::IO(e) => {
                        let path = interner.search_path(path_id);
                        let core_msg =
                            core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());
                        let src_diag = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            current_path_id,
                        )
                        .add_annotation(path_span, AnnotationKind::Primary, None)
                        .build();
                        diags.push(src_diag);
                    }
                }
                broken_region
            }
            ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                match cfg_err {
                    ConfigLoadError::Diagnostic(diag) => {
                        diags.push(diag);
                    }
                    ConfigLoadError::IO(e) => {
                        let path = interner.search_path(path_id);
                        let core_msg =
                            core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());
                        let src_diag = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            current_path_id,
                        )
                        .add_annotation(path_span, AnnotationKind::Primary, None)
                        .build();
                        diags.push(src_diag);
                    }
                }
                continue;
            }
        };

        let file_name = match path.file_prefix().and_then(|n| n.to_str()) {
            Some(p) => p.to_string(),
            _ => {
                if let Some(name_id) = import.alias_id {
                    interner.search(name_id).to_string()
                } else {
                    let core_msg = format!(
                        "The path \"{}\" does not have a valid UTF-8 file name usable within the program.",
                        path.display()
                    );
                    let src_diag = SourceDiagnostic::builder(
                        DiagnosticLevel::Error,
                        core_msg.clone(),
                        current_path_id,
                    )
                    .add_annotation(
                        path_span,
                        AnnotationKind::Primary,
                        "Caused by this import".to_string().into(),
                    )
                    .build();
                    diags.push(src_diag);
                    continue;
                }
            }
        };

        let sub_mod_name_id = interner.intern(&file_name);

        let (_, sub_imports, mut finder_diags) = ModuleFinder::new(
            &sub_region.src_bytes,
            settings,
            reserved_mod_ids,
            &sub_region,
            sub_region.script_start,
            sub_region.serial_start,
        )
        .collect_imports(interner);

        diags.append(&mut finder_diags);

        let expected_len = reserved_mod_ids.len() - 1;

        if modules.len() < expected_len {
            modules.resize(expected_len, None);
        }

        sub_regions.push(sub_region);
        debug_assert_eq!(
            SourceRegionId::new(sub_regions.len() as u32),
            sub_region_id,
            "Sub-region id must match the pre-computed id"
        );

        let sub_mod = Module::new(
            sub_mod_name_id,
            ModuleState::Loaded,
            current_mod_id,
            sub_imports,
            Some(sub_region_id),
        );

        resolve_modules_lsp(
            reserved_mod_ids,
            seen,
            modules,
            sub_regions,
            &sub_mod,
            settings,
            interner,
            doc_cache,
            diags,
            path_id,
        );

        modules[(current_mod_id.id - 1) as usize] = Some(sub_mod);
    }
}
