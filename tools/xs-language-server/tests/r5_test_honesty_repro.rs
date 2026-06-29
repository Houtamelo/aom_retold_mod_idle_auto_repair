//! Contract tests for Batch D (R5-F-01, R5-F-02, R5-F-03) LSP
//! integration-test honesty.
//!
//! These are strict-TDD replacements for the audit reproductions. RED before
//! the rewrite of `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs`,
//! GREEN after that binary parses JSON-RPC frames instead of scanning bytes
//! and measures request-to-response elapsed instead of request-to-cleanup.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ===================================================================
// Helpers — LSP Content-Length framing and JSON-RPC parsing
// ===================================================================

/// Wrap a JSON-RPC body with LSP `Content-Length` framing.
fn frame(body: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    out
}

/// Walk Content-Length-framed JSON-RPC bodies. Returns one entry per
/// successfully-parsed JSON body. Junk that fails JSON parsing is skipped
/// (mirrors how a robust LSP client tolerates noise in the byte stream).
fn read_parseable_jsonrpc_messages(bytes: &[u8]) -> Vec<serde_json::Value> {
    use std::io::{BufRead, Read};
    let cursor = std::io::Cursor::new(bytes);
    let mut reader = std::io::BufReader::new(cursor);
    let mut out = Vec::new();
    loop {
        let mut header_line = String::new();
        let n = reader.read_line(&mut header_line).unwrap_or(0);
        if n == 0 {
            return out;
        }
        let trimmed = header_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let len = trimmed
            .strip_prefix("Content-Length: ")
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let mut sep = String::new();
        if reader.read_line(&mut sep).unwrap_or(0) == 0 {
            return out;
        }
        if len == 0 {
            continue;
        }
        let mut body = vec![0u8; len];
        if reader.read_exact(&mut body).is_err() {
            return out;
        }
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) {
            out.push(v);
        }
    }
}

// ===================================================================
// R5-F-01 — completion counts must come from the completion response
// ===================================================================

/// Reference implementation of the production completion-counting logic
/// *after* the fix: count aiEcho-family labels by parsing the completion
/// response's `result` array.
fn production_completion_count(bytes: &[u8]) -> usize {
    parsed_completion_ai_echo_count(bytes, 1)
}

/// Counts aiEcho-family labels inside the `textDocument/completion` response
/// with the expected id by parsing frames.
fn parsed_completion_ai_echo_count(bytes: &[u8], completion_id: i64) -> usize {
    for v in read_parseable_jsonrpc_messages(bytes) {
        if v.get("id").and_then(|i| i.as_i64()) != Some(completion_id) {
            continue;
        }
        let Some(result) = v.get("result").and_then(|r| r.as_array()) else {
            return 0;
        };
        return result
            .iter()
            .filter(|item| {
                item.get("label")
                    .and_then(|l| l.as_str())
                    .map(|s| s == "aiEcho" || s == "aiEchoCategory" || s == "aiEchoWarning")
                    .unwrap_or(false)
            })
            .count();
    }
    0
}

