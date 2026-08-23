//! # session
//!
//! In-process LSP session for protocol-level tests.
//!
//! [`Session`] drives a real [`LspService`] wrapping a real [`Backend`], so a test can
//! send `initialize`, `textDocument/didOpen`, `textDocument/didChange` and feature
//! requests the way an editor does, then assert on both the JSON-RPC response and the
//! [`Backend`] state that produced it. [`Session::backend`] exposes the document cache,
//! version counters and diagnostic digests that the wire protocol never shows.
//!
//! The transport layer is *not* exercised here. `main.rs`, the `Content-Length` codec and
//! stdout framing are bypassed, so a stray `println!` in the server is invisible to these
//! tests.
//!
//! ## Time
//!
//! Use `#[tokio::test(start_paused = true)]`. Tokio auto-advances to the next timer
//! deadline whenever the runtime goes idle, so the 150 ms `did_change` debounce resolves
//! immediately and deterministically rather than being slept through. The settle timeout
//! below is virtual under a paused clock and costs no wall time.
//!
//! ## Diagnostics
//!
//! `publishDiagnostics` arrives on the client socket, a bounded channel. A session that
//! never drains it stalls the publishing analysis task, so every helper that can advance
//! analysis drains the socket while it waits.
//!
//! `publish_if_current` suppresses a publish whose diagnostic hash is unchanged. An edit
//! that leaves diagnostics identical therefore produces no notification at all: use
//! [`Session::settle`] for those, and [`Session::publish_for`] only where a publish is
//! part of the behaviour under test.
#![allow(dead_code)]

use std::collections::HashMap;
use std::future::poll_fn;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::StreamExt;
use serde_json::{Value, json};
use tower::Service;
use tower_lsp::jsonrpc::{Request, Response};
use tower_lsp::lsp_types::{
    CompletionResponse, Diagnostic, GotoDefinitionResponse, Hover, HoverContents, InitializeParams,
    InitializeResult, Location, MarkedString, Position, Range, SemanticTokensResult, Url,
    WorkspaceEdit,
};
use tower_lsp::{ClientSocket, LspService};

use crate::backend::Backend;

/// How long a drain waits for further server-to-client traffic before concluding the
/// server is idle. Virtual time under `start_paused`, so this is not a wall-clock cost.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);

/// A running language server plus the client half of its loopback channel.
pub struct Session {
    service: LspService<Backend>,
    socket: ClientSocket,
    next_id: i64,
    doc_versions: HashMap<String, i32>,
    diagnostics: HashMap<String, Vec<Diagnostic>>,
}

impl Session {
    /// Starts a server and completes the `initialize` / `initialized` handshake.
    pub async fn new() -> Session {
        let (service, socket) = LspService::new(Backend::new);
        let mut session = Session {
            service,
            socket,
            next_id: 1,
            doc_versions: HashMap::new(),
            diagnostics: HashMap::new(),
        };

        let params = serde_json::to_value(InitializeParams::default())
            .expect("InitializeParams serializes");
        let result = session.request("initialize", params).await;
        let _: InitializeResult =
            serde_json::from_value(result).expect("initialize returns an InitializeResult");
        session.notify("initialized", json!({})).await;

        session
    }

    /// The live server backend, for asserting on state the protocol does not expose.
    pub fn backend(&self) -> &Backend {
        self.service.inner()
    }

