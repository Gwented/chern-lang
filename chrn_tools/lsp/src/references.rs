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

use crate::state::{DocumentCache, DocumentState, EntityOccurrence, SemanticEntity};
use crate::text::{LineIndex, position_to_offset};
use chrn_utils::id_types::SymbolId;
use chrn_utils::source_map::source_span::SourceSpan;
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
    let lines = LineIndex::new(&state.text);
    for (span, ent) in &state.symbol_map {
        if let SemanticEntity::Local {
            decl_span,
            owner_sym_id,
            ..
        } = ent
            && *decl_span == *def_span
            && *owner_sym_id == def_owner_sym_id
        {
            // `span` is relative to the region's `src_bytes`; shift to absolute
            // coordinates by adding `script_start` before converting to an LSP
            // `Position`.
            let abs_start = crate::text::rel_to_abs_offset(span.start, state.script_start) as usize;
            let abs_end = crate::text::rel_to_abs_offset(span.end, state.script_start) as usize;
            results.push(Location {
                uri: uri.clone(),
                range: Range {
                    start: lines.position(abs_start),
                    end: lines.position(abs_end),
                },
            });
        }
    }
    results
}

/// Converts raw matching-entity tuples into deduplicated [`Location`] values.
fn matching_entities_to_locations(entities: Vec<EntityOccurrence>) -> Vec<Location> {
    let mut results = Vec::new();
    for (state_uri, ranges) in crate::text::occurrences_to_ranges(entities) {
        if let Ok(uri) = Url::parse(&state_uri) {
            results.extend(ranges.into_iter().map(|range| Location {
                uri: uri.clone(),
                range,
            }));
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

    // The local search reads `state`; the cross-module search re-reads every
    // cached document, *including this one*.  `parking_lot`'s `RwLock` is not
    // reentrant, so holding this guard across `find_matching_entities` deadlocks
    // as soon as a writer (an analysis task) is queued between the two reads.
    // Resolve the definition key under the guard, then drop it before searching.
    let (def_path, def_span, def_owner_sym_id, is_local, local_locations) = {
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

        let (def_path, def_span, def_owner_sym_id) = state.definition_site(entity)?;
        let def_path = def_path.to_path_buf();
        let is_local = matches!(entity, SemanticEntity::Local { .. });
        let local_locations = if is_local {
            collect_local_occurrences(&state, &def_span, def_owner_sym_id, uri)
        } else {
            Vec::new()
        };
        (
            def_path,
            def_span,
            def_owner_sym_id,
            is_local,
            local_locations,
        )
    };

    let locations = if is_local {
        local_locations
    } else {
        let entities =
            DocumentState::find_matching_entities(doc_cache, &def_path, def_span, def_owner_sym_id);
        matching_entities_to_locations(entities)
    };

    if locations.is_empty() {
        return None;
    }

    Some(locations)
}
