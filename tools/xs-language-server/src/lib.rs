//! Library entry point for the XS language server.
//!
//! `main.rs` consumes this crate to run the LSP server; the integration tests
//! under `tests/` consume it to validate parsing, symbol extraction, and
//! semantic analysis against the live AoM:R game folder.

pub mod cache;
pub mod completion;
pub mod diagnostics;
pub mod doxygen;
pub mod engine_api;
pub mod parser;
pub mod references;
pub mod semantic;
pub mod server;
pub mod symbols;
pub mod typecheck;
pub mod word;
pub mod workspace;