    /// The most recently published diagnostics for `uri`, or an empty slice if the
    /// server has never published for it.
    pub fn diagnostics(&self, uri: &Url) -> &[Diagnostic] {
        self.diagnostics
            .get(&uri.to_string())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    // ---------------------------------------------------------------- document sync

    /// Sends `didOpen` and returns the first published diagnostic set.
    ///
    /// The first publish for a URI is never suppressed: `publish_if_current` has no
    /// cached digest to compare against.
    pub async fn open(&mut self, uri: &Url, text: &str) -> Vec<Diagnostic> {
        self.doc_versions.insert(uri.to_string(), 1);
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "chrn",
                    "version": 1,
                    "text": text,
                }
            }),
        )
        .await;

        self.publish_for(uri).await
    }

    /// Sends a whole-document `didChange`, then drains until the server is idle.
    ///
    /// Returns without requiring a publish, because an edit that leaves the diagnostic
    /// set byte-identical is suppressed by design.
    pub async fn change_full(&mut self, uri: &Url, text: &str) {
        self.change(uri, json!([{ "text": text }])).await;
    }

    /// Sends a ranged (incremental) `didChange`, exercising `apply_text_change`.
    pub async fn change_range(&mut self, uri: &Url, range: Range, text: &str) {
        self.change(uri, json!([{ "range": range, "text": text }]))
            .await;
    }

    async fn change(&mut self, uri: &Url, content_changes: Value) {
        let version = self
            .doc_versions
            .entry(uri.to_string())
            .and_modify(|v| *v += 1)
            .or_insert(2);

        let params = json!({
            "textDocument": { "uri": uri, "version": *version },
            "contentChanges": content_changes,
        });
        self.notify("textDocument/didChange", params).await;
        self.settle().await;
    }

    /// Sends `didSave`, optionally replacing the stored text, then drains.
    pub async fn save(&mut self, uri: &Url, text: Option<&str>) {
        let mut params = json!({ "textDocument": { "uri": uri } });
        if let Some(text) = text {
            params["text"] = json!(text);
        }
        self.notify("textDocument/didSave", params).await;
        self.settle().await;
    }

    /// Sends `didClose` and drops the locally tracked version.
    pub async fn close(&mut self, uri: &Url) {
        self.doc_versions.remove(&uri.to_string());
        self.notify(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri } }),
        )
        .await;
        self.settle().await;
    }

    // -------------------------------------------------------------------- features

    pub async fn hover(&mut self, uri: &Url, position: Position) -> Option<Hover> {
        let value = self
            .request("textDocument/hover", text_document_position(uri, position))
            .await;
        serde_json::from_value(value).expect("hover returns Hover | null")
    }

    pub async fn definition(
        &mut self,
        uri: &Url,
        position: Position,
    ) -> Option<GotoDefinitionResponse> {
        let value = self
            .request(
                "textDocument/definition",
                text_document_position(uri, position),
            )
            .await;
        serde_json::from_value(value).expect("definition returns a location payload | null")
    }

    pub async fn references(&mut self, uri: &Url, position: Position) -> Option<Vec<Location>> {
        let mut params = text_document_position(uri, position);
        params["context"] = json!({ "includeDeclaration": true });
        let value = self.request("textDocument/references", params).await;
        serde_json::from_value(value).expect("references returns Location[] | null")
    }

    pub async fn rename(
        &mut self,
        uri: &Url,
        position: Position,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        let mut params = text_document_position(uri, position);
        params["newName"] = json!(new_name);
        let value = self.request("textDocument/rename", params).await;
        serde_json::from_value(value).expect("rename returns WorkspaceEdit | null")
    }

    /// `trigger` mirrors the character an editor reports for a trigger-character
    /// completion (`@`, `#`, `.`, `:`); `None` requests an invoked completion.
    pub async fn completion(
        &mut self,
        uri: &Url,
        position: Position,
        trigger: Option<&str>,
    ) -> Option<CompletionResponse> {
        let mut params = text_document_position(uri, position);
        params["context"] = match trigger {
            Some(character) => json!({ "triggerKind": 2, "triggerCharacter": character }),
            None => json!({ "triggerKind": 1 }),
        };
        let value = self.request("textDocument/completion", params).await;
        serde_json::from_value(value).expect("completion returns a completion payload | null")
    }

    pub async fn semantic_tokens(&mut self, uri: &Url) -> Option<SemanticTokensResult> {
        let value = self
            .request(
                "textDocument/semanticTokens/full",
                json!({ "textDocument": { "uri": uri } }),
            )
            .await;
        serde_json::from_value(value).expect("semanticTokens/full returns a token payload | null")
    }

    // ------------------------------------------------------------- socket draining

    /// Drains server-to-client traffic until `uri` receives a `publishDiagnostics`.
    ///
    /// Panics on timeout. An edit whose diagnostics are unchanged never publishes, so
    /// call [`Session::settle`] instead when a publish is not guaranteed.
    pub async fn publish_for(&mut self, uri: &Url) -> Vec<Diagnostic> {
        let target = uri.to_string();
        loop {
            match tokio::time::timeout(SETTLE_TIMEOUT, self.socket.next()).await {
                Ok(Some(message)) => {
                    if self.record(message).as_deref() == Some(target.as_str()) {
                        return self.diagnostics(uri).to_vec();
                    }
                }
                Ok(None) => panic!("client socket closed while waiting for `{target}`"),
                Err(_) => panic!(
                    "no publishDiagnostics for `{target}`; the server may have suppressed an \
                     unchanged diagnostic set — use `settle` if no publish is expected"
                ),
            }
        }
    }

    /// Drains every pending server-to-client message and returns once the server has
    /// gone quiet, recording any diagnostics seen along the way.
    ///
    /// Under a paused clock this advances through the `did_change` debounce first, so
    /// the debounced analysis has run by the time it returns.
    pub async fn settle(&mut self) {
        loop {
            match tokio::time::timeout(SETTLE_TIMEOUT, self.socket.next()).await {
                Ok(Some(message)) => {
                    self.record(message);
                }
                Ok(None) | Err(_) => return,
            }
        }
    }

    /// Files a server-to-client message, returning the URI when it was a diagnostic
    /// publication.
    fn record(&mut self, message: Request) -> Option<String> {
        let (method, _, params) = message.into_parts();
        if method != "textDocument/publishDiagnostics" {
            return None;
        }

        let params = params.expect("publishDiagnostics carries params");
        let uri = params["uri"]
            .as_str()
            .expect("publishDiagnostics carries a uri")
            .to_string();
        let diagnostics: Vec<Diagnostic> = serde_json::from_value(params["diagnostics"].clone())
            .expect("publishDiagnostics carries a diagnostic array");

        self.diagnostics.insert(uri.clone(), diagnostics);
        Some(uri)
    }

    // ---------------------------------------------------------------- jsonrpc plumbing

    /// Sends a request and unwraps a successful result.
    async fn request(&mut self, method: &'static str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let request = Request::build(method).params(params).id(id).finish();
        let response = self
            .dispatch(request)
            .await
            .unwrap_or_else(|| panic!("request `{method}` produced no response"));

        match response.into_parts().1 {
            Ok(value) => value,
            Err(err) => panic!("request `{method}` failed: {err:?}"),
        }
    }

    /// Sends a notification and asserts the server answered with nothing.
    async fn notify(&mut self, method: &'static str, params: Value) {
        let request = Request::build(method).params(params).finish();
        if let Some(response) = self.dispatch(request).await {
            panic!("notification `{method}` produced a response: {response:?}");
        }
    }

    async fn dispatch(&mut self, request: Request) -> Option<Response> {
        poll_fn(|cx| self.service.poll_ready(cx))
            .await
            .expect("server has exited");
        self.service
            .call(request)
            .await
            .expect("server has exited")
    }
}

