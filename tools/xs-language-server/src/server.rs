use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tracing::{debug, info, warn};

use crate::{completion, diagnostics, engine_api, merged_view, parser, references, semantic, symbols, word, workspace};

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

    pub fn uris(&self) -> Vec<Url> {
        self.inner.keys().cloned().collect()
    }
}

pub struct XsLanguageServer {
    pub client: Client,
    pub documents: Arc<Mutex<DocumentStore>>,
    /// Per-file symbol tables, rebuilt on every `did_open` / `did_change`.
    pub symbol_tables: Arc<Mutex<HashMap<Url, symbols::SymbolTable>>>,
    pub engine: engine_api::SharedEngineApi,
    pub game_path: PathBuf,
    /// Registered workspace folders, each representing one mod.
    pub workspace: Arc<Mutex<workspace::Workspace>>,
    /// Most recently touched document, used to scope `workspace/symbol`.
    pub last_active_uri: Arc<Mutex<Option<Url>>>,
    /// Capabilities advertised by the client on initialize.
    pub client_capabilities: Arc<Mutex<ClientCapabilities>>,
}

impl XsLanguageServer {
    pub fn new(
        client: Client,
        engine: engine_api::SharedEngineApi,
        game_path: PathBuf,
    ) -> Self {
        let workspace = workspace::Workspace::new(game_path.clone());
        Self {
            client,
            documents: Arc::new(Mutex::new(DocumentStore::default())),
            symbol_tables: Arc::new(Mutex::new(HashMap::new())),
            engine,
            game_path,
            workspace: Arc::new(Mutex::new(workspace)),
            last_active_uri: Arc::new(Mutex::new(None)),
            client_capabilities: Arc::new(Mutex::new(ClientCapabilities::default())),
        }
    }

    /// Re-evaluate ownership for every open document after the workspace
    /// folder set changes. Newly-unowned files receive the engine-API-only
    /// warning; newly-owned files are silently re-analysed on the next edit.
    async fn revalidate_open_document_ownership(&self) {
        let uris = {
            let docs = self.documents.lock().await;
            docs.uris()
        };
        for uri in uris {
            let owned = {
                let ws = self.workspace.lock().await;
                ws.lookup_mod(&uri).is_some()
            };
            if !owned {
                warn_unowned_file(&self.client, &uri).await;
            }
        }
    }

