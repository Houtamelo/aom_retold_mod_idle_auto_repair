use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use tokio::sync::Mutex;
use tower_lsp_server::{Client, LanguageServer, jsonrpc::Result, ls_types::*};
use tracing::{debug, info, warn};

use crate::{
    completion,
    diagnostics,
    engine_api,
    include_graph,
    merged_view,
    parser,
    references,
    semantic,
    semantic_tokens,
    signature_help,
    symbols,
    word,
    workspace,
};

/// Holds the parsed-but-not-yet-processed text of every document the client
/// has opened. Populated by `did_open` / `did_change`, cleared by `did_close`.
#[derive(Default)]
pub struct DocumentStore {
    inner: HashMap<Uri, String>,
}

impl DocumentStore {
    pub fn open(&mut self, uri: Uri, text: String) { self.inner.insert(uri, text); }

    pub fn change(&mut self, uri: &Uri, text: String) { self.inner.insert(uri.clone(), text); }

    pub fn close(&mut self, uri: &Uri) { self.inner.remove(uri); }

    pub fn get(&self, uri: &Uri) -> Option<&str> { self.inner.get(uri).map(String::as_str) }

    pub fn uris(&self) -> Vec<Uri> { self.inner.keys().cloned().collect() }
}

/// Lock-pattern helpers for `did_close`. Kept in a private inner module and
/// re-exported as `#[doc(hidden)]` public items only so the integration tests
/// can exercise the lock-discipline invariant.
mod lock_pattern {
    use std::{collections::HashMap, future::Future, ops::DerefMut, sync::Arc};

    use tokio::sync::Mutex;
    use tower_lsp_server::ls_types::Uri;

    use super::DocumentStore;
    use crate::symbols;

    /// Internal async-mutex abstraction so `did_close_lock_pattern` can be
    /// exercised in tests with an instrumented mutex.
    pub trait AsyncMutex<T: ?Sized + Send>: Send + Sync {
        type Guard<'a>: DerefMut<Target = T> + Send + 'a
        where Self: 'a;
        fn lock(&self) -> impl Future<Output = Self::Guard<'_>> + Send;
    }

    impl<T: ?Sized + Send> AsyncMutex<T> for Mutex<T> {
        type Guard<'a>
            = tokio::sync::MutexGuard<'a, T>
        where T: 'a;
        fn lock(&self) -> impl Future<Output = Self::Guard<'_>> + Send { Mutex::lock(self) }
    }

    impl<T: ?Sized + Send> AsyncMutex<T> for Arc<Mutex<T>> {
        type Guard<'a>
            = tokio::sync::MutexGuard<'a, T>
        where T: 'a;
        fn lock(&self) -> impl Future<Output = Self::Guard<'_>> + Send { Mutex::lock(self) }
    }

    /// Lock-acquisition sequence for `textDocument/didClose`. Each state-bearing
    /// mutex is held in its own scoped block so that no more than one guard is
    /// alive at any await point.
    pub async fn did_close_lock_pattern<D, S, Clear>(documents: &D, symbol_tables: &S, clear_cache: Clear, uri: &Uri)
    where
        D: AsyncMutex<DocumentStore>,
        S: AsyncMutex<HashMap<Uri, symbols::SymbolTable>>,
        Clear: Future<Output = ()> + Send,
    {
        {
            let mut docs = documents.lock().await;
            docs.close(uri);
        }
        {
            let mut tables = symbol_tables.lock().await;
            tables.remove(uri);
        }
        clear_cache.await;
    }
}

/// Internal helpers for `did_close`. Re-exported as `#[doc(hidden)]` public
/// items only so the integration tests can exercise the lock-discipline
/// invariant; not part of the stable LSP API.
#[doc(hidden)]
pub use self::lock_pattern::{AsyncMutex, did_close_lock_pattern};

/// LSP server state and request handlers.
///
/// All handlers that acquire more than one of the `Arc<Mutex<...>>` fields
/// MUST scope each `lock().await` in its own `{ ... }` block so the guard is
/// dropped before the next `.await`. No handler should hold two mutex guards
/// at the same time.
pub struct XsLanguageServer {
    pub client: Client,
    pub documents: Arc<Mutex<DocumentStore>>,
    /// Per-file symbol tables, rebuilt on every `did_open` / `did_change`.
    pub symbol_tables: Arc<Mutex<HashMap<Uri, symbols::SymbolTable>>>,
    /// Per-file merged include-paste views, keyed by content-hash.
    pub merged_views: Arc<Mutex<HashMap<Uri, (merged_view::MergedViewCacheKey, merged_view::MergedView)>>>,
    pub engine: engine_api::SharedEngineApi,
    pub game_path: PathBuf,
    /// Registered workspace folders, each representing one mod.
    pub workspace: Arc<Mutex<workspace::Workspace>>,
    /// Session-only cache of merged views built from root files, invalidated
    /// by `did_change_watched_files`.
    pub per_root_cache: crate::cache::PerRootMergedViewCache,
    /// Most recently touched document, used to scope `workspace/symbol`.
    pub last_active_uri: Arc<Mutex<Option<Uri>>>,
    /// Capabilities advertised by the client on initialize.
    pub client_capabilities: Arc<Mutex<ClientCapabilities>>,
}