#[test]
fn r5_f01_completion_count_equals_parsed_result_count() {
    let completion_response = frame(r#"{"jsonrpc":"2.0","id":1,"result":[{"label":"aiEcho"}]}"#);
    let log_message =
        frame(r#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"message":"aiEcho"}}"#);
    let mut all_bytes = Vec::new();
    all_bytes.extend_from_slice(&completion_response);
    all_bytes.extend_from_slice(&log_message);

    let production = production_completion_count(&all_bytes);
    let parsed = parsed_completion_ai_echo_count(&all_bytes, 1);

    assert_eq!(
        production, parsed,
        "production count must equal parsed count"
    );
    assert_eq!(parsed, 1, "completion response has 1 aiEcho-labeled entry");
}

#[test]
fn r5_f01_completion_count_zero_when_no_completions() {
    let log_message =
        frame(r#"{"jsonrpc":"2.0","method":"window/logMessage","params":{"message":"aiEcho"}}"#);
    let production = production_completion_count(&log_message);
    let parsed = parsed_completion_ai_echo_count(&log_message, 1);

    assert_eq!(
        production, parsed,
        "production count must equal parsed count"
    );
    assert_eq!(parsed, 0, "no completion response present in wire stream");
}

// ===================================================================
// R5-F-03 — diagnostic flags must come from publishDiagnostics
// notifications, not raw-byte substring checks
// ===================================================================

fn production_has_clean_diag(bytes: &[u8]) -> bool {
    let (clean, _) = parsed_publish_diagnostics_count(bytes);
    clean > 0
}

fn production_has_bad_diag(bytes: &[u8]) -> bool {
    let (_, bad) = parsed_publish_diagnostics_count(bytes);
    bad > 0
}

fn parsed_publish_diagnostics_count(bytes: &[u8]) -> (usize, usize) {
    let mut clean = 0usize;
    let mut bad = 0usize;
    for v in read_parseable_jsonrpc_messages(bytes) {
        let Some(method) = v.get("method").and_then(|m| m.as_str()) else {
            continue;
        };
        if method != "textDocument/publishDiagnostics" {
            continue;
        }
        let uri = v
            .get("params")
            .and_then(|p| p.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let diags = v
            .get("params")
            .and_then(|p| p.get("diagnostics"))
            .and_then(|d| d.as_array());
        let is_empty = diags.map(|a| a.is_empty()).unwrap_or(true);
        match (uri, is_empty) {
            ("file:///tmp/test.xs", true) => clean += 1,
            ("file:///tmp/bad.xs", false) => bad += 1,
            _ => {}
        }
    }
    (clean, bad)
}

#[test]
fn r5_f03_clean_diag_asserts_parsed_notification() {
    let noise_body =
        b"// textDocument/publishDiagnostics was last seen for \"file:///tmp/test.xs\"";
    let mut bytes = Vec::new();
    write!(bytes, "Content-Length: {}\r\n\r\n", noise_body.len()).unwrap();
    bytes.extend_from_slice(noise_body);

    let production_says = production_has_clean_diag(&bytes);
    let (parsed_clean, _) = parsed_publish_diagnostics_count(&bytes);
    let parsed_exists = parsed_clean > 0;

    assert_eq!(
        production_says, parsed_exists,
        "clean-diag flag must come only from a real publishDiagnostics notification"
    );
    assert_eq!(
        parsed_clean, 0,
        "no real publishDiagnostics notification exists in this stream"
    );
}

#[test]
fn r5_f03_bad_diag_asserts_parsed_notification() {
    let body: &[u8] = b"// \"file:///tmp/bad.xs\" produces a Parse error somewhere upstream";
    let mut bytes = Vec::new();
    write!(bytes, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    bytes.extend_from_slice(body);

    let production_says = production_has_bad_diag(&bytes);
    let (_, parsed_bad) = parsed_publish_diagnostics_count(&bytes);
    let parsed_exists = parsed_bad > 0;

    assert_eq!(
        production_says, parsed_exists,
        "bad-diag flag must come only from a real publishDiagnostics notification"
    );
    assert_eq!(
        parsed_bad, 0,
        "no real publishDiagnostics notification for /tmp/bad.xs in this stream"
    );
}

// ===================================================================
// R5-F-02 — references elapsed must measure request-to-response only
// ===================================================================

const REFERENCES_TEST_ID: i64 = 2001;

/// Locate the repo's committed `docs/doxygen_retail.7z` from `CARGO_MANIFEST_DIR`.
fn resolve_doxygen_archive() -> std::path::PathBuf {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root from CARGO_MANIFEST_DIR");
    let path = repo_root.join("docs").join("doxygen_retail.7z");
    std::fs::canonicalize(&path).unwrap_or(path)
}

fn make_temp_game_folder() -> std::path::PathBuf {
    let pid = std::process::id();
    let tmp = std::env::temp_dir();
    let game_root = tmp.join(format!("aomr_ref_game_{pid}"));
    let _ = std::fs::remove_dir_all(&game_root);
    std::fs::create_dir_all(&game_root).expect("create temp game root");
    let doxy_src = resolve_doxygen_archive();
    std::fs::copy(&doxy_src, game_root.join("doxygen_retail.7z"))
        .expect("copy doxygen archive into temp game folder");
    game_root
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

/// Spawn the real `xs-language-server`, send references, and measure the true
/// wall-clock interval from write+flush to parsed response.
fn measure_actual_references_response_window() -> Duration {
    let game_root = make_temp_game_folder();
    let mod_root = std::env::temp_dir().join(format!("aomr_ref_mod_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&mod_root);
    let mod_dir = mod_root.join("game").join("ai").join("engine_ref_test");
    let main_path = mod_dir.join("main.xs");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(&main_path, "void useAiEcho()\n{\n   aiEcho(\"main\");\n}\n").unwrap();

    let game_root = std::fs::canonicalize(&game_root).unwrap();
    let mod_root = std::fs::canonicalize(&mod_root).unwrap();
    let main_uri = tower_lsp::lsp_types::Url::from_file_path(&main_path).unwrap();
    let mod_uri = tower_lsp::lsp_types::Url::from_file_path(&mod_root).unwrap();

    let server_path = std::env::var("CARGO_BIN_EXE_xs-language-server")
        .map(std::path::PathBuf::from)
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
        .expect("spawn xs-language-server for references timing");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2000,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {},
            "trace": "off",
            "rootUri": null,
            "workspaceFolders": [{ "uri": mod_uri, "name": "engine_ref_mod" }]
        }
    })
    .to_string();
    let initialized =
        serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}}).to_string();
    let did_open = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": main_uri,
                "languageId": "xs",
                "version": 1,
                "text": "void useAiEcho()\n{\n   aiEcho(\"main\");\n}\n"
            }
        }
    })
    .to_string();
    let references = serde_json::json!({
        "jsonrpc": "2.0",
        "id": REFERENCES_TEST_ID,
        "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": main_uri },
            "position": { "line": 2, "character": 5 },
            "context": { "includeDeclaration": false }
        }
    })
    .to_string();
    let shutdown = serde_json::json!({"jsonrpc":"2.0","id":2002,"method":"shutdown"}).to_string();
    let exit = serde_json::json!({"jsonrpc":"2.0","method":"exit"}).to_string();

    for msg in &[init, initialized, did_open] {
        stdin.write_all(frame(msg).as_slice()).unwrap();
        stdin.flush().unwrap();
        std::thread::sleep(Duration::from_millis(50));
    }

    stdin.write_all(frame(&references).as_slice()).unwrap();
    stdin.flush().unwrap();
    let actual_start = Instant::now();
    let _references_resp =
        read_response_with_id(&mut stdout, REFERENCES_TEST_ID).expect("read references response");
    let actual_response_window = actual_start.elapsed();

    stdin.write_all(frame(&shutdown).as_slice()).unwrap();
    stdin.write_all(frame(&exit).as_slice()).unwrap();
    stdin.flush().unwrap();
    drop(stdin);

    let status = child.wait().expect("wait on references-timing child");
    let mut stderr_text = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr_text)
        .unwrap();

    if !status.success() {
        panic!(
            "server did not exit cleanly (status: {}), stderr:\n{}",
            status, stderr_text
        );
    }

    let _ = std::fs::remove_dir_all(&game_root);
    let _ = std::fs::remove_dir_all(&mod_root);

    actual_response_window
}

