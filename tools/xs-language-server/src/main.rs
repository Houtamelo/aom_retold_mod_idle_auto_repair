use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use tower_lsp::{LspService, Server};
use tracing::info;
use tracing_subscriber::EnvFilter;

use xs_language_server::cache;
use xs_language_server::engine_api;
use xs_language_server::server::XsLanguageServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Honor RUST_LOG; default to info so the JSON-RPC traffic stays quiet.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let game_path = resolve_game_path()?;
    let archive = game_path.join("doxygen_retail.7z");

    if !archive.exists() {
        bail!(
            "Could not find doxygen_retail.7z in {game_path:?}. \
             Provide the Age of Mythology: Retold installation root \
             (the folder containing doxygen_retail.7z and game/) via \
             --game-path <PATH>, a positional argument, or the AOMR_GAME_PATH \
             environment variable."
        );
    }

    let cache_dir = cache::state_cache_dir();
    let engine = Arc::new(
        engine_api::EngineApi::load_from_archive(&archive, &cache_dir)
            .with_context(|| format!("loading engine API from {archive:?}"))?,
    );

    info!(
        "loaded engine API: {} syscalls, {} aiplans; cache dir: {:?}",
        engine.syscalls.len(),
        engine.aiplans.len(),
        cache_dir,
    );

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(move |client| {
        XsLanguageServer::new(client, Arc::clone(&engine), game_path.clone())
    })
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}

/// Resolve the AoM:R installation root from, in order of precedence:
/// 1. `--game-path <PATH>`
/// 2. a positional argument
/// 3. the `AOMR_GAME_PATH` environment variable
fn resolve_game_path() -> anyhow::Result<PathBuf> {
    let mut args = env::args().skip(1).peekable();
    let mut positional: Option<String> = None;
    let mut explicit: Option<String> = None;

    while let Some(arg) = args.next() {
        if arg == "--game-path" {
            let value = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--game-path requires a value"))?;
            explicit = Some(value);
        } else if let Some(value) = arg.strip_prefix("--game-path=") {
            explicit = Some(value.to_string());
        } else if positional.is_none() && !arg.starts_with('-') {
            positional = Some(arg);
        }
    }

    let raw = explicit
        .or(positional)
        .or_else(|| env::var("AOMR_GAME_PATH").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No game folder provided. Set AOMR_GAME_PATH environment variable, \
                 pass --game-path <PATH>, or configure in IntelliJ Settings → \
                 XS Language Server → Game Folder."
            )
        })?;

    if raw.trim().is_empty() {
        bail!("Game folder path is empty.");
    }

    let path = Path::new(&raw).to_path_buf();
    if !path.exists() {
        bail!("Game folder does not exist: {path:?}");
    }
    Ok(path)
}