impl XsLanguageServer {
    pub fn new(client: Client, engine: engine_api::SharedEngineApi, game_path: PathBuf) -> Self {
        let workspace = workspace::Workspace::new(game_path.clone());
        let cache_dir = crate::cache::state_cache_dir();
        let per_root_cache = crate::cache::PerRootMergedViewCache::new(&cache_dir);
        Self {
            client,
            documents: Arc::new(Mutex::new(DocumentStore::default())),
            symbol_tables: Arc::new(Mutex::new(HashMap::new())),
            merged_views: Arc::new(Mutex::new(HashMap::new())),
            engine,
            game_path,
            workspace: Arc::new(Mutex::new(workspace)),
            per_root_cache,
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
                let registered: Vec<String> = {
                    let ws = self.workspace.lock().await;
                    ws.mods().iter().map(|m| m.mod_uri.to_string()).collect()
                };
                let file_path = uri.to_file_path();
                warn_unowned_file(&self.client, &uri, &registered, file_path.as_deref()).await;
            }
        }
    }

    /// Return a cached merged view for `uri` when the content hash matches,
    /// otherwise build a new one from the current buffer and cache it.
    async fn get_or_build_merged_view(&self, uri: &Uri, text: &str) -> Option<merged_view::MergedView> {
        if let Some(cached) = self.cached_merged_view(uri, text).await {
            return Some(cached);
        }
        let current_file = uri.to_file_path()?;
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
        let view = merged_view::MergedView::build(&current_file, text, &own_table, &ws_clone, &project, &cache_dir);
        let key = merged_view::MergedViewCacheKey::new(text, &view);
        {
            let mut views = self.merged_views.lock().await;
            views.insert(uri.clone(), (key, view.clone()));
        }
        Some(view)
    }

    /// Look up a cached merged view for `uri` that matches `text`.
    ///
    /// Returns `None` when there is no cache entry or the content hash differs,
    /// forcing a rebuild on the next call to [`Self::get_or_build_merged_view`].
    async fn cached_merged_view(&self, uri: &Uri, text: &str) -> Option<merged_view::MergedView> {
        let views = self.merged_views.lock().await;
        let (key, view) = views.get(uri)?;
        let expected = merged_view::MergedViewCacheKey::new(text, view);
        if key == &expected { Some(view.clone()) } else { None }
    }

    /// Remove a single cached merged view. Called on `textDocument/didOpen`
    /// (to clear stale state) and `textDocument/didClose`.
    async fn clear_merged_view_cache(&self, uri: &Uri) {
        let mut views = self.merged_views.lock().await;
        views.remove(uri);
    }

    /// Drop every cached merged view whose include closure contains any of
    /// `changed_paths`. Returns the affected open-file URIs so callers can
    /// re-diagnose them.
    async fn invalidate_merged_views_for(&self, changed_paths: &[PathBuf]) -> Vec<Uri> {
        if changed_paths.is_empty() {
            return Vec::new();
        }
        let mut views = self.merged_views.lock().await;
        let mut affected = Vec::new();
        for uri in views.keys().cloned().collect::<Vec<_>>() {
            let impacted = views
                .get(&uri)
                .map(|(_, mv)| changed_paths.iter().any(|p| mv.files().any(|f| f == p)))
                .unwrap_or(false);
            if impacted {
                views.remove(&uri);
                affected.push(uri);
            }
        }
        affected
    }
}

