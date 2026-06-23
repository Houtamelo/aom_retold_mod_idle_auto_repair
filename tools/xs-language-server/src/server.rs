use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tracing::{debug, info};

use crate::{completion, diagnostics, engine_api, parser, word};

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
    pub engine: engine_api::SharedEngineApi,
}

impl XsLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(Mutex::new(DocumentStore::default())),
            engine: Arc::new(engine_api::EngineApi::load_default().unwrap_or_default()),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for XsLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!(
            "initialize: client={:?}, root_uri={:?}, capabilities=present ({} syscalls, {} aiplans loaded)",
            params.client_info.map(|i| i.name),
            params.root_uri,
            self.engine.syscalls.len(),
            self.engine.aiplans.len(),
        );

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // We re-parse on every change. Full sync is fine for now;
                // incremental sync lands in week 4 if needed.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    // We don't need resolve_provider; detail is in the item.
                    resolve_provider: Some(false),
                    // Trigger on every identifier character and dot.
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "_".to_string(),
                    ]),
                    ..Default::default()
                }),
                // Week 2: hover returns the syscall signature + help as
                // Markdown. No options (no work-done progress, no dynamic
                // registration) — keep it simple until clients ask for more.
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Week 2: go-to-definition resolves engine-API symbols to
                // a virtual `xs-stub://engine/<name>` URI (range 0:0-0:0
                // since stubs have no real source position). No
                // linkSupport yet — that comes when we have real workspace
                // symbols.
                definition_provider: Some(OneOf::Left(true)),
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
        let version = params.text_document.version;
        debug!("did_open: {} ({} bytes, v{})", uri, text.len(), version);
        {
            let mut docs = self.documents.lock().await;
            docs.open(uri.clone(), text.clone());
        }
        self.publish_diagnostics(&uri, &text, version).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        // FULL sync: only one change with the entire new text.
        let text = params
            .content_changes
            .into_iter()
            .next()
            .map(|c| c.text)
            .unwrap_or_default();
        debug!("did_change: {} ({} bytes, v{})", uri, text.len(), version);
        {
            let mut docs = self.documents.lock().await;
            docs.change(&uri, text.clone());
        }
        self.publish_diagnostics(&uri, &text, version).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("did_close: {}", uri);
        let mut docs = self.documents.lock().await;
        docs.close(&uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };
        let items = completion::complete(&self.engine, &text, &params);
        debug!("completion: {} item(s) at {:?}", items.len(), params.text_document_position.position);
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
            debug!("hover: no identifier at {:?}", pos);
            return Ok(None);
        };

        let markdown = if let Some(s) = self.engine.find_syscall(&ident) {
            format_hover_syscall(s)
        } else if let Some(c) = self.engine.find_aiplan(&ident) {
            format_hover_aiplan(c)
        } else {
            debug!("hover: identifier `{ident}` not in engine API");
            return Ok(None);
        };

        debug!("hover: `{ident}` -> {} chars of markdown", markdown.len());
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
            debug!("definition: no identifier at {:?}", pos);
            return Ok(None);
        };

        if self.engine.find_syscall(&ident).is_none()
            && self.engine.find_aiplan(&ident).is_none()
        {
            debug!("definition: identifier `{ident}` not in engine API");
            return Ok(None);
        }

        // Virtual URI — these stubs don't have a real source position, so
        // we use 0:0-0:0. Clients will show a synthetic file for this URI.
        let stub_uri = format!("xs-stub://engine/{ident}");
        let location = Location {
            uri: Url::parse(&stub_uri)
                .map_err(|e| tower_lsp::jsonrpc::Error::internal_error())?,
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        };
        debug!("definition: `{ident}` -> {stub_uri}");
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }
}

impl XsLanguageServer {
    /// Parse `text` as XS and publish any parse errors as LSP diagnostics.
    /// An empty `Vec` (clean parse) is also published so clients clear
    /// stale diagnostics for this URI.
    async fn publish_diagnostics(&self, uri: &Url, text: &str, version: i32) {
        let diagnostics = match parser::parse(text) {
            Some(tree) => diagnostics::collect_diagnostics(&tree, text),
            None => vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("xs-language-server".to_string()),
                message: "internal error: failed to install XS language".to_string(),
                related_information: None,
                tags: None,
                data: None,
            }],
        };
        debug!(
            "publish_diagnostics: {} ({} issue(s))",
            uri,
            diagnostics.len()
        );
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, Some(version))
            .await;
    }
}

/// Format a syscall as a Markdown hover card.
fn format_hover_syscall(s: &engine_api::Syscall) -> String {
    let params = s
        .params
        .iter()
        .map(|p| format!("{} {}", p.ty, p.name))
        .collect::<Vec<_>>()
        .join(", ");
    let signature = format!("{} {}({})", s.return_type, s.name, params);
    let mut md = format!("```xs\n{}\n```\n", signature);
    if !s.help.is_empty() {
        // Indent multi-paragraph help so it renders as a single block under
        // the code fence.
        md.push_str(&s.help);
        md.push('\n');
    }
    md
}

/// Format an AI-plan constant as a Markdown hover card.
fn format_hover_aiplan(c: &engine_api::AiplanConstant) -> String {
    let signature = format!("const {} {} = {}", c.variable_type, c.name, c.variable_value);
    let md = format!("```xs\n{}\n```\n", signature);
    md
}