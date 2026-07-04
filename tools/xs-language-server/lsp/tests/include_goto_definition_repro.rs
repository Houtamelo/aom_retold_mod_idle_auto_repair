//! Regression tests for go-to-definition on `include "..."` directives.
//!
//! Covers vanilla resolution, mod-overlay precedence, vanilla fallback,
//! missing-target null response, and fallback to identifier resolution when
//! the cursor is not on the include path token.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tower_lsp_server::ls_types::Uri;

/// Monotonic counter used to give every `Fixture::new` call a unique
/// temp-dir suffix. Replaces the previous `std::process::id()` based
/// naming, which collided across parallel tests in the same process
/// (cargo runs integration tests in parallel by default, so two
/// fixtures in the same test binary would `remove_dir_all` each
/// other's directories mid-run).
static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_fixture_id() -> u64 {
    FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// ===================================================================
// Helpers — LSP Content-Length framing and JSON-RPC parsing
// ===================================================================

fn frame(body: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    out
}

fn read_framed_message<R: Read>(reader: &mut R) -> std::io::Result<serde_json::Value> {
    let mut byte = [0u8; 1];
    let mut accumulated: Vec<u8> = Vec::new();

    let header_end = loop {
        let n = reader.read(&mut byte)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF before headers complete",
            ));
        }
        accumulated.push(byte[0]);
        if accumulated.len() >= 4 && &accumulated[accumulated.len() - 4..] == b"\r\n\r\n" {
            break accumulated.len();
        }
    };

    let header_str = std::str::from_utf8(&accumulated[..header_end - 4])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let content_length: usize = header_str
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "missing Content-Length")
        })?;

    let mut body = accumulated[header_end..].to_vec();
    while body.len() < content_length {
        let n = reader.read(&mut byte)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF in body",
            ));
        }
        body.push(byte[0]);
    }
    body.truncate(content_length);

    serde_json::from_slice(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[allow(clippy::collapsible_if)]
fn read_response_with_id<R: Read>(
    reader: &mut R,
    expected_id: i64,
) -> std::io::Result<serde_json::Value> {
    loop {
        let msg = read_framed_message(reader)?;
        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
            if id == expected_id {
                return Ok(msg);
            }
        }
    }
}

/// Locate the repo's committed `docs/doxygen_retail.7z` from `CARGO_MANIFEST_DIR`.
fn resolve_doxygen_archive() -> PathBuf {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("repo root from CARGO_MANIFEST_DIR");
    let path = repo_root.join("docs").join("doxygen_retail.7z");
    std::fs::canonicalize(&path).unwrap_or(path)
}

fn write_files(root: &Path, files: &[(&str, &str)]) {
    for (rel, content) in files {
        let path = root.join("game").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }
}

// ===================================================================
// Fixture
// ===================================================================

struct Fixture {
    #[allow(dead_code)]
    game_root: PathBuf,
    mod_root: Option<PathBuf>,
    child: Child,
    stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
    next_id: i64,
}

impl Fixture {
    fn new(vanilla_files: &[(&str, &str)], mod_files: &[(&str, &str)]) -> Fixture {
        let id = next_fixture_id();
        let tmp = std::env::temp_dir();

        let game_root = tmp.join(format!("aomr_include_game_{id}"));
        let _ = std::fs::remove_dir_all(&game_root);
        std::fs::create_dir_all(&game_root).expect("create temp game root");
        std::fs::copy(
            resolve_doxygen_archive(),
            game_root.join("doxygen_retail.7z"),
        )
        .expect("copy doxygen archive");
        write_files(&game_root, vanilla_files);
        let game_root = std::fs::canonicalize(&game_root).unwrap();

        let mod_root = if mod_files.is_empty() {
            None
        } else {
            let root = tmp.join(format!("aomr_include_mod_{id}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("create temp mod root");
            write_files(&root, mod_files);
            Some(std::fs::canonicalize(&root).unwrap())
        };

        let server_path = std::env::var("CARGO_BIN_EXE_xs-language-server")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("target")
                    .join("debug")
                    .join("xs-language-server")
            });
        if !server_path.exists() {
            panic!("xs-language-server binary not found at {:?}", server_path);
        }

        let mut child = Command::new(&server_path)
            .arg("--game-path")
            .arg(&game_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn xs-language-server");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");
        let stderr = child.stderr.take().expect("child stderr");

        Fixture {
            game_root,
            mod_root,
            child,
            stdin,
            stdout,
            stderr,
            next_id: 1,
        }
    }

    fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send(&mut self, body: &str) {
        self.stdin.write_all(&frame(body)).unwrap();
        self.stdin.flush().unwrap();
        std::thread::sleep(Duration::from_millis(50));
    }

    fn initialize(&mut self) {
        let workspace_folders: Vec<_> = self
            .mod_root
            .as_ref()
            .map(|root| {
                let uri = Uri::from_file_path(root).unwrap();
                serde_json::json!({"uri": uri, "name": "include_mod"})
            })
            .into_iter()
            .collect();

        let init = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "initialize",
            "params": {
                "processId": null,
                "capabilities": {},
                "trace": "off",
                "rootUri": null,
                "workspaceFolders": workspace_folders
            }
        })
        .to_string();
        self.send(&init);

        let initialized =
            serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}}).to_string();
        self.send(&initialized);
    }

    fn did_open(&mut self, uri: &Uri, text: &str) {
        let did_open = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "xs",
                    "version": 1,
                    "text": text
                }
            }
        })
        .to_string();
        self.send(&did_open);
    }

    fn definition(&mut self, uri: &Uri, line: u32, character: u32) -> serde_json::Value {
        let id = self.next_id();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/definition",
            "params": {
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }
        })
        .to_string();
        self.stdin.write_all(&frame(&req)).unwrap();
        self.stdin.flush().unwrap();

        read_response_with_id(&mut self.stdout, id).expect("read definition response")
    }

    fn shutdown(mut self) {
        let shutdown =
            serde_json::json!({"jsonrpc":"2.0","id": self.next_id(),"method":"shutdown"})
                .to_string();
        let exit = serde_json::json!({"jsonrpc":"2.0","method":"exit"}).to_string();
        self.send(&shutdown);
        self.send(&exit);
        self.stdin.flush().unwrap();
        drop(self.stdin);
        let _ = self.child.wait();
        let mut stderr_text = String::new();
        let _ = self.stderr.read_to_string(&mut stderr_text);
    }
}