impl LanguageServer for XsLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Seed workspace folders from the initialize request (added by clients
        // such as IntelliJ at startup).
        if let Some(ref folders) = params.workspace_folders {
            let mut ws = self.workspace.lock().await;
            for folder in folders {
                let uri = folder.uri.clone();
                if let Err(e) = ws.register_mod(uri.clone()) {
                    warn!("failed to register workspace folder {uri:?}: {e}");
                }
            }
        }

        {
            let mut caps = self.client_capabilities.lock().await;
            *caps = params.capabilities;
        }

        // Prefer the first workspace folder; `root_uri` was deprecated in favor
        // of `workspace_folders` in LSP 3.17. Modern clients send folders; older
        // clients may still send only `root_uri`, but the deprecation makes reading
        // it a warning, so we don't touch it here.
        let root_uri = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first().map(|f| f.uri.clone()));
        info!(
            "initialize: client={:?}, root_uri={:?}, workspace_folders={}, capabilities=present ({} syscalls, {} \
             aiplans loaded)",
            params.client_info.map(|i| i.name),
            root_uri,
            self.workspace.lock().await.mods().len(),
            self.engine.syscalls.len(),
            self.engine.aiplans.len(),
        );

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // We re-parse on every change. Full sync is fine for now;
                // incremental sync lands in week 4 if needed.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions {
                    // We don't need resolve_provider; detail is in the item.
                    resolve_provider: Some(false),
                    // Trigger on every identifier character and dot.
                    trigger_characters: Some(vec![".".to_string(), "_".to_string()]),
                    ..Default::default()
                }),
                // Week 2: hover returns the syscall signature + help as
                // Markdown. No options (no work-done progress, no dynamic
                // registration) — keep it simple until clients ask for more.
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Engine-API symbols have no source location and return
                // `null`; workspace-defined symbols jump to their real
                // declaration. No linkSupport yet.
                definition_provider: Some(OneOf::Left(true)),
                // Week 3: document symbol outline built from the per-file
                // symbol table.
                document_symbol_provider: Some(OneOf::Left(true)),
                // Find all uses of an identifier across the workspace.
                // Engine-API symbols are resolved by scanning visible files.
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
                    file_operations:   None,
                }),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                    identifier: Some("xs-language-server".to_string()),
                    inter_file_dependencies: true,
                    workspace_diagnostics: false,
                    work_done_progress_options: Default::default(),
                })),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                semantic_tokens_provider: Some(semantic_tokens::server_capabilities()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("initialized: client confirmed init");

        let dynamic_supported = {
            let caps = self.client_capabilities.lock().await;
            caps.workspace
                .as_ref()
                .and_then(|w| w.did_change_watched_files.as_ref())
                .and_then(|d| d.dynamic_registration)
                .unwrap_or(false)
        };

        if dynamic_supported {
            // Register one `**/*.xs` watcher per workspace root (the game
            // install folder plus every registered mod). Using the same
            // registration id makes re-registration idempotent.
            let patterns = {
                let ws = self.workspace.lock().await;
                let mut patterns = Vec::new();
                patterns.push(format!(
                    "{}/game/**/*.xs",
                    self.game_path.to_string_lossy().replace('\\', "/")
                ));
                for m in ws.mods() {
                    patterns.push(format!(
                        "{}/game/**/*.xs",
                        m.mod_path.to_string_lossy().replace('\\', "/")
                    ));
                }
                patterns
            };

            let client = self.client.clone();
            tokio::spawn(async move {
                let watchers = patterns
                    .into_iter()
                    .map(|pattern| FileSystemWatcher {
                        glob_pattern: GlobPattern::String(pattern),
                        kind: None,
                    })
                    .collect();
                let options = DidChangeWatchedFilesRegistrationOptions { watchers };
                let registration = Registration {
                    id: "xs-watched-files".to_string(),
                    method: "workspace/didChangeWatchedFiles".to_string(),
                    register_options: Some(serde_json::to_value(options).unwrap_or(serde_json::Value::Null)),
                };
                if let Err(e) = client.register_capability(vec![registration]).await {
                    warn!("failed to register didChangeWatchedFiles watcher: {}", e);
                } else {
                    info!("registered didChangeWatchedFiles watcher for workspace folders");
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
        debug!("did_open: {:?} ({} bytes, v{})", uri, text.len(), version);
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
            // Log enough context to diagnose why lookup failed: which mods
            // are registered, what the file path resolved to, and what
            // prefix match would have succeeded.
            let ws = self.workspace.lock().await;
            let registered: Vec<String> = ws.mods().iter().map(|m| m.mod_uri.to_string()).collect();
            let file_path = uri.to_file_path();
            warn_unowned_file(&self.client, &uri, registered.as_slice(), file_path.as_deref()).await;
        } else {
            info!(
                "did_open: {:?} owned by mod_uri={}",
                uri,
                owning_mod.as_ref().map(|m| m.mod_uri.as_str()).unwrap_or("?")
            );
        }

        self.clear_merged_view_cache(&uri).await;
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
        debug!("did_change: {:?} ({} bytes, v{})", uri, text.len(), version);
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
        debug!("did_close: {:?}", uri);
        did_close_lock_pattern(&self.documents, &self.symbol_tables, self.clear_merged_view_cache(&uri), &uri).await;
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        let added_count = params.event.added.len();
        let removed_count = params.event.removed.len();
        info!("did_change_workspace_folders: +{} -{}", added_count, removed_count);

        {
            let mut ws = self.workspace.lock().await;
            for folder in &params.event.added {
                info!("  + registering mod folder: uri={:?} name={}", folder.uri, folder.name);
                if let Err(e) = ws.register_mod(folder.uri.clone()) {
                    warn!("failed to add workspace folder {:?}: {}", folder.uri, e);
                }
            }
            for folder in &params.event.removed {
                info!("  - unregistering mod folder: uri={:?}", folder.uri);
                ws.unregister_mod(&folder.uri);
            }
            let summary: Vec<String> = ws
                .mods()
                .iter()
                .map(|m| format!("{:?} (overlay={})", m.mod_uri, m.overlay_path.display()))
                .collect();
            info!("registered mods after update ({} total):", summary.len());
            for line in summary {
                info!("    {}", line);
            }
        }

        self.revalidate_open_document_ownership().await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        info!("did_change_watched_files: {} change(s)", params.changes.len());
        let mut changed_paths = Vec::new();
        for change in &params.changes {
            debug!("watched file change: {:?} {:?}", change.typ, change.uri);
            let Some(path) = change.uri.to_file_path() else {
                continue;
            };

            changed_paths.push(path.to_path_buf());
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

        // Drop cached root-chain views that contain any changed file. PR-6 will
        // add a dependent-URI lookup to re-publish diagnostics for files that
        // include the changed file.
        self.per_root_cache.invalidate_for_paths(&changed_paths);

        // Drop any cached merged view whose include closure contains a
        // changed file, then re-diagnose those open files.
        let affected = self.invalidate_merged_views_for(&changed_paths).await;
        for uri in affected {
            let (text, version) = {
                let docs = self.documents.lock().await;
                docs.get(&uri).map(|t| (t.to_string(), 0i32)).unwrap_or_default()
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
        let merged = self.get_or_build_merged_view(uri, &text).await;
        let items = completion::complete(&self.engine, merged.as_ref(), &text, &params);
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
        // Merged include-paste scope second, with the current file's symbol
        // table as a final fallback.
        let markdown = if let Some(s) = self.engine.find_syscall(&ident) {
            format_hover_syscall(s)
        } else if let Some(c) = self.engine.find_aiplan(&ident) {
            format_hover_aiplan(c)
        } else if let Some(ms) = self
            .get_or_build_merged_view(uri, &text)
            .await
            .and_then(|mv| mv.find(&ident).cloned())
        {
            format_hover_merged_symbol(&ms)
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
                kind:  MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        let merged = self.get_or_build_merged_view(uri, &text).await;
        let own_table = {
            let tables = self.symbol_tables.lock().await;
            tables.get(uri).cloned()
        };

        let help =
            signature_help::signature_help(&text, pos, &self.engine, merged.as_ref(), own_table.as_ref());
        debug!("signature_help: {:?} -> {}", pos, help.is_some());
        Ok(help)
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = {
            let docs = self.documents.lock().await;
            docs.get(uri).unwrap_or("").to_string()
        };

        // -----------------------------------------------------------------
        // Phase 0: include-directive path token (runs before identifier
        // resolution; a miss falls through to the existing logic below)
        // -----------------------------------------------------------------
        if let Some(current_file) = uri.to_file_path() {
            if let Some(include_target) =
                parser::detect_include_path_at_position(&current_file, &text, pos.line, pos.character)
            {
                let (ws_clone, project) = {
                    let ws = self.workspace.lock().await;
                    let entry = ws.lookup_mod(uri);
                    let project = match entry {
                        Some(e) => ws.build_virtual_project(e),
                        None => workspace::VirtualProject::default(),
                    };
                    (ws.clone(), project)
                };

                if let Some(resolved) = ws_clone.resolve_include_for_file(&project, &current_file, &include_target) {
                    let def_uri = Uri::from_file_path(&resolved)
                        .ok_or_else(|| tower_lsp_server::jsonrpc::Error::internal_error())?;
                    let range = Range::new(Position::new(0, 0), Position::new(0, 0));
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location { uri: def_uri, range })));
                }
                return Ok(None);
            }
        }

        let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
            debug!("definition: no identifier at {:?}", pos);
            return Ok(None);
        };

        // Merged include-paste scope first so included symbols jump to their
        // defining file. Fall back to the current file's symbol table.
        // Engine-API symbols have no source location and return null.
        let location = if let Some(ms) = self
            .get_or_build_merged_view(uri, &text)
            .await
            .and_then(|mv| mv.find(&ident).cloned())
        {
            let def_path = ms
                .provenance
                .origin()
                .map(PathBuf::from)
                .or_else(|| uri.to_file_path().map(|p| p.to_path_buf()))
                .ok_or_else(tower_lsp_server::jsonrpc::Error::internal_error)?;
            let def_uri =
                Uri::from_file_path(&def_path).ok_or_else(|| tower_lsp_server::jsonrpc::Error::internal_error())?;
            Location {
                uri:   def_uri,
                range: ms.symbol.selection_range,
            }
        } else {
            let tables = self.symbol_tables.lock().await;
            match tables.get(uri).and_then(|t| t.find(&ident)) {
                Some(sym) => Location {
                    uri:   uri.clone(),
                    range: sym.selection_range,
                },
                None => {
                    if self.engine.find_syscall(&ident).is_some() || self.engine.find_aiplan(&ident).is_some() {
                        debug!("definition: `{ident}` is engine API; returning null");
                    } else {
                        debug!("definition: identifier `{ident}` not in engine API or workspace");
                    }
                    return Ok(None);
                }
            }
        };

        debug!("definition: `{ident}` -> {:?}", location.range.start);
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let tables = self.symbol_tables.lock().await;
        let Some(table) = tables.get(uri) else {
            return Ok(None);
        };
        let items = build_document_symbol_tree(table);
        debug!("document_symbol: {} top-level symbol(s) for {:?}", items.len(), uri);
        Ok(Some(DocumentSymbolResponse::Nested(items)))
    }

    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let text = { self.documents.lock().await.get(&uri).unwrap_or("").to_string() };
        let current_file = uri.to_file_path();
        let merged = match current_file {
            Some(_) => self.get_or_build_merged_view(&uri, &text).await,
            None => None,
        };
        let own_table = {
            let tables = self.symbol_tables.lock().await;
            tables.get(&uri).cloned().unwrap_or_default()
        };
        let (project, ws) = {
            let ws = self.workspace.lock().await;
            let project = ws
                .lookup_mod(&uri)
                .map(|e| ws.build_virtual_project(e))
                .unwrap_or_default();
            (project, ws.clone())
        };
        let cache_dir = crate::cache::state_cache_dir();
        let member_index =
            semantic_tokens::MemberIndex::build(&own_table, &ws, &project, &cache_dir, current_file.as_deref());
        let tokens = semantic_tokens::compute_tokens(
            &text,
            current_file.as_deref(),
            &own_table,
            merged.as_ref(),
            &self.engine,
            &ws,
            &project,
            &member_index,
        );
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: semantic_tokens::encode(&tokens),
        })))
    }

    async fn symbol(&self, params: WorkspaceSymbolParams) -> Result<Option<WorkspaceSymbolResponse>> {
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
            let Some(uri) = Uri::from_file_path(&path) else {
                continue;
            };
            let Ok(text) = tokio::fs::read_to_string(&path).await else {
                continue;
            };
            let table = symbols::build_symbol_table(&text);
            for sym in &table.symbols {
                if !query.is_empty() && !sym.name.to_lowercase().contains(&query) {
                    continue;
                }
                items.push(symbol_to_workspace_symbol(&sym, &uri, &rel));
            }
        }

        debug!("workspace_symbol: {} item(s)", items.len());
        Ok(Some(items.into()))
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

        let current_file = uri.to_file_path();
        let merged = self.get_or_build_merged_view(uri, &text).await;

        let mut locs = Vec::new();
        let mut seen: HashSet<(String, u32, u32, u32, u32)> = HashSet::new();

        let is_engine_api = self.engine.find_syscall(&ident).is_some() || self.engine.find_aiplan(&ident).is_some();
        let has_workspace_def = is_engine_api
            && (merged.as_ref().and_then(|mv| mv.find(&ident)).is_some()
                || self
                    .symbol_tables
                    .lock()
                    .await
                    .get(uri)
                    .and_then(|t| t.find(&ident))
                    .is_some());

        if is_engine_api && !has_workspace_def {
            // Engine symbols have no declaration. Scan every visible file in
            // the workspace for use sites, with bounded time budgets.
            let (project, ws_locked) = {
                let ws = self.workspace.lock().await;
                let entry = ws.lookup_mod(uri);
                let project = entry.map(|e| ws.build_virtual_project(e)).unwrap_or_default();
                (project, ws.clone())
            };
            let files = project.visible_files(&ws_locked);
            const PER_FILE_BUDGET_MS: u64 = 50;
            const TOTAL_BUDGET_MS: u64 = 500;
            let total_start = std::time::Instant::now();
            for (_rel, path) in files {
                if total_start.elapsed() >= std::time::Duration::from_millis(TOTAL_BUDGET_MS) {
                    warn!(
                        "references: total workspace scan budget of {} ms exceeded for `{ident}`; truncating",
                        TOTAL_BUDGET_MS
                    );
                    break;
                }
                let file_start = std::time::Instant::now();
                let Ok(file_text) = tokio::fs::read_to_string(&path).await else {
                    continue;
                };
                let raw = references::find_identifier_uses(&file_text, &ident);
                let Some(file_uri) = Uri::from_file_path(&path) else {
                    continue;
                };
                for range in raw {
                    if seen.insert((
                        file_uri.to_string(),
                        range.start.line,
                        range.start.character,
                        range.end.line,
                        range.end.character,
                    )) {
                        locs.push(Location {
                            uri: file_uri.clone(),
                            range,
                        });
                    }
                }
                if file_start.elapsed() >= std::time::Duration::from_millis(PER_FILE_BUDGET_MS) {
                    warn!(
                        "references: per-file scan budget of {} ms exceeded for {}",
                        PER_FILE_BUDGET_MS,
                        path.display()
                    );
                }
            }
            debug!("references: `{ident}` -> {} engine-API location(s) across workspace", locs.len());
            return Ok(Some(locs));
        }

        if let Some(mv) = merged {
            // Walk every file in the include-paste scope.
            for path in mv.files() {
                let source = mv.source(path).unwrap_or("");
                let table =
                    if current_file.as_deref() == Some(path) { Some(mv.own_table()) } else { mv.tables().get(path) };
                let raw = references::find_identifier_uses(source, &ident);
                let ranges = match table {
                    Some(t) => references::filter_declaration(raw, t, &ident, params.context.include_declaration),
                    None => raw,
                };
                let Some(file_uri) = Uri::from_file_path(path) else {
                    continue;
                };
                for range in ranges {
                    if seen.insert((
                        file_uri.to_string(),
                        range.start.line,
                        range.start.character,
                        range.end.line,
                        range.end.character,
                    )) {
                        locs.push(Location {
                            uri: file_uri.clone(),
                            range,
                        });
                    }
                }
            }
        } else {
            // Fallback to the current file only when no merged view is available.
            let ranges = {
                let tables = self.symbol_tables.lock().await;
                let table = tables.get(uri);
                let raw = references::find_identifier_uses(&text, &ident);
                match table {
                    Some(t) => references::filter_declaration(raw, t, &ident, params.context.include_declaration),
                    None => raw,
                }
            };
            for range in ranges {
                if seen.insert((
                    uri.to_string(),
                    range.start.line,
                    range.start.character,
                    range.end.line,
                    range.end.character,
                )) {
                    locs.push(Location {
                        uri: uri.clone(),
                        range,
                    });
                }
            }
        }

        debug!("references: `{ident}` -> {} location(s) across merged scope", locs.len());
        Ok(Some(locs))
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
        if self.engine.find_syscall(&ident).is_some() || self.engine.find_aiplan(&ident).is_some() {
            debug!("rename: `{ident}` is engine API; refusing");
            return Ok(None);
        }

        let raw = references::find_identifier_uses(&text, &ident);
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
            "rename: `{ident}` -> `{}` ({} edit(s) in {:?})",
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

    async fn prepare_rename(&self, params: TextDocumentPositionParams) -> Result<Option<PrepareRenameResponse>> {
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
        if self.engine.find_syscall(&ident).is_some() || self.engine.find_aiplan(&ident).is_some() {
            debug!("prepare_rename: `{ident}` is engine API; refusing");
            return Ok(None);
        }

        let Some(range) = references::identifier_range_at(&text, pos.line, pos.character) else {
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
    async fn rebuild_symbol_table(&self, uri: &Uri, text: &str) {
        let table = symbols::build_full_symbol_table(text);
        debug!("rebuild_symbol_table: {:?} -> {} symbol(s)", uri, table.symbols.len());
        let mut tables = self.symbol_tables.lock().await;
        tables.insert(uri.clone(), table);
    }

    /// Parse `text` as XS and publish parse, type-check, and semantic
    /// diagnostics per URI. An empty entry for a URI (clean file) is also
    /// published so clients clear stale diagnostics for that file.
    ///
    /// PR-5 orchestration:
    ///   1. Run a local pass with the file treated as its own root.
    ///   2. Compute the reverse-include roots of the current file.
    ///   3. If there are no roots, or if the local pass has no cross-file
    ///      symbol issues, publish the local pass directly.
    ///   4. Otherwise, run one diagnostic pass per root, rebase each root view
    ///      to the current file, and aggregate equivalent diagnostics.
    async fn publish_diagnostics(&self, uri: &Uri, text: &str, version: i32) {
        let current_file = uri.to_file_path();

        // Build the semantic project and keep the workspace project around so
        // we can build root-chain views for indirect-include resolution.
        let (project, ws, workspace_project) = if let Some(_cf) = current_file.as_ref() {
            let ws = self.workspace.lock().await;
            match ws.lookup_mod(uri).cloned() {
                Some(entry) => {
                    let wp = ws.build_virtual_project(&entry);
                    let cache_dir = crate::cache::state_cache_dir();
                    let semantic = semantic::VirtualProject::load_from_workspace(&ws, &wp, &cache_dir);
                    (Some(semantic), ws.clone(), Some(wp))
                }
                None => (None, ws.clone(), None),
            }
        } else {
            (None, workspace::Workspace::new(self.game_path.clone()), None)
        };

        let merged = self.get_or_build_merged_view(uri, text).await;

        // Hold the symbol-tables lock briefly to look up the per-file table;
        // releasing before the heavier checks keeps the lock window minimal.
        let table = {
            let tables = self.symbol_tables.lock().await;
            tables.get(uri).cloned()
        };

        let Some(file_view) = merged.as_ref() else {
            // No merged view available: fall back to parser diagnostics only.
            let mut map = HashMap::new();
            map.insert(uri.clone(), diagnostics::collect_diagnostics(text));
            for (diag_uri, diags) in map {
                self.client.publish_diagnostics(diag_uri, diags, Some(version)).await;
            }
            return;
        };
        let Some(table) = table else {
            let mut map = HashMap::new();
            map.insert(uri.clone(), diagnostics::collect_diagnostics(text));
            for (diag_uri, diags) in map {
                self.client.publish_diagnostics(diag_uri, diags, Some(version)).await;
            }
            return;
        };

        // Local pass: the file is analysed as its own root. This is the
        // fallback path for orphan files and the short-circuit path when no
        // cross-file issues are present.
        let local_ctx = diagnostics::DiagnosticContext {
            source: text,
            engine: &self.engine,
            table: &table,
            project: project.as_ref(),
            file: current_file.as_deref(),
            file_view,
            root_view: Some(file_view),
        };
        let local_diags = diagnostics::collect_all(&local_ctx);

        // Reverse-include graph built from the file's forward closure. In PR-5
        // this is intentionally local to the file; the project-wide graph is
        // scheduled for PR-6.
        let roots: Vec<PathBuf> = current_file
            .as_deref()
            .map(|cf| {
                let graph = include_graph::ReverseIncludeGraph::build(file_view.graph());
                graph.roots_that_include(cf)
            })
            .unwrap_or_default();

        // Orphan files, or files whose local diagnostics are already complete,
        // publish the local pass directly and skip the expensive per-root loop.
        if roots.is_empty() || diagnostics::should_skip_multi_root_pass(&local_diags) {
            let total: usize = local_diags.values().map(|v| v.len()).sum();
            debug!("publish_diagnostics: {:?} ({} issue(s) local pass)", uri, total);
            for (diag_uri, diags) in local_diags {
                self.client.publish_diagnostics(diag_uri, diags, Some(version)).await;
            }
            return;
        }

        // Multi-root pass: diagnose the file through each root's include chain.
        let fallback_project = workspace::VirtualProject::default();
        let wp = workspace_project.as_ref().unwrap_or(&fallback_project);
        let mut per_root: Vec<diagnostics::PerRootDiagnostic> = Vec::new();
        for root in &roots {
            let root_view = match merged_view::MergedView::build_from_root(root, &ws, wp, &self.per_root_cache) {
                Ok(v) => v,
                Err(e) => {
                    warn!("failed to build root view for {}: {}; skipping root", root.display(), e);
                    continue;
                }
            };
            let Some(cf) = current_file.as_deref() else { continue };
            let rebased = root_view.rebase_to_file(cf, text.to_string(), table.clone());
            let ctx = diagnostics::DiagnosticContext {
                source: text,
                engine: &self.engine,
                table: &table,
                project: project.as_ref(),
                file: Some(cf),
                file_view,
                root_view: Some(&rebased),
            };
            let root_diags = diagnostics::run_pass(&ctx);
            for (diag_uri, diags) in root_diags {
                for d in &diags {
                    per_root.push(diagnostics::PerRootDiagnostic::from_diagnostic(
                        d,
                        diag_uri.clone(),
                        diagnostics::categorize(d),
                        root.clone(),
                    ));
                }
            }
        }

        // Priority-order inputs for the aggregation suffix formatter.
        let currently_open: Vec<PathBuf> = {
            let docs = self.documents.lock().await;
            docs.uris()
                .into_iter()
                .filter(|u| u != uri)
                .filter_map(|u| u.to_file_path().map(|p| p.into_owned()))
                .collect()
        };
        let mod_overlay_paths: Vec<PathBuf> = {
            let ws_lock = self.workspace.lock().await;
            ws_lock.mods().iter().map(|m| m.mod_path.clone()).collect()
        };

        let aggregated =
            diagnostics::aggregate_results(per_root, roots.len(), &mod_overlay_paths, &currently_open);
        let total: usize = aggregated.values().map(|v| v.len()).sum();
        debug!(
            "publish_diagnostics: {:?} ({} issue(s) across {} root(s))",
            uri,
            total,
            roots.len()
        );
        for (diag_uri, diags) in aggregated {
            self.client.publish_diagnostics(diag_uri, diags, Some(version)).await;
        }
    }
}

/// Notify the client that a file is not covered by any registered mod.
/// Logs every registered mod and the resolved file path so a user hitting
/// this warning can grep the log to see exactly which prefixes were checked
/// and which prefix would have matched.
async fn warn_unowned_file(
    client: &Client,
    uri: &Uri,
    registered_mods: &[String],
    file_path: Option<&std::path::Path>,
) {
    warn!("file not part of any registered mod; engine API only: {:?}", uri);
    warn!("  resolved file path: {:?}", file_path);
    if registered_mods.is_empty() {
        warn!("  no mods registered with the LSP at all");
        warn!("  (the IntelliJ plugin must call workspace/didChangeWorkspaceFolders");
        warn!("   with the project's mod paths; check the XsStartupActivity run)");
    } else {
        warn!("  registered mod URIs ({}):", registered_mods.len());
        for m in registered_mods {
            warn!("    - {}", m);
        }
        if let Some(_path) = file_path {
            warn!("  hint: a mod at <mod_uri> owns a file when file_path.starts_with(<mod_uri>);");
            warn!("        check for case sensitivity, trailing slash, or symlink mismatches");
            warn!("        between the registered prefix and the actual file path.");
        }
    }
    client
        .show_message(MessageType::WARNING, "File not part of any registered mod; engine API only")
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

/// Format a merged-scope symbol for hover, including the file it was
/// pasted from.
fn format_hover_merged_symbol(ms: &merged_view::MergedSymbol) -> String {
    let kind_label = ms.symbol.kind.label();
    let mut md = format!("*{}* — `{}`\n", kind_label, ms.symbol.detail);
    if !ms.symbol.params.is_empty() {
        md.push_str("\n**Parameters:**\n");
        for p in &ms.symbol.params {
            md.push_str(&format!("- `{} {}`\n", p.ty, p.name));
        }
    }
    if let Some(origin) = ms.provenance.origin() {
        md.push_str(&format!("\n*Defined in: `{}`*\n", origin.display()));
    }
    md
}

/// Map an XS `SymbolKind` to the LSP `SymbolKind` used by document and
/// workspace symbol responses.
fn lsp_symbol_kind(kind: symbols::SymbolKind) -> SymbolKind {
    match kind {
        symbols::SymbolKind::Rule => SymbolKind::FUNCTION, // XS has no RULE kind in LSP
        symbols::SymbolKind::Function => SymbolKind::FUNCTION,
        symbols::SymbolKind::Variable => SymbolKind::VARIABLE,
        symbols::SymbolKind::Constant => SymbolKind::CONSTANT,
        symbols::SymbolKind::Class => SymbolKind::CLASS,
        symbols::SymbolKind::ClassField => SymbolKind::FIELD,
        symbols::SymbolKind::ClassMethod => SymbolKind::METHOD,
    }
}

/// Build a tree of `DocumentSymbol`s where class members are nested under
/// their owning class.
// tower_lsp_server migration from `deprecated` to `tags` is a breaking change and
// orthogonal to this mechanical clippy cleanup.
#[allow(deprecated)]
fn build_document_symbol_tree(table: &symbols::SymbolTable) -> Vec<DocumentSymbol> {
    // First pass: group members by their owning class. We don't rely on
    // insertion order in `table.symbols` because `extract_class` emits the
    // class symbol before its members, so draining a `pending_members` list
    // at the moment we encounter a class would always see an empty buffer.
    let mut class_members: std::collections::HashMap<String, Vec<&symbols::Symbol>> = std::collections::HashMap::new();
    for sym in &table.symbols {
        if let Some(owner) = sym.class_owner.as_deref() {
            class_members.entry(owner.to_string()).or_default().push(sym);
        }
    }

    // Second pass: emit a DocumentSymbol for every non-member symbol, and
    // attach the pre-collected members under their owning class.
    let mut items = Vec::new();
    for sym in &table.symbols {
        if sym.class_owner.is_some() {
            // Will be emitted under its owning class below.
            continue;
        }

        let children = if sym.kind == symbols::SymbolKind::Class {
            class_members
                .remove(&sym.name)
                .map(|mems| mems.into_iter().map(symbol_to_lsp_child).collect::<Vec<_>>())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        items.push(DocumentSymbol {
            name: sym.name.clone(),
            detail: Some(sym.detail.clone()),
            kind: lsp_symbol_kind(sym.kind),
            tags: None,
            deprecated: None,
            range: sym.full_range,
            selection_range: sym.selection_range,
            children: if children.is_empty() { None } else { Some(children) },
        });
    }

    items
}

// See `build_document_symbol_tree` above: tower_lsp_server deprecated field migration is out of scope.
#[allow(deprecated)]
fn symbol_to_lsp_child(s: &symbols::Symbol) -> DocumentSymbol {
    DocumentSymbol {
        name: s.name.clone(),
        detail: Some(s.detail.clone()),
        kind: lsp_symbol_kind(s.kind),
        tags: None,
        deprecated: None,
        range: s.full_range,
        selection_range: s.selection_range,
        children: None,
    }
}

/// Convert a workspace symbol to the LSP `SymbolInformation` shape for
/// `workspace/symbol` responses.
// See `build_document_symbol_tree` above: tower_lsp_server deprecated field migration is out of scope.
#[allow(deprecated)]
fn symbol_to_workspace_symbol(s: &symbols::Symbol, uri: &Uri, _rel: &str) -> SymbolInformation {
    SymbolInformation {
        name: s.name.clone(),
        kind: lsp_symbol_kind(s.kind),
        tags: None,
        deprecated: None,
        location: Location {
            uri:   uri.clone(),
            range: s.selection_range,
        },
        container_name: s.class_owner.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression for the `extract_class`-emits-class-before-members bug:
    /// `build_document_symbol_tree` was draining a `pending_members` buffer
    /// the moment it saw a `Class` symbol, but the class symbol is pushed
    /// first by `extract_class`, so the buffer was always empty.
    #[test]
    fn class_members_nest_under_their_class_in_document_symbol_tree() {
        let class_only = symbols::Symbol {
            name: "Foo".to_string(),
            kind: symbols::SymbolKind::Class,
            ty: String::new(),
            params: Vec::new(),
            class_owner: None,
            is_extern: false,
            is_mutable: false,
            is_static: false,
            is_forward: false,
            visibility: symbols::Visibility::Public,
            full_range: tower_lsp_server::ls_types::Range::default(),
            selection_range: tower_lsp_server::ls_types::Range::default(),
            detail: "class Foo".to_string(),
        };
        let mut field = class_only.clone();
        field.name = "health".to_string();
        field.kind = symbols::SymbolKind::ClassField;
        field.class_owner = Some("Foo".to_string());
        field.detail = "field health".to_string();
        let mut method = class_only.clone();
        method.name = "takeDamage".to_string();
        method.kind = symbols::SymbolKind::ClassMethod;
        method.class_owner = Some("Foo".to_string());
        method.detail = "method takeDamage".to_string();

        // Members appear AFTER the class (matches `extract_class` ordering).
        let table = symbols::SymbolTable {
            symbols: vec![class_only, field, method],
        };

        let items = build_document_symbol_tree(&table);

        assert_eq!(items.len(), 1, "class should appear once at top level");
        let foo = &items[0];
        assert_eq!(foo.name, "Foo");
        let children = foo.children.as_ref().expect("class should now have children attached");
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"health"), "field 'health' should be nested under class; got {:?}", names);
        assert!(names.contains(&"takeDamage"), "method 'takeDamage' should be nested under class; got {:?}", names);
    }
}
