//! # references
//!
//! Computes "find all references" results for a symbol under the cursor.
//!
//! The single public entry point is [`compute_references`], called from
//! [`crate::backend::Backend::references`].
//!
//! ## Search strategy
//!
//! * **Local bindings** (alias type parameters etc.) are searched only within the
//!   current file, keying on the declaration span and owning symbol ID.
//! * **All other symbols** (types, variables, module members) are searched across
//!   every document currently held in the [`DocumentCache`](crate::state::DocumentCache),
//!   matching by `(definition_path, definition_span, owning_symbol_id)`.
//!
//! After collecting all candidate [`Location`] values, overlapping/redundant ranges
//! within each file are removed with [`crate::text::deduplicate_range_indices`].

use crate::state::{DocumentCache, DocumentState, SemanticEntity};
use crate::text::{offset_to_position, position_to_offset};
use chrn_utils::id_types::SymbolId;
use chrn_utils::source_map::source_span::SourceSpan;
use std::sync::Arc;
use tower_lsp::lsp_types::{Location, Position, Range, Url};

/// Finds all symbol-map entries in the current file that share the same
/// `(decl_span, owner_sym_id)` key — used for local bindings.
fn collect_local_occurrences(
    state: &DocumentState,
    def_span: &SourceSpan,
    def_owner_sym_id: Option<SymbolId>,
    uri: &Url,
) -> Vec<Location> {
    let mut results = Vec::new();
    for (span, ent) in &state.symbol_map {
        if let SemanticEntity::Local {
            decl_span,
            owner_sym_id,
            ..
        } = ent
            && *decl_span == *def_span && *owner_sym_id == def_owner_sym_id
        {
                results.push(Location {
                    uri: uri.clone(),
                    range: Range {
                        start: offset_to_position(&state.text, span.start as usize),
                        end: offset_to_position(&state.text, span.end as usize),
                    },
                });
        }
    }
    results
}

/// Converts raw matching-entity tuples into deduplicated [`Location`] values.
fn matching_entities_to_locations(
    entities: Vec<(String, Arc<String>, u32, u32)>,
) -> Vec<Location> {
    let mut results = Vec::new();
    // Group by URI to deduplicate per file
    let mut by_uri: std::collections::HashMap<String, Vec<Range>> = std::collections::HashMap::new();
    for (state_uri, text, start, end) in entities {
        let range = Range {
            start: offset_to_position(&text, start as usize),
            end: offset_to_position(&text, end as usize),
        };
        by_uri.entry(state_uri).or_default().push(range);
    }
    for (state_uri, ranges) in by_uri {
        if let Ok(uri) = Url::parse(&state_uri) {
            for &i in &crate::text::deduplicate_range_indices(&ranges) {
                results.push(Location {
                    uri: uri.clone(),
                    range: ranges[i],
                });
            }
        }
    }
    results
}

/// Computes the list of locations where the symbol at `position` is referenced.
///
/// # Parameters
/// * `uri`       — URI of the file containing the cursor.
/// * `position`  — Cursor position in LSP UTF-16 coordinates.
/// * `doc_cache` — Cache of all analysed documents to search.
///
/// # Returns
/// * `Some(Vec<Location>)` with one entry per reference occurrence.
/// * `None` when the cursor is in a comment, no entity is found, or the entity
///   is a module (module references are not yet tracked).
pub fn compute_references(
    uri: &Url,
    position: Position,
    doc_cache: &DocumentCache,
) -> Option<Vec<Location>> {
    let uri_str = uri.to_string();
    let state_arc = doc_cache.get(&uri_str)?;
    let state = state_arc.read();

    let byte_offset = position_to_offset(&state.text, position);

    if state.offset_in_comment(byte_offset) {
        return None;
    }

    let entity = state.get_entity_at_offset(byte_offset)?;

    // We don't support references for modules yet
    if matches!(entity, SemanticEntity::Module(_)) {
        return None;
    }

    let (def_path, def_span, def_owner_sym_id) = state.get_definition_location(entity)?;
    let is_local = matches!(entity, SemanticEntity::Local { .. });

    let locations = if is_local {
        collect_local_occurrences(&state, &def_span, def_owner_sym_id, uri)
    } else {
        let entities = DocumentState::find_matching_entities(doc_cache, &def_path, def_span, def_owner_sym_id);
        matching_entities_to_locations(entities)
    };

    if locations.is_empty() {
        return None;
    }

    Some(locations)
}