fn uri_for(root: &Path, rel: &str) -> Uri {
    Uri::from_file_path(root.join("game").join(rel)).unwrap()
}

// ===================================================================
// Tests
// ===================================================================

#[test]
fn test_direct_include_resolves_to_target() {
    let mut fx = Fixture::new(
        &[
            ("ai/core/main.xs", "include \"core/debug.xs\";\n"),
            ("ai/core/debug.xs", "void helper() {}\n"),
        ],
        &[],
    );
    fx.initialize();

    let uri = uri_for(&fx.game_root, "ai/core/main.xs");
    fx.did_open(&uri, "include \"core/debug.xs\";\n");

    // Cursor on the `d` of `debug.xs`.
    let resp = fx.definition(&uri, 0, 14);
    let result = resp.get("result").expect("result present");
    let target_uri = result
        .get("uri")
        .and_then(|u| u.as_str())
        .expect("uri present");
    assert!(
        target_uri.ends_with("game/ai/core/debug.xs"),
        "expected game debug.xs, got {}",
        target_uri
    );
    let range = result.get("range").expect("range present");
    assert_eq!(range["start"]["line"], 0);
    assert_eq!(range["start"]["character"], 0);
    assert_eq!(range["end"]["line"], 0);
    assert_eq!(range["end"]["character"], 0);

    fx.shutdown();
}

#[test]
fn test_mod_overlay_resolves_to_mod_file() {
    let mut fx = Fixture::new(
        &[("ai/core/debug.xs", "void vanilla_helper() {}\n")],
        &[
            ("ai/core/main.xs", "include \"core/debug.xs\";\n"),
            ("ai/core/debug.xs", "void mod_helper() {}\n"),
        ],
    );
    fx.initialize();

    let mod_root = fx.mod_root.as_ref().unwrap().clone();
    let uri = uri_for(&mod_root, "ai/core/main.xs");
    fx.did_open(&uri, "include \"core/debug.xs\";\n");

    let resp = fx.definition(&uri, 0, 14);
    let result = resp.get("result").expect("result present");
    let target_uri = result
        .get("uri")
        .and_then(|u| u.as_str())
        .expect("uri present");
    let vanilla_uri = Uri::from_file_path(fx.game_root.join("game").join("ai/core/debug.xs"))
        .unwrap()
        .to_string();
    assert!(
        target_uri != vanilla_uri && target_uri.ends_with("game/ai/core/debug.xs"),
        "expected mod overlay debug.xs, got {}",
        target_uri
    );

    fx.shutdown();
}

#[test]
fn test_vanilla_fallback_when_mod_missing() {
    let mut fx = Fixture::new(
        &[("ai/core/debug.xs", "void vanilla_helper() {}\n")],
        &[("ai/core/main.xs", "include \"core/debug.xs\";\n")],
    );
    fx.initialize();

    let mod_root = fx.mod_root.as_ref().unwrap().clone();
    let uri = uri_for(&mod_root, "ai/core/main.xs");
    fx.did_open(&uri, "include \"core/debug.xs\";\n");

    let resp = fx.definition(&uri, 0, 14);
    let result = resp.get("result").expect("result present");
    let target_uri = result
        .get("uri")
        .and_then(|u| u.as_str())
        .expect("uri present");
    assert!(
        target_uri.ends_with("game/ai/core/debug.xs"),
        "expected vanilla fallback, got {}",
        target_uri
    );

    fx.shutdown();
}

#[test]
fn test_not_found_returns_null() {
    let mut fx = Fixture::new(
        &[("ai/core/main.xs", "include \"does/not/exist.xs\";\n")],
        &[],
    );
    fx.initialize();

    let uri = uri_for(&fx.game_root, "ai/core/main.xs");
    fx.did_open(&uri, "include \"does/not/exist.xs\";\n");

    let resp = fx.definition(&uri, 0, 9);
    let result = resp.get("result");
    assert!(
        result.map(|v| v.is_null()).unwrap_or(false),
        "expected null result for missing include, got {:?}",
        result
    );

    fx.shutdown();
}

#[test]
fn test_cursor_outside_path_token_uses_existing_logic() {
    let mut fx = Fixture::new(
        &[
            (
                "ai/core/main.xs",
                "include \"core/debug.xs\"; void foo() {}\n",
            ),
            ("ai/core/debug.xs", "void helper() {}\n"),
        ],
        &[],
    );
    fx.initialize();

    let uri = uri_for(&fx.game_root, "ai/core/main.xs");
    fx.did_open(&uri, "include \"core/debug.xs\"; void foo() {}\n");

    let resp = fx.definition(&uri, 0, 2);
    let result = resp.get("result");
    assert!(
        result.map(|v| v.is_null()).unwrap_or(false),
        "expected null when cursor is outside include path, got {:?}",
        result
    );

    fx.shutdown();
}
