use crate::state::{DocumentCache, SemanticEntity};
use crate::text::{offset_to_position, position_to_offset};
use tower_lsp::lsp_types::{Location, Position, Range, Url};

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
                            start: offset_to_position(&state.text, span.start),
                            end: offset_to_position(&state.text, span.end + 1),
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
                                start: offset_to_position(&other_state.text, span.start),
                                end: offset_to_position(&other_state.text, span.end + 1),
                            },
                        });
                    }
                }
            }

            // Deduplicate: prefer smaller ranges
            let mut deduplicated = Vec::new();
            for i in 0..file_locations.len() {
                let loc1 = &file_locations[i];
                let mut is_redundant = false;
                for j in 0..file_locations.len() {
                    if i == j {
                        continue;
                    }
                    let loc2 = &file_locations[j];

                    let r1 = &loc1.range;
                    let r2 = &loc2.range;

                    let starts_after_or_at = r2.start.line > r1.start.line
                        || (r2.start.line == r1.start.line
                            && r2.start.character >= r1.start.character);
                    let ends_before_or_at = r2.end.line < r1.end.line
                        || (r2.end.line == r1.end.line && r2.end.character <= r1.end.character);

                    if starts_after_or_at && ends_before_or_at {
                        if r1.start != r2.start || r1.end != r2.end {
                            is_redundant = true;
                            break;
                        } else if j < i {
                            is_redundant = true;
                            break;
                        }
                    }
                }
                if !is_redundant {
                    deduplicated.push(loc1.clone());
                }
            }
            locations.extend(deduplicated);
        });
    }

    if locations.is_empty() {
        return None;
    }

    Some(locations)
}
