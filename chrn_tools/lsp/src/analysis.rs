use parking_lot::RwLock;
use script_lib::config_loader::ChernConfigLoader;
use script_lib::script_compiler::ScriptCompiler;
use script_lib::semantic::name_resolver::NamespaceResolver;
use script_lib::semantic::type_resolver::TypeResolver;
use script_lib::semantic::type_resolver::type_context::TypeContext;
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::Client;
use tower_lsp::lsp_types::*;

use chrn_utils::id_types::{InternedId, ModuleId, PathId};
use chrn_utils::intern::Intern;
use common::chrn_settings::ChernSettings;
use script_lib::lexer::Lexer;
use script_lib::modules::Module;
use std::io::Cursor;

const MAX_DIAGS_CACHE_SIZE: usize = 100;

fn evict_cache_if_needed(cache: &mut HashMap<String, String>) {
    if cache.len() >= MAX_DIAGS_CACHE_SIZE {
        let to_remove = cache.len() - MAX_DIAGS_CACHE_SIZE + 10;
        let keys: Vec<_> = cache.keys().take(to_remove).cloned().collect();
        for key in keys {
            cache.remove(&key);
        }
    }
}

pub async fn analyze_and_publish_task(
    client: Client,
    uri: Url,
    text: String,
    diags_cache: Arc<RwLock<HashMap<String, String>>>,
) {
    // Create interner and minimal module so parser can produce diagnostics
    let mut interner = Intern::init();

    let settings = ChernSettings::default();

    // prepare module metadata and module
    let src_bytes = text.as_bytes().to_vec();

    // derive a path to intern for diagnostics; try to convert uri to file path
    let path_buf = uri
        .to_file_path()
        .unwrap_or_else(|_| PathBuf::from(uri.path()));

    // Try to reuse the same config loader logic used by the CLI to determine the script
    // start/serial boundaries. We do this in-memory (no file IO) using a Cursor over bytes.
    let metadata = match ChernConfigLoader::new(
        path_buf.as_path(),
        Cursor::new(src_bytes.clone()),
        &settings,
    )
    .load_config()
    {
        Ok(m) => m,
        // I DID NOT ASK

        // If config loading fails, surface a diagnostic to the user instead of silently
        // falling back. This indicates a syntax/configuration problem the language can't
        // recover from (e.g. unclosed quotes, missing @end), so publish it as an error and
        // return early.
        Err(e) => {
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
                        code: None,
                        code_description: None,
                        source: Some("chern-config".to_string()),
                        message: diag.core_msg.clone(),
                        related_information: None,
                        tags: None,
                        data: None,
                    }
                }
                common::core_error::ConfigLoadError::IO(io) => tower_lsp::lsp_types::Diagnostic {
                    range: Range { start, end: start },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("chern-config".to_string()),
                    message: io.to_string(),
                    related_information: None,
                    tags: None,
                    data: None,
                },
            };

            // serialize diagnostics and compare to cache to avoid noisy re-publishes
            let diags_vec = vec![diag];
            if let Ok(serialized) = serde_json::to_string(&diags_vec) {
                let key = uri.to_string();
                let should_send = {
                    let mut cache = diags_cache.write();
                    evict_cache_if_needed(&mut cache);
                    match cache.get(&key) {
                        Some(prev) if prev == &serialized => false,
                        _ => {
                            cache.insert(key.clone(), serialized.clone());
                            true
                        }
                    }
                };

                if should_send {
                    client
                        .publish_diagnostics(uri.clone(), diags_vec, None)
                        .await;
                }
            } else {
                client
                    .publish_diagnostics(uri.clone(), diags_vec, None)
                    .await;
            }

            return;
        }
    };

    // Build diagnostics in a narrower scope so large temporaries can be dropped
    // before awaiting on client publishing. This reduces peak memory usage.
    let lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = {
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

        let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();

        let parse_result =
            script_lib::parser::parse(&settings, &compiler.mods[0], &toks, &interner);

        let (ast_info, parse_errors) = match parse_result {
            Ok(ast_info) => (ast_info, None),
            Err((partial_ast, err)) => (partial_ast, Some(err)),
        };

        if let Some(err) = parse_errors {
            match err {
                common::core_error::ScriptError::Parser(mut diags)
                | common::core_error::ScriptError::Semantic(mut diags) => {
                    for diag in diags.drain(..) {
                        let doc_len = compiler.mods[0].metadata.src_bytes.len();
                        let (start_byte, end_byte) = match diag.span {
                            Some(span) => {
                                let s = span.start.min(doc_len);
                                let e = span.end.saturating_add(1).min(doc_len);
                                (s, e)
                            }
                            None => (0, 0),
                        };

                        let start_pos = crate::text::offset_to_position(&text, start_byte);
                        let end_pos = crate::text::offset_to_position(&text, end_byte);

                        let diag = tower_lsp::lsp_types::Diagnostic {
                            range: Range {
                                start: start_pos,
                                end: end_pos,
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: None,
                            code_description: None,
                            source: Some("chern-parser".to_string()),
                            message: diag.core_msg,
                            related_information: None,
                            tags: None,
                            data: None,
                        };

                        lsp_diags.push(diag);
                    }
                }
                _ => {
                    let start = Position {
                        line: 0,
                        character: 0,
                    };
                    let diag = tower_lsp::lsp_types::Diagnostic {
                        range: Range { start, end: start },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: Some("chern-parser".to_string()),
                        message: format!("Parser error: {:?}. -UNFINISHED-", err),
                        related_information: None,
                        tags: None,
                        data: None,
                    };

                    lsp_diags.push(diag);
                }
            }
        }

        let mut ns_resolver = NamespaceResolver::new(
            &settings,
            &ast_info,
            &interner,
            ModuleId::new(0),
            &mut compiler,
        );

        if let Err(ns_diags) = ns_resolver.resolve() {
            for diag in ns_diags {
                let (start_byte, end_byte) = match diag.span {
                    Some(span) => {
                        let doc_len = compiler.mods[0].metadata.src_bytes.len();
                        let s = span.start.min(doc_len);
                        let e = span.end.saturating_add(1).min(doc_len);
                        (s, e)
                    }
                    None => (0, 0),
                };

                let start_pos = crate::text::offset_to_position(&text, start_byte);
                let end_pos = crate::text::offset_to_position(&text, end_byte);

                let lsp_diag = tower_lsp::lsp_types::Diagnostic {
                    range: Range {
                        start: start_pos,
                        end: end_pos,
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    source: Some("chern-namespace".to_string()),
                    message: diag.core_msg,
                    related_information: None,
                    tags: None,
                    data: None,
                };
                lsp_diags.push(lsp_diag);
            }
        }

        if lsp_diags.is_empty() {
            let mut ty_ctx = TypeContext::new();
            let mut type_resolver = TypeResolver::new(
                &settings,
                &ast_info,
                ModuleId::new(0),
                &mut ty_ctx,
                &interner,
                &mut compiler,
            );

            if let Err(ty_diags) = type_resolver.resolve() {
                for diag in ty_diags {
                    let (start_byte, end_byte) = match diag.span {
                        Some(span) => {
                            let doc_len = compiler.mods[0].metadata.src_bytes.len();
                            let s = span.start.min(doc_len);
                            let e = span.end.saturating_add(1).min(doc_len);
                            (s, e)
                        }
                        None => (0, 0),
                    };

                    let start_pos = crate::text::offset_to_position(&text, start_byte);
                    let end_pos = crate::text::offset_to_position(&text, end_byte);

                    let diag = tower_lsp::lsp_types::Diagnostic {
                        range: Range {
                            start: start_pos,
                            end: end_pos,
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: Some("chern-type".to_string()),
                        message: diag.core_msg,
                        related_information: None,
                        tags: None,
                        data: None,
                    };

                    lsp_diags.push(diag);
                }
            }
        }

        lsp_diags
    };

    // Use caching to avoid publishing identical diagnostic payloads repeatedly.
    if let Ok(serialized) = serde_json::to_string(&lsp_diags) {
        let key = uri.to_string();
        let should_send = {
            let mut cache = diags_cache.write();
            evict_cache_if_needed(&mut cache);
            match cache.get(&key) {
                Some(prev) if prev == &serialized => false,
                _ => {
                    cache.insert(key.clone(), serialized.clone());
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
