use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tracing::{debug, info};

use crate::{completion, diagnostics, engine_api, parser, symbols, word};

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
    /// Per-file symbol tables, rebuilt on every `did_open` / `did_change`.
    pub symbol_tables: Arc<Mutex<HashMap<Url, symbols::SymbolTable>>>,
    pub engine: engine_api::SharedEngineApi,
}

impl XsLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(Mutex::new(DocumentStore::default())),
            symbol_tables: Arc::new(Mutex::new(HashMap::new())),
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
                // Week 3: document symbol outline built from the per-file
                // symbol table.
                document_symbol_provider: Some(OneOf::Left(true)),
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
        self.rebuild_symbol_table(&uri, &text).await;
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
        self.rebuild_symbol_table(&uri, &text).await;
        self.publish_diagnostics(&uri, &text, version).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("did_close: {}", uri);
        let mut docs = self.documents.lock().await;
        docs.close(&uri);
        let mut tables = self.symbol_tables.lock().await;
        tables.remove(&uri);
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

        // Engine API first (richer info: signature + help text + return type).
        // Workspace symbol table as fallback (signature only, but anchored
        // to a real source location).
        let markdown = if let Some(s) = self.engine.find_syscall(&ident) {
            format_hover_syscall(s)
        } else if let Some(c) = self.engine.find_aiplan(&ident) {
            format_hover_aiplan(c)
        } else {
            let tables = self.symbol_tables.lock().await;
            match tables.get(uri).and_then(|t| t.find(&ident)) {
                Some(sym) => format_hover_symbol(sym),
                None => {
                    debug!("hover: identifier `{ident}` not in engine API or workspace");
                    return Ok(None);
                }
            }
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

        // Workspace symbol table first — has a real file:line location.
        // Engine API as fallback — returns a virtual xs-stub:// URI.
        let location = {
            let tables = self.symbol_tables.lock().await;
            tables
                .get(uri)
                .and_then(|t| t.find(&ident))
                .map(|sym| Location {
                    uri: uri.clone(),
                    range: sym.selection_range,
                })
        };

        let location = match location {
            Some(loc) => {
                debug!("definition: `{ident}` -> workspace at {:?}", loc.range.start);
                loc
            }
            None => {
                if self.engine.find_syscall(&ident).is_none()
                    && self.engine.find_aiplan(&ident).is_none()
                {
                    debug!("definition: identifier `{ident}` not in engine API or workspace");
                    return Ok(None);
                }
                // Virtual URI — these stubs don't have a real source position.
                let stub_uri = format!("xs-stub://engine/{ident}");
                let loc = Location {
                    uri: Url::parse(&stub_uri)
                        .map_err(|_e| tower_lsp::jsonrpc::Error::internal_error())?,
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                };
                debug!("definition: `{ident}` -> {stub_uri}");
                loc
            }
        };
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let tables = self.symbol_tables.lock().await;
        let Some(table) = tables.get(uri) else {
            return Ok(None);
        };
        let items: Vec<DocumentSymbol> = table.symbols.iter().map(symbol_to_lsp).collect();
        debug!(
            "document_symbol: {} symbol(s) for {}",
            items.len(),
            uri
        );
        Ok(Some(DocumentSymbolResponse::Nested(items)))
    }
}

impl XsLanguageServer {
    /// Re-parse `text` and rebuild the per-file symbol table for `uri`.
    /// Called on `did_open` / `did_change`. On parse failure, we keep the
    /// stale table rather than clearing it — the old symbols still help
    /// with hover/definition until the user fixes the parse error.
    async fn rebuild_symbol_table(&self, uri: &Url, text: &str) {
        if let Some(tree) = parser::parse(text) {
            let table = symbols::build_symbol_table(&tree, text);
            debug!(
                "rebuild_symbol_table: {} -> {} symbol(s)",
                uri,
                table.symbols.len()
            );
            let mut tables = self.symbol_tables.lock().await;
            tables.insert(uri.clone(), table);
        }
    }

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

/// Format a workspace symbol as a Markdown hover card. Workspace symbols
/// don't carry help text, but they have a real source location.
fn format_hover_symbol(s: &symbols::Symbol) -> String {
    let kind_label = s.kind.label();
    let mut md = format!("*{}* — `{}`\n", kind_label, s.detail);
    if !s.params.is_empty() {
        md.push_str("\n**Parameters:**\n");
        for p in &s.params {
            md.push_str(&format!("- `{} {}`\n", p.ty, p.name));
        }
    }
    md
}

/// Convert a workspace symbol to the LSP `DocumentSymbol` shape.
fn symbol_to_lsp(s: &symbols::Symbol) -> DocumentSymbol {
    let kind = match s.kind {
        symbols::SymbolKind::Rule => SymbolKind::FUNCTION, // XS has no RULE kind in LSP
        symbols::SymbolKind::Function => SymbolKind::FUNCTION,
        symbols::SymbolKind::Variable => SymbolKind::VARIABLE,
        symbols::SymbolKind::Constant => SymbolKind::CONSTANT,
    };
    DocumentSymbol {
        name: s.name.clone(),
        detail: Some(s.detail.clone()),
        kind,
        tags: None,
        deprecated: None,
        range: s.full_range,
        selection_range: s.selection_range,
        children: None,
    }
}