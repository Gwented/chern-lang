use parking_lot::RwLock;
use script_lib::config_loader::ChrnConfigLoader;
use script_lib::modules::Module;
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::Client;
use tower_lsp::lsp_types::*;

use chrn_utils::id_types::{InternedId, ModuleId, PathId};
use chrn_utils::intern::Intern;
use common::chrn_settings::ChrnSettings;
use script_lib::modules::mod_finder::ModuleFinder;
use std::io::Cursor;
use tower_lsp::lsp_types::Url;

use crate::state::DocumentCache;

const MAX_DIAGS_CACHE_SIZE: usize = 100;

struct ArcReader {
    data: Arc<String>,
    pos: usize,
}

impl std::io::Read for ArcReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = &self.data.as_bytes()[self.pos..];
        let n = std::cmp::min(buf.len(), remaining.len());
        buf[..n].copy_from_slice(&remaining[..n]);
        self.pos += n;
        Ok(n)
    }
}

fn evict_cache_if_needed(cache: &mut HashMap<String, String>) {
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

pub async fn analyze_and_publish_task(
    client: Client,
    uri: Url,
    text: Arc<String>,
    diags_cache: Arc<RwLock<HashMap<String, String>>>,
    doc_cache: Arc<DocumentCache>,
    pending_versions: Arc<RwLock<HashMap<String, u64>>>,
    version: u64,
) {
    let settings = ChrnSettings::default();

    let path_buf = uri
        .to_file_path()
        .unwrap_or_else(|_| PathBuf::from(uri.path()));

    let mut interner = Intern::init();
    let path_id = PathId::new(interner.intern_path(&path_buf));
    // 1. Initial config load to find boundaries
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
            // Handle config load error (same as before)
            let start = Position {
                line: 0,
                character: 0,
            };
            let diag = match e {
                common::core_error::ConfigLoadError::Unclosed(diag)
                | common::core_error::ConfigLoadError::Module(diag) => {
                    let diag_span = diag.span.unwrap_or_default();
                    let start_pos = crate::text::offset_to_position(&text, diag_span.start);
                    let end_pos = crate::text::offset_to_position(&text, diag_span.end);
                    tower_lsp::lsp_types::Diagnostic {
                        range: Range {
                            start: start_pos,
                            end: end_pos,
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some("chrn-config".to_string()),
                        message: diag.core_msg,
                        ..Default::default()
                    }
                }
                common::core_error::ConfigLoadError::IO(io) => tower_lsp::lsp_types::Diagnostic {
                    range: Range { start, end: start },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("chrn-config".to_string()),
                    message: io.to_string(),
                    ..Default::default()
                },
            };
            publish_if_current(
                &client,
                &uri,
                vec![diag],
                &diags_cache,
                &pending_versions,
                version,
            )
            .await;
            return;
        }
    };

    // 2. Use DocumentCache for heavy lifting
    let state_arc = doc_cache.get_or_create(
        &uri.to_string(),
        text,
        metadata.script_start,
        metadata.serial_start,
        version,
    );

    let imported_uris = {
        let mut state = state_arc.write();
        state.ensure_analyzed(&doc_cache, &path_buf)
    };

    if !imported_uris.is_empty() {
        doc_cache.register_dependencies(&uri.to_string(), &imported_uris);
    }

    // 3. Get diagnostics and publish if still current
    let lsp_diags = state_arc.read().get_lsp_diagnostics();
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

async fn publish_if_current(
    client: &Client,
    uri: &Url,
    lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic>,
    diags_cache: &RwLock<HashMap<String, String>>,
    pending_versions: &RwLock<HashMap<String, u64>>,
    version: u64,
) {
    // Check version
    {
        let vers = pending_versions.read();
        if let Some(&v) = vers.get(&uri.to_string()) {
            if v != version {
                return; // Newer version exists, discard these results
            }
        }
    }

    // Cache check
    if let Ok(serialized) = serde_json::to_string(&lsp_diags) {
        let key = uri.to_string();
        let should_send = {
            let mut cache = diags_cache.write();
            evict_cache_if_needed(&mut cache);
            match cache.get(&key) {
                Some(prev) if prev == &serialized => false,
                _ => {
                    cache.insert(key, serialized);
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

pub(crate) fn push_diagnostics(
    lsp_diags: &mut Vec<tower_lsp::lsp_types::Diagnostic>,
    diags: &[common::reporter::diagnostic::Diagnostic],
    doc_len: usize,
    text: &str,
    source: &str,
) {
    for diag in diags {
        let (start_byte, end_byte) = match diag.span {
            Some(span) => {
                let s = span.start.min(doc_len);
                let e = span.end.saturating_add(1).min(doc_len);
                (s, e)
            }
            None => (0, 0),
        };

        let start_pos = crate::text::offset_to_position(text, start_byte);
        let end_pos = crate::text::offset_to_position(text, end_byte);

        let diag = tower_lsp::lsp_types::Diagnostic {
            range: Range {
                start: start_pos,
                end: end_pos,
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some(source.to_string()),
            message: diag.core_msg.clone(),
            related_information: None,
            tags: None,
            data: None,
        };

        lsp_diags.push(diag);
    }
}

pub(crate) fn resolve_modules_lsp(
    seen: &mut std::collections::HashSet<PathId>,
    modules: &mut Vec<Module>,
    prev_mod: &Module,
    mod_map: &mut HashMap<InternedId, ModuleId>,
    settings: &ChrnSettings,
    interner: &mut Intern,
    doc_cache: &DocumentCache,
) -> Result<(), common::core_error::ConfigLoadError> {
    for import in &prev_mod.imports {
        if seen.contains(&import.path_id) {
            continue;
        }

        let current_mod_id = seen.len();
        seen.insert(import.path_id);

        let path_owned = interner
            .search_path(import.path_id.id as usize)
            .to_path_buf();
        let path = path_owned.as_path();

        // Try to get from doc_cache first
        let uri = Url::from_file_path(path).unwrap();
        let source_res: Result<Box<dyn std::io::Read + Send>, common::core_error::ConfigLoadError> =
            if let Some(text) = doc_cache.get_text(&uri.to_string()) {
                Ok(Box::new(ArcReader { data: text, pos: 0 }))
            } else {
                // Fallback to disk
                std::fs::File::open(path)
                    .map(|f| Box::new(f) as Box<dyn std::io::Read + Send>)
                    .map_err(|e| {
                        let core_msg = common::core_error::form_string_from_io_err(&e, path)
                            .unwrap_or(e.to_string());
                        let metadata = prev_mod.src_metadata.as_ref().unwrap();
                        let ln_data = common::reporter::form_err_diag(
                            &metadata.src_bytes,
                            &[import.path_span],
                            settings.can_color,
                        );
                        let prev_path = interner.search_path(metadata.path_id.id as usize);
                        let fmtted_diag = common::reporter::standardize_err(
                            &core_msg,
                            &ln_data,
                            None,
                            prev_path,
                            settings.can_color,
                        );
                        common::core_error::ConfigLoadError::Module(
                            common::reporter::diagnostic::Diagnostic::new(
                                path,
                                core_msg,
                                Some(import.path_span),
                                None,
                                fmtted_diag,
                                common::reporter::diagnostic::Area::ConfigLoad,
                            ),
                        )
                    })
            };

        let src = match source_res {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        let mod_metadata =
            ChrnConfigLoader::new(import.path_id, src, settings, interner).load_config()?;

        let file_name = match path.file_stem().and_then(|n| n.to_str()) {
            Some(p) => p.to_string(),
            _ => {
                if let Some(name_id) = import.alias_id {
                    interner.search(name_id.id as usize).to_string()
                } else {
                    let core_msg = format!(
                        "The path \"{}\" does not have a valid UTF-8 file name usable within the program.",
                        path.display()
                    );
                    let diag = common::reporter::diagnostic::Diagnostic::new(
                        path,
                        core_msg.clone(),
                        None,
                        None,
                        core_msg,
                        common::reporter::diagnostic::Area::ConfigLoad,
                    );
                    return Err(common::core_error::ConfigLoadError::Module(diag));
                }
            }
        };

        let name_id = InternedId::new(interner.intern(&file_name));
        let metadata = prev_mod.src_metadata.as_ref().unwrap();
        let origin = interner.search_path(metadata.path_id.id as usize);

        let (_, sub_imports) = ModuleFinder::new(
            &mod_metadata.src_bytes,
            settings,
            origin.to_path_buf(),
            mod_metadata.script_start,
            mod_metadata.serial_start,
        )
        .collect_imports(interner)?;

        let sub_mod = Module::new(
            name_id,
            ModuleId::new(current_mod_id),
            sub_imports,
            Some(mod_metadata),
        );

        if let Some(alias_id) = import.alias_id {
            mod_map.insert(alias_id, ModuleId::new(current_mod_id));
        }

        resolve_modules_lsp(
            seen, modules, &sub_mod, mod_map, settings, interner, doc_cache,
        )?;

        modules.push(sub_mod);
        mod_map.insert(name_id, ModuleId::new(current_mod_id));
    }

    Ok(())
}
