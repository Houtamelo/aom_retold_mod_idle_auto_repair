use tower_lsp::{LspService, Server};
use tracing_subscriber::EnvFilter;

mod cache;
mod completion;
mod diagnostics;
mod doxygen;
mod engine_api;
mod parser;
mod references;
mod server;
mod symbols;
mod typecheck;
mod word;

use server::XsLanguageServer;

#[tokio::main]
async fn main() {
    // Honor RUST_LOG; default to info so the JSON-RPC traffic stays quiet.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(XsLanguageServer::new).finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}