    /// Build a merged include-paste view for `uri` from its current buffer.
    ///
    /// The view is built on demand; T9 adds a content-keyed cache for this.
    async fn build_merged_view_for_uri(
        &self,
        uri: &Url,
        text: &str,
    ) -> Option<merged_view::MergedView> {
        let current_file = uri.to_file_path().ok()?;
        let (ws_clone, project) = {
            let ws = self.workspace.lock().await;
            let entry = ws.lookup_mod(uri);
            let project = match entry {
                Some(e) => ws.build_virtual_project(e),
                None => workspace::VirtualProject::default(),
            };
            (ws.clone(), project)
        };
        let own_table = {
            let tables = self.symbol_tables.lock().await;
            tables.get(uri).cloned().unwrap_or_default()
        };
        let cache_dir = crate::cache::state_cache_dir();
        match merged_view::MergedView::build(
            &current_file,
            text,
            &own_table,
            &ws_clone,
            &project,
            &cache_dir,
        ) {
            Ok(mv) => Some(mv),
            Err(e) => {
                warn!("failed to build merged view for {}: {}", uri, e);
                None
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for XsLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Seed workspace folders from the initialize request (added by clients
        // such as IntelliJ at startup).
        if let Some(folders) = params.workspace_folders {
            let mut ws = self.workspace.lock().await;
            for folder in folders {
                let uri = folder.uri.clone();
                if let Err(e) = ws.register_mod(uri.clone()) {
                    warn!("failed to register workspace folder {}: {}", uri, e);
                }
            }
        }

        {
            let mut caps = self.client_capabilities.lock().await;
            *caps = params.capabilities;
        }

        info!(
            "initialize: client={:?}, root_uri={:?}, workspace_folders={}\
             , capabilities=present ({} syscalls, {} aiplans loaded)",
            params.client_info.map(|i| i.name),
            params.root_uri,
            self.workspace.lock().await.mods().len(),
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
                // Week 4: find all uses of an identifier in the current
                // file (workspace-wide lands in week 5+).
                references_provider: Some(OneOf::Left(true)),
                // Week 4: rename an identifier across the current file,
                // with prepare_rename enabled so the client can ask first
                // whether the cursor is on a renameable identifier.
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                // Phase 2: workspace symbols are scoped to the active mod.
                workspace_symbol_provider: Some(OneOf::Left(true)),
                // Phase 2: workspace folders and dynamic file watching.
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("xs-language-server".to_string()),
                        inter_file_dependencies: true,
                        workspace_diagnostics: false,
                        work_done_progress_options: Default::default(),
                    },
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("initialized: client confirmed init");

        let (dynamic_supported, game_path) = {
            let caps = self.client_capabilities.lock().await;
            let supported = caps
                .workspace
                .as_ref()
                .and_then(|w| w.did_change_watched_files.as_ref())
                .and_then(|d| d.dynamic_registration)
                .unwrap_or(false);
            (supported, self.game_path.clone())
        };

        if dynamic_supported {
            let pattern = format!(
                "{}/game/**/*.xs",
                game_path.to_string_lossy().replace('\\', "/")
            );
            let client = self.client.clone();
            tokio::spawn(async move {
                let options = DidChangeWatchedFilesRegistrationOptions {
                    watchers: vec![FileSystemWatcher {
                        glob_pattern: GlobPattern::String(pattern),
                        kind: None,
                    }],
                };
                let registration = Registration {
                    id: "xs-game-folder-watcher".to_string(),
                    method: "workspace/didChangeWatchedFiles".to_string(),
                    register_options: Some(
                        serde_json::to_value(options)
                            .unwrap_or(serde_json::Value::Null),
                    ),
                };
                if let Err(e) = client.register_capability(vec![registration]).await {
                    warn!("failed to register didChangeWatchedFiles watcher: {}", e);
                } else {
                    info!("registered didChangeWatchedFiles watcher for game folder");
                }
            });
        } else {
            info!("client does not support dynamic watched-file registration");
        }
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

        // Track the active document for workspace-symbol scoping.
        {
            let mut active = self.last_active_uri.lock().await;
            *active = Some(uri.clone());
        }

        // Determine whether this file belongs to a registered mod.
        let owning_mod = {
            let ws = self.workspace.lock().await;
            ws.lookup_mod(&uri).cloned()
        };

        if owning_mod.is_none() {
            warn_unowned_file(&self.client, &uri).await;
        } else {
            debug!("did_open: {} owned by {:?}", uri, owning_mod.as_ref().map(|m| &m.mod_uri));
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
        {
            let mut active = self.last_active_uri.lock().await;
            *active = Some(uri.clone());
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

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        let added = params.event.added.len();
        let removed = params.event.removed.len();
        info!(
            "did_change_workspace_folders: +{} -{}",
            added, removed
        );

        {
            let mut ws = self.workspace.lock().await;
            for folder in params.event.added {
                if let Err(e) = ws.register_mod(folder.uri) {
                    warn!("failed to add workspace folder: {}", e);
                }
            }
            for folder in params.event.removed {
                ws.unregister_mod(&folder.uri);
            }
        }

        self.revalidate_open_document_ownership().await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        info!("did_change_watched_files: {} change(s)", params.changes.len());
        for change in &params.changes {
            debug!("watched file change: {:?} {:?}", change.typ, change.uri);
            if let Ok(path) = change.uri.to_file_path() {
                let rel = {
                    let ws = self.workspace.lock().await;
                    path.strip_prefix(ws.game_path().join("game"))
                        .ok()
                        .map(|p| p.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"))
                };
                if let Some(rel) = rel {
                    let cache_dir = crate::cache::state_cache_dir();
                    if let Err(e) = crate::cache::invalidate_parse_cache(&rel, &cache_dir) {
                        warn!("failed to invalidate parse cache for {}: {}", rel, e);
                    }
                }
            }
        }

        // Re-diagnose open mod files whose dependencies may have changed.
        // In Phase 3 this will be narrowed to true include-graph dependents.
        let uris = {
            let docs = self.documents.lock().await;
            docs.uris()
        };
        for uri in uris {
            let is_owned = {
                let ws = self.workspace.lock().await;
                ws.lookup_mod(&uri).is_some()
            };
            if !is_owned {
                continue;
            }
            let (text, version) = {
                let docs = self.documents.lock().await;
                docs.get(&uri)
                    .map(|t| (t.to_string(), 0i32))
                    .unwrap_or_default()
            };
            if !text.is_empty() {
                self.rebuild_symbol_table(&uri, &text).await;
                self.publish_diagnostics(&uri, &text, version).await;
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };
        let merged = self.build_merged_view_for_uri(uri, &text).await;
        let items = completion::complete(
            &self.engine,
            merged.as_ref(),
            &text,
            &params,
        );
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

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();

        let active_uri = {
            let active = self.last_active_uri.lock().await;
            match active.clone() {
                Some(uri) => uri,
                None => return Ok(None),
            }
        };

        // Find the owning mod to scope the search.
        let project = {
            let ws = self.workspace.lock().await;
            let Some(entry) = ws.lookup_mod(&active_uri) else {
                return Ok(None);
            };
            ws.build_virtual_project(entry)
        };

        let ws_locked = self.workspace.lock().await;
        let files = project.visible_files(&ws_locked);
        drop(ws_locked);

        let mut items = Vec::new();
        for (rel, path) in files {
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            let Ok(text) = tokio::fs::read_to_string(&path).await else {
                continue;
            };
            let Some(tree) = parser::parse(&text) else {
                continue;
            };
            let table = symbols::build_symbol_table(&tree, &text);
            for sym in &table.symbols {
                if !query.is_empty() && !sym.name.to_lowercase().contains(&query) {
                    continue;
                }
                items.push(symbol_to_workspace_symbol(&sym, &uri, &rel));
            }
        }

        debug!("workspace_symbol: {} item(s)", items.len());
        Ok(Some(items))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
            debug!("references: no identifier at {:?}", pos);
            return Ok(None);
        };

        // Engine API has no source location — return an empty list rather
        // than a malformed Location.
        if self.engine.find_syscall(&ident).is_some()
            || self.engine.find_aiplan(&ident).is_some()
        {
            debug!("references: `{ident}` is engine API; returning []");
            return Ok(Some(vec![]));
        }

        let Some(tree) = parser::parse(&text) else {
            debug!("references: parse failed for {}", uri);
            return Ok(None);
        };
        let ranges = {
            let tables = self.symbol_tables.lock().await;
            let table = tables.get(uri);
            let raw = references::find_identifier_uses(&tree, &text, &ident);
            match table {
                Some(t) => references::filter_declaration(raw, t, &ident, params.context.include_declaration),
                None => raw,
            }
        };

        debug!(
            "references: `{ident}` -> {} range(s) in {}",
            ranges.len(),
            uri
        );
        Ok(Some(references::to_locations(uri, ranges)))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = &params.new_name;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
            debug!("rename: no identifier at {:?}", pos);
            return Ok(None);
        };

        // Engine API symbols can't be renamed — no source to rewrite.
        if self.engine.find_syscall(&ident).is_some()
            || self.engine.find_aiplan(&ident).is_some()
        {
            debug!("rename: `{ident}` is engine API; refusing");
            return Ok(None);
        }

        let Some(tree) = parser::parse(&text) else {
            debug!("rename: parse failed for {}", uri);
            return Ok(None);
        };

        let raw = references::find_identifier_uses(&tree, &text, &ident);
        let ranges = {
            let tables = self.symbol_tables.lock().await;
            match tables.get(uri) {
                Some(t) => references::filter_declaration(raw, t, &ident, /* include */ true),
                None => raw,
            }
        };

        if ranges.is_empty() {
            return Ok(None);
        }

        let edits: Vec<TextEdit> = ranges
            .into_iter()
            .map(|range| TextEdit {
                range,
                new_text: new_name.clone(),
            })
            .collect();

        let mut changes = HashMap::new();
        changes.insert(uri.clone(), edits);
        debug!(
            "rename: `{ident}` -> `{}` ({} edit(s) in {})",
            new_name,
            changes.values().map(|v| v.len()).sum::<usize>(),
            uri
        );
        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = &params.text_document.uri;
        let pos = params.position;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
            debug!("prepare_rename: no identifier at {:?}", pos);
            return Ok(None);
        };

        // Engine API symbols can't be renamed.
        if self.engine.find_syscall(&ident).is_some()
            || self.engine.find_aiplan(&ident).is_some()
        {
            debug!("prepare_rename: `{ident}` is engine API; refusing");
            return Ok(None);
        }

        let Some(tree) = parser::parse(&text) else {
            debug!("prepare_rename: parse failed for {}", uri);
            return Ok(None);
        };

        let Some(range) = references::identifier_range_at(&tree, pos.line, pos.character) else {
            debug!("prepare_rename: no identifier node under {:?}", pos);
            return Ok(None);
        };
        Ok(Some(PrepareRenameResponse::Range(range)))
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

    /// Parse `text` as XS and publish parse, type-check, and semantic
    /// diagnostics. An empty `Vec` (clean parse) is also published so clients
    /// clear stale diagnostics for this URI.
    async fn publish_diagnostics(&self, uri: &Url, text: &str, version: i32) {
        let current_file = uri.to_file_path().ok();
        let project = if current_file.is_some() {
            self.build_semantic_project(uri).await
        } else {
            None
        };
        let merged = self.build_merged_view_for_uri(uri, text).await;

        let diagnostics = match parser::parse(text) {
            Some(tree) => {
                // Hold the symbol-tables lock briefly to look up the
                // per-file table; releasing before the heavier checks keeps
                // the lock window minimal.
                let table = {
                    let tables = self.symbol_tables.lock().await;
                    tables.get(uri).cloned()
                };
                match table {
                    Some(table) => diagnostics::collect_all(
                        &tree,
                        text,
                        &self.engine,
                        &table,
                        project.as_ref(),
                        current_file.as_deref(),
                        merged.as_ref(),
                    ),
                    None => diagnostics::collect_diagnostics(&tree, text),
                }
            }
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

    /// Build a semantic virtual project for the mod that owns `uri`.
    /// Returns `None` for unowned files or if the project cannot be read.
    async fn build_semantic_project(&self, uri: &Url) -> Option<semantic::VirtualProject> {
        let (ws_clone, project) = {
            let ws = self.workspace.lock().await;
            let entry = ws.lookup_mod(uri).cloned()?;
            let project = ws.build_virtual_project(&entry);
            (ws.clone(), project)
        };
        let cache_dir = crate::cache::state_cache_dir();
        match semantic::VirtualProject::load_from_workspace(&ws_clone, &project, &cache_dir) {
            Ok(p) => Some(p),
            Err(e) => {
                warn!("failed to build semantic project for {}: {}", uri, e);
                None
            }
        }
    }
}

/// Notify the client that a file is not covered by any registered mod.
async fn warn_unowned_file(client: &Client, uri: &Url) {
    warn!(
        "file not part of any registered mod; engine API only: {}",
        uri
    );
    client
        .show_message(
            MessageType::WARNING,
            "File not part of any registered mod; engine API only",
        )
        .await;
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

/// Convert a workspace symbol to the LSP `SymbolInformation` shape for
/// `workspace/symbol` responses.
fn symbol_to_workspace_symbol(s: &symbols::Symbol, uri: &Url, _rel: &str) -> SymbolInformation {
    let kind = match s.kind {
        symbols::SymbolKind::Rule => SymbolKind::FUNCTION,
        symbols::SymbolKind::Function => SymbolKind::FUNCTION,
        symbols::SymbolKind::Variable => SymbolKind::VARIABLE,
        symbols::SymbolKind::Constant => SymbolKind::CONSTANT,
    };
    SymbolInformation {
        name: s.name.clone(),
        kind,
        tags: None,
        deprecated: None,
        location: Location {
            uri: uri.clone(),
            range: s.selection_range,
        },
        container_name: None,
    }
}
