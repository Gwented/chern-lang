//! # chrn_lsp
//!
//! Language Server Protocol (LSP) implementation for the Chern language (`.chrn` files).
//!
//! This crate wires together the Chern compiler pipeline and exposes it to editors
//! over the LSP protocol via [`backend::Backend`].  Editors connect through stdio
//! (see `main.rs`) using the [`tower_lsp`] framework.
//!
//! ## Module overview
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`backend`] | `LanguageServer` trait impl – routes every LSP request/notification |
//! | [`state`] | Per-document analysis state (`DocumentState`) and the LRU/dependency cache (`DocumentCache`) |
//! | [`analyser`] | Drives analysis tasks asynchronously: config load → module resolve → diagnostics publish |
//! | [`hover`] | Computes rich Markdown hover content for tokens and semantic entities |
//! | [`document`] | Static documentation tables for keywords, builtin types, and intrinsic functions |
//! | [`references`] | Finds all references to a symbol across all cached documents |
//! | [`rename`] | Produces `WorkspaceEdit` payloads for symbol renames across all cached documents |
//! | [`text`] | UTF-8 ↔ LSP UTF-16 position conversion utilities and incremental text-change application |
//!
//! ## Analysis pipeline
//!
//! Each time a document is opened or changed, the backend spawns an async analysis task:
//!
//! ```text
//! did_open / did_change
//!     └─ analyser::analyze_and_publish_task (async, debounced on change)
//!         ├─ ConfigLoader::load_config       — lex config header, find @def/@end
//!         ├─ analyser::resolve_document_modules — tokenise and resolve imports outside locks
//!         ├─ DocumentCache::insert_or_get    — cache the prepared lexical state
//!         ├─ DocumentState::ensure_analyzed  — parse, name-resolve, type-check
//!         └─ publish_if_current              — send diagnostics only when not superseded
//! ```
//!
//! ## LSP features supported
//!
//! * Diagnostics (config errors, parse errors, namespace errors, type errors)
//! * Hover (keywords, builtin types, intrinsic functions, variables, structs, enums, aliases, modules)
//! * Go-to-definition (cross-module)
//! * Find references (cross-module)
//! * Rename (cross-module)
//! * Completion (keywords, core-library exports, module members, in-scope identifiers)
//! * Semantic tokens (keyword / type / function / variable / operator highlighting)

pub mod analyser;
pub mod backend;
pub mod document;
pub mod hover;
pub mod references;
pub mod rename;
pub mod state;
pub mod text;

#[cfg(test)]
mod tests;
