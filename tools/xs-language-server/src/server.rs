use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tracing::{debug, info};

/// Holds the parsed-but-not-yet-processed text of every document the client
/// has opened. Populated by `did_open` / `did_change`, cleared by `did_close`.
#[derive(Default)]
pub struct DocumentStore {
    inner: HashMap<Url, String>,
}

impl DocumentStore {
    pub fn open(&mut self, uri: Url, text: String) {
        self.inner.insert(uri, text);
    }

    pub fn change(&mut self, uri: &Url, text: String) {
        self.inner.insert(uri.clone(), text);
    }

    pub fn close(&mut self, uri: &Url) {
        self.inner.remove(uri);
    }

    pub fn get(&self, uri: &Url) -> Option<&str> {
        self.inner.get(uri).map(String::as_str)
    }
}

pub struct XsLanguageServer {
    pub client: Client,
    pub documents: Arc<Mutex<DocumentStore>>,
}

impl XsLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(Mutex::new(DocumentStore::default())),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for XsLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!(
            "initialize: client={:?}, root_uri={:?}, capabilities=present",
            params.client_info.map(|i| i.name),
            params.root_uri
        );

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // We re-parse on every change. Full sync is fine for now;
                // incremental sync lands in week 4 if needed.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // Reserved for Day 4-5.
                diagnostic_provider: None,
                // Reserved for Day 6-7.
                completion_provider: None,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("initialized: client confirmed init");
    }

    async fn shutdown(&self) -> Result<()> {
        info!("shutdown requested");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        debug!("did_open: {} ({} bytes)", uri, text.len());
        {
            let mut docs = self.documents.lock().await;
            docs.open(uri.clone(), text);
        }
        self.client
            .log_message(
                MessageType::INFO,
                format!("opened {} (will report diagnostics in Day 4-5)", uri),
            )
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        // FULL sync: only one change with the entire new text.
        let text = params
            .content_changes
            .into_iter()
            .next()
            .map(|c| c.text)
            .unwrap_or_default();
        debug!("did_change: {} ({} bytes)", uri, text.len());
        let mut docs = self.documents.lock().await;
        docs.change(&uri, text);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("did_close: {}", uri);
        let mut docs = self.documents.lock().await;
        docs.close(&uri);
    }
}