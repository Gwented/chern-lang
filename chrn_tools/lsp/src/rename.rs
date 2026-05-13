use crate::state::{DocumentCache, SemanticEntity};
use crate::text::{offset_to_position, position_to_offset};
use std::collections::HashMap;
use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};

pub fn compute_rename(
    uri: &Url,
    position: Position,
    new_name: String,
    doc_cache: &DocumentCache,
) -> Option<WorkspaceEdit> {
    let uri_str = uri.to_string();
    let state_arc = doc_cache.get(&uri_str)?;
    let state = state_arc.read();

    let byte_offset = position_to_offset(&state.text, position);

    if state.offset_in_comment(byte_offset) {
        return None;
    }

    let entity = state.get_entity_at_offset(byte_offset)?;

    // We don't support renaming modules yet as it usually implies renaming files
    if matches!(entity, SemanticEntity::Module(_)) {
        return None;
    }

    let (def_path, def_span, def_owner_sym_id) = state.get_definition_location(entity)?;
    let is_local = matches!(entity, SemanticEntity::Local { .. });

    let mut changes = HashMap::new();

    if is_local {
        // Optimization: for locals, we only need to look at the current file
        let mut edits = Vec::new();
        for (span, ent) in &state.symbol_map {
            if let SemanticEntity::Local {
                decl_span,
                owner_sym_id,
                ..
            } = ent
            {
                if *decl_span == def_span && *owner_sym_id == def_owner_sym_id {
                    edits.push(TextEdit {
                        range: Range {
                            start: offset_to_position(&state.text, span.start),
                            end: offset_to_position(&state.text, span.end + 1),
                        },
                        new_text: new_name.clone(),
                    });
                }
            }
        }
        if !edits.is_empty() {
            changes.insert(uri.clone(), edits);
        }
    } else {
        // Cross-module rename: search all cached documents
        doc_cache.for_each_state(|state_uri, other_state_arc| {
            let other_state = other_state_arc.read();
            let mut file_edits: Vec<(Range, String)> = Vec::new();

            for (span, ent) in &other_state.symbol_map {
                if let Some((other_def_path, other_def_span, other_def_owner_sym_id)) =
                    other_state.get_definition_location(ent)
                {
                    if other_def_path == def_path
                        && other_def_span == def_span
                        && other_def_owner_sym_id == def_owner_sym_id
                    {
                        let range = Range {
                            start: offset_to_position(&other_state.text, span.start),
                            end: offset_to_position(&other_state.text, span.end + 1),
                        };
                        file_edits.push((range, new_name.clone()));
                    }
                }
            }

            if !file_edits.is_empty() {
                // Deduplicate: if we have overlapping ranges for the same definition,
                // keep only the most specific ones (smallest ranges).
                // A range is redundant if there exists another range for the same definition
                // that is entirely contained within it.
                let mut final_edits = Vec::new();
                for i in 0..file_edits.len() {
                    let (r1, _) = &file_edits[i];
                    let mut is_redundant = false;
                    for j in 0..file_edits.len() {
                        if i == j {
                            continue;
                        }
                        let (r2, _) = &file_edits[j];

                        // Check if r2 is strictly contained in r1, or if they are identical (keep first)
                        let starts_after_or_at = r2.start.line > r1.start.line
                            || (r2.start.line == r1.start.line
                                && r2.start.character >= r1.start.character);
                        let ends_before_or_at = r2.end.line < r1.end.line
                            || (r2.end.line == r1.end.line && r2.end.character <= r1.end.character);

                        if starts_after_or_at && ends_before_or_at {
                            if r1.start != r2.start || r1.end != r2.end {
                                // r2 is strictly smaller than r1
                                is_redundant = true;
                                break;
                            } else if j < i {
                                // identical range, already processed or will be kept by j
                                is_redundant = true;
                                break;
                            }
                        }
                    }
                    if !is_redundant {
                        final_edits.push(TextEdit {
                            range: file_edits[i].0,
                            new_text: file_edits[i].1.clone(),
                        });
                    }
                }

                if let Ok(u) = Url::parse(state_uri) {
                    changes.insert(u, final_edits);
                }
            }
        });
    }

    if changes.is_empty() {
        return None;
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}
