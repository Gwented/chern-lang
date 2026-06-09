use compilation::modules::ImportKind;
use compilation::modules::Module;
use lang::config_loader::ChrnConfigLoader;
use parking_lot::RwLock;
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::Client;
use tower_lsp::lsp_types;

use chrn_utils::chrn_settings::ChrnSettings;
use chrn_utils::id_types::{ModuleId, PathId, SourceRegionId};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::{
    AnnotationKind, DiagnosticLevel, SourceDiagnostic,
};
use chrn_utils::source_map::source_region::SourceRegionArena;
use compilation::modules::mod_finder::ModuleFinder;
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

/// Convert a `ConfigLoadError` into LSP diagnostics (error + optional hint).
pub(crate) fn config_load_error_to_diagnostics(
    err: chrn_utils::core_error::ConfigLoadError,
    text: &str,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let source = "chrn-config";
    let start = lsp_types::Position {
        line: 0,
        character: 0,
    };

    match err {
        chrn_utils::core_error::ConfigLoadError::General(diag) => {
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
                let s_pos = crate::text::offset_to_position(text, annotation.span.start as usize);
                let e_pos =
                    crate::text::offset_to_position(text, (annotation.span.end + 1) as usize);
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

                let ann_start =
                    crate::text::offset_to_position(text, annotation.span.start as usize);
                let ann_end =
                    crate::text::offset_to_position(text, (annotation.span.end + 1) as usize);
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
    let path_id = interner.intern_path(&path_buf);
    // 1. Initial config load to find boundaries
    let region = match ChrnConfigLoader::new(
        SourceRegionId::new(0),
        Cursor::new(text.as_bytes()),
        path_id,
        &settings,
        &mut interner,
    )
    .load_config()
    {
        Ok(m) => m,
        Err(e) => {
            let diags = config_load_error_to_diagnostics(e, &text);
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

    // 2. Use DocumentCache for heavy lifting
    let state_arc = doc_cache.get_or_create(
        &uri.to_string(),
        text,
        region.script_start,
        region.serial_start,
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
    diags: &[SourceDiagnostic],
    doc_len: usize,
    text: &str,
    source: &str,
) {
    for core_diag in diags {
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
            let s = (annotation.span.start as usize).min(doc_len);
            let e = (annotation.span.end as usize)
                .saturating_add(1)
                .min(doc_len);
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

            let ann_start = (annotation.span.start as usize).min(doc_len);
            let ann_end = (annotation.span.end as usize)
                .saturating_add(1)
                .min(doc_len);
            let ann_sev = match annotation.kind {
                AnnotationKind::Primary => severity,
                AnnotationKind::Secondary | AnnotationKind::Help => {
                    lsp_types::DiagnosticSeverity::HINT
                }
                AnnotationKind::Note => lsp_types::DiagnosticSeverity::INFORMATION,
            };
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

pub(crate) fn resolve_modules_lsp(
    reserved_mod_ids: &mut Vec<(PathId, ModuleId)>,
    seen: &mut Vec<PathId>,
    modules: &mut Vec<Option<Module>>,
    prev_mod: &Module,
    settings: &ChrnSettings,
    interner: &mut Intern,
    doc_cache: &DocumentCache,
    region_arena: &mut SourceRegionArena,
    diags: &mut Vec<SourceDiagnostic>,
) {
    use chrn_utils::core_error::{self, ConfigLoadError};
    use compilation::modules::ModuleState;

    for import in &prev_mod.imports {
        let ImportKind::Source(path_id, path_span) = import.kind else {
            continue;
        };

        if seen.iter().any(|p_id| *p_id == path_id) {
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
        let source_res: Result<Box<dyn std::io::Read + Send>, ConfigLoadError> =
            if let Some(text) = doc_cache.get_text(&uri.to_string()) {
                Ok(Box::new(ArcReader { data: text, pos: 0 }))
            } else {
                // Fallback to disk
                match std::fs::File::open(path) {
                    Ok(_) if path.is_dir() => {
                        let core_msg = format!("The path \"{}\" is a directory", path.display());
                        let src_diag =
                            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
                                .add_annotation(
                                    path_span,
                                    AnnotationKind::Primary,
                                    "Caused by this import".to_string().into(),
                                )
                                .build();
                        Err(ConfigLoadError::General(src_diag))
                    }
                    Ok(f) => Ok(Box::new(f) as Box<dyn std::io::Read + Send>),
                    Err(e) => {
                        let core_msg =
                            core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());
                        let src_diag =
                            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
                                .add_annotation(
                                    path_span,
                                    AnnotationKind::Primary,
                                    "Caused by this import".to_string().into(),
                                )
                                .build();
                        Err(ConfigLoadError::General(src_diag))
                    }
                }
            };

        let src = match source_res {
            Ok(s) => s,
            Err(ConfigLoadError::General(diag)) => {
                diags.push(diag);
                continue;
            }
            Err(ConfigLoadError::IO(e)) => {
                let core_msg = format!("IO error: {}", e);
                let src_diag = SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
                    .add_annotation(path_span, AnnotationKind::Primary, None)
                    .build();
                diags.push(src_diag);
                continue;
            }
        };

        let sub_region_id = SourceRegionId::new(region_arena.regions.len() as u32);

        let sub_region =
            match ChrnConfigLoader::new(sub_region_id, src, path_id, settings, interner)
                .load_config()
            {
                Ok(reg) => reg,
                Err(cfg_err) => {
                    match cfg_err {
                        ConfigLoadError::General(diag) => {
                            diags.push(diag);
                        }
                        ConfigLoadError::IO(e) => {
                            let path = interner.search_path(path_id);
                            let core_msg = core_error::form_string_from_io_err(&e, path)
                                .unwrap_or(e.to_string());
                            let src_diag = SourceDiagnostic::builder(
                                DiagnosticLevel::Error,
                                core_msg,
                                path_id,
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
                        path_id,
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

        region_arena.regions.push(sub_region);

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
            &sub_mod,
            settings,
            interner,
            doc_cache,
            region_arena,
            diags,
        );

        modules[current_mod_id.id - 1] = Some(sub_mod);
    }
}
