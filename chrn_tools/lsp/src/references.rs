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

use crate::state::{DocumentCache, SemanticEntity};
use crate::text::{offset_to_position, position_to_offset};
use tower_lsp::lsp_types::{Location, Position, Range, Url};

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

    let mut locations = Vec::new();

    if is_local {
        // Optimization: for locals, we only need to look at the current file
        for (span, ent) in &state.symbol_map {
            if let SemanticEntity::Local {
                decl_span,
                owner_sym_id,
                ..
            } = ent
            {
                if *decl_span == def_span && *owner_sym_id == def_owner_sym_id {
                    locations.push(Location {
                        uri: uri.clone(),
                        range: Range {
                            start: offset_to_position(&state.text, span.start as usize),
                            end: offset_to_position(&state.text, (span.end + 1) as usize),
                        },
                    });
                }
            }
        }
    } else {
        // Cross-module references: search all cached documents
        doc_cache.for_each_state(|state_uri, other_state_arc| {
            let other_state = other_state_arc.read();
            let other_uri = match Url::parse(state_uri) {
                Ok(u) => u,
                Err(_) => return,
            };

            let mut file_locations = Vec::new();
            for (span, ent) in &other_state.symbol_map {
                if let Some((other_def_path, other_def_span, other_def_owner_sym_id)) =
                    other_state.get_definition_location(ent)
                {
                    if other_def_path == def_path
                        && other_def_span == def_span
                        && other_def_owner_sym_id == def_owner_sym_id
                    {
                        file_locations.push(Location {
                            uri: other_uri.clone(),
                            range: Range {
                                start: offset_to_position(&other_state.text, span.start as usize),
                                end: offset_to_position(&other_state.text, (span.end + 1) as usize),
                            },
                        });
                    }
                }
            }

            let ranges: Vec<Range> = file_locations.iter().map(|l| l.range).collect();
            for &i in &crate::text::deduplicate_range_indices(&ranges) {
                locations.push(file_locations[i].clone());
            }
        });
    }

    if locations.is_empty() {
        return None;
    }

    Some(locations)
}
