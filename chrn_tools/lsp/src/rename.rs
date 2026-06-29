//! # rename
//!
//! Produces [`WorkspaceEdit`] payloads for symbol renames across all cached documents.
//!
//! The single public entry point is [`compute_rename`], called from
//! [`crate::backend::Backend::rename`].
//!
//! ## Rename strategy
//!
//! * **Module renames** are not supported because they imply a file-system rename;
//!   `None` is returned immediately.
//! * **Local bindings** are renamed within the current file only, keying on the
//!   declaration span and owning symbol ID (same logic as `references`).
//! * **All other symbols** are renamed cross-module by searching every cached
//!   document for the matching `(definition_path, definition_span, owning_symbol_id)`.
//!
//! Overlapping/redundant `TextEdit` ranges within each file are removed with
//! [`crate::text::deduplicate_range_indices`] before the `WorkspaceEdit` is assembled.

use crate::state::{DocumentCache, DocumentState, SemanticEntity};
use crate::text::{offset_to_position, position_to_offset};
use chrn_utils::id_types::SymbolId;
use chrn_utils::source_map::source_span::SourceSpan;
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};

/// Collects all occurrences of a local binding in the current file.
fn collect_local_edits(
    state: &DocumentState,
    def_span: &SourceSpan,
    def_owner_sym_id: Option<SymbolId>,
    new_name: &str,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for (span, ent) in &state.symbol_map {
        if let SemanticEntity::Local {
            decl_span,
            owner_sym_id,
            ..
        } = ent
        {
            if *decl_span == *def_span && *owner_sym_id == def_owner_sym_id {
                edits.push(TextEdit {
                    range: Range {
                        start: offset_to_position(&state.text, span.start as usize),
                        end: offset_to_position(&state.text, (span.end + 1) as usize),
                    },
                    new_text: new_name.to_string(),
                });
            }
        }
    }
    edits
}

/// Converts raw matching-entity tuples into a per-URI map of deduplicated text edits.
fn matching_entities_to_edits(
    entities: Vec<(String, Arc<String>, u32, u32)>,
    new_name: &str,
) -> HashMap<Url, Vec<TextEdit>> {
    let mut by_uri: HashMap<String, Vec<Range>> = HashMap::new();
    let mut text_map: HashMap<String, Arc<String>> = HashMap::new();
    for (state_uri, text, start, end) in &entities {
        let range = Range {
            start: offset_to_position(text, *start as usize),
            end: offset_to_position(text, (*end + 1) as usize),
        };
        by_uri.entry(state_uri.clone()).or_default().push(range);
        text_map.entry(state_uri.clone()).or_insert_with(|| Arc::clone(text));
    }

    let mut changes = HashMap::new();
    for (state_uri, ranges) in by_uri {
        if let Ok(uri) = Url::parse(&state_uri) {
            let mut edits = Vec::new();
            for &i in &crate::text::deduplicate_range_indices(&ranges) {
                edits.push(TextEdit {
                    range: ranges[i],
                    new_text: new_name.to_string(),
                });
            }
            changes.insert(uri, edits);
        }
    }
    changes
}

/// Computes the workspace edits required to rename the symbol at `position`.
///
/// # Parameters
/// * `uri`       — URI of the file containing the cursor.
/// * `position`  — Cursor position in LSP UTF-16 coordinates.
/// * `new_name`  — The replacement identifier string.
/// * `doc_cache` — Cache of all analysed documents to search for usages.
///
/// # Returns
/// * `Some(WorkspaceEdit)` mapping each affected URI to a list of [`TextEdit`] values.
/// * `None` when the cursor is in a comment, no entity is found, or the entity is
///   a module (module renames are not supported).
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

    let changes = if is_local {
            let edits = collect_local_edits(&state, &def_span, def_owner_sym_id, &new_name);
        if edits.is_empty() {
            HashMap::new()
        } else {
            [(uri.clone(), edits)].into_iter().collect()
        }
    } else {
        let entities = DocumentState::find_matching_entities(doc_cache, &def_path, def_span, def_owner_sym_id);
        matching_entities_to_edits(entities, &new_name)
    };

    if changes.is_empty() {
        return None;
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}