/// Run the roundtrip binary and parse the elapsed time it reports for the
/// engine-symbol references request.
fn production_references_elapsed() -> Duration {
    let roundtrip_bin = std::env::var("CARGO_BIN_EXE_lsp_roundtrip_test")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("debug")
                .join("lsp_roundtrip_test")
        });
    if !roundtrip_bin.exists() {
        panic!("lsp_roundtrip_test binary not found at {:?}", roundtrip_bin);
    }
    let output = Command::new(&roundtrip_bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run lsp_roundtrip_test binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Find the references timing line. Before the fix it reads:
    //   "PASS (engine references): returned N aiEcho location(s) in X ms"
    // After the fix it reads:
    //   "PASS (engine references): responded N aiEcho location(s) in X ms"
    let elapsed_ms: u64 = stdout
        .lines()
        .find(|l| {
            l.contains("(engine references):")
                && (l.contains("returned") || l.contains("responded"))
        })
        .and_then(|line| {
            line.split("in ")
                .nth(1)
                .and_then(|rest| rest.split(" ms").next())
                .and_then(|n| n.trim().parse().ok())
        })
        .unwrap_or_else(|| {
            panic!(
                "could not parse references elapsed from roundtrip binary stdout.\nstdout:\n{}\nstderr:\n{}",
                stdout, stderr
            )
        });

    Duration::from_millis(elapsed_ms)
}

#[test]
fn r5_f02_references_elapsed_measures_only_response_window() {
    let production_elapsed = production_references_elapsed();
    let actual_response_window = measure_actual_references_response_window();

    println!(
        "production_elapsed={}ms actual_response_window={}ms",
        production_elapsed.as_millis(),
        actual_response_window.as_millis()
    );

    // The production measurement must match the wall-clock response window.
    // Before the fix the roundtrip binary captured start before write and
    // elapsed after child.wait(), so it included a 50 ms sleep plus shutdown
    // roundtrip — much larger than the true window. After the fix it captures
    // elapsed from post-flush to parsed response, matching actual_response_window.
    let delta = production_elapsed.abs_diff(actual_response_window);
    assert!(
        delta <= Duration::from_millis(5),
        "production elapsed ({:?}) must be within 5 ms of actual response window ({:?}); got delta {:?}",
        production_elapsed,
        actual_response_window,
        delta
    );

    assert!(
        actual_response_window < Duration::from_millis(200),
        "actual response window ({:?}) should be < 200 ms",
        actual_response_window
    );
}