fn text_document_position(uri: &Url, position: Position) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": position,
    })
}

/// Locates the `nth` (0-based) occurrence of `needle` and returns its start as an LSP
/// position.
///
/// Columns are counted in UTF-16 code units, independently of `crate::text`, so a test
/// position and the conversion under test cannot share a bug.
pub fn position_of(text: &str, needle: &str, nth: usize) -> Position {
    let offset = text
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("`{needle}` does not occur {} time(s) in the document", nth + 1))
        .0;

    let line = text[..offset].matches('\n').count() as u32;
    let line_start = text[..offset]
        .rfind('\n')
        .map(|newline| newline + 1)
        .unwrap_or(0);
    let character = text[line_start..offset].encode_utf16().count() as u32;

    Position { line, character }
}

/// Flattens hover contents into plain text for substring assertions.
pub fn hover_text(hover: &Hover) -> String {
    fn marked(marked: &MarkedString) -> String {
        match marked {
            MarkedString::String(value) => value.clone(),
            MarkedString::LanguageString(value) => value.value.clone(),
        }
    }

    match &hover.contents {
        HoverContents::Scalar(value) => marked(value),
        HoverContents::Array(values) => values.iter().map(marked).collect::<Vec<_>>().join("\n"),
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

/// A throwaway directory of real `.chrn` files.
///
/// Import resolution reads dependencies from disk, so any multi-module test needs its
/// modules to actually exist. The directory is removed on drop.
pub struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    pub fn new(name: &str) -> TempWorkspace {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "chrn_lsp_session_{name}_{}_{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temp workspace is creatable");

        TempWorkspace { root }
    }

    /// Writes `text` to `relative_path` and returns the file's URI.
    pub fn write(&self, relative_path: &str, text: &str) -> Url {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent directory is creatable");
        }
        std::fs::write(&path, text).expect("temp file is writable");

        Url::from_file_path(&path).expect("temp path is absolute")
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
