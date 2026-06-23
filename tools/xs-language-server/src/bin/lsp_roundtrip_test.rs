//! Day 3 LSP round-trip test.
//!
//! Spawns the `xs-language-server` binary, sends an LSP message sequence
//! (initialize → initialized → didOpen → didChange → shutdown → exit),
//! and verifies the server responds correctly.
//!
//! Why we send `exit` BEFORE reading: `tokio::io::stdout()` buffers writes
//! and only flushes when the server exits. So we let the server process
//! everything and exit, then read all responses from the drained pipe.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

// Reference LSP messages. Kept as constants so the framing and the body
// can never drift apart at the test site.
const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"capabilities":{},"trace":"off","rootUri":null,"workspaceFolders":null}}"#;
const INITIALIZED: &str =
    r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
const DID_OPEN: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/test.xs","languageId":"xs","version":1,"text":"rule test\nminInterval 5\nactive\n{\n   aiEcho(\"hello\");\n}"}}}"#;
const DID_CHANGE: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///tmp/test.xs","version":2},"contentChanges":[{"text":"rule test2\nactive\n{\n   aiEcho(\"changed\");\n}"}]}}"#;
const SHUTDOWN: &str = r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#;
const EXIT: &str = r#"{"jsonrpc":"2.0","method":"exit"}"#;

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn read_framed_message<R: Read>(reader: &mut R) -> std::io::Result<serde_json::Value> {
    // Read one byte at a time so we don't advance the reader past data we
    // haven't parsed yet. (With Cursor, `read(&mut [4096])` consumes up to
    // 4096 bytes per call even if we only use a handful, which breaks
    // multi-message parsing.)
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
        if accumulated.len() >= 4
            && &accumulated[accumulated.len() - 4..] == b"\r\n\r\n"
        {
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

    serde_json::from_slice(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Read messages until we get one whose JSON has an `"id"` field equal to the
/// expected id. Notifications (no id) are skipped. Returns that response.
fn read_response_with_id<R: Read>(
    reader: &mut R,
    expected_id: i64,
) -> std::io::Result<serde_json::Value> {
    loop {
        let msg = read_framed_message(reader)?;
        eprintln!("[test] received msg: {}", msg);
        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
            if id == expected_id {
                return Ok(msg);
            }
        }
        // Otherwise it's a notification; skip and read next.
    }
}

fn locate_server_binary() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("xs-language-server")))
        .expect("could not derive target/debug/ from current_exe()")
}

fn main() {
    let server_path = locate_server_binary();

    let mut child = Command::new(&server_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(
            std::fs::File::create("/tmp/xs_lsp_server_stderr.log")
                .expect("create stderr log"),
        ))
        .spawn()
        .expect("spawn xs-language-server");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    // Write the full message sequence (including exit) so the server
    // processes everything and exits, flushing stdout.
    for msg in &[INITIALIZE, INITIALIZED, DID_OPEN, DID_CHANGE, SHUTDOWN, EXIT] {
        eprintln!("[test] writing {} bytes", frame(msg).len());
        stdin
            .write_all(frame(msg).as_bytes())
            .expect("write LSP frame to stdin");
        stdin.flush().expect("flush stdin");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    drop(stdin);
    eprintln!("[test] all messages sent; waiting on child");

    let status = child.wait().expect("wait on child");
    eprintln!("[test] child exited with status {:?}", status);

    // Now read the responses. The pipe buffer holds all data the server wrote.
    let mut all_bytes = Vec::new();
    let mut init_resp = None;
    let mut shutdown_resp = None;

    // Read everything from stdout, then parse.
    stdout.read_to_end(&mut all_bytes).expect("read stdout to end");
    eprintln!("[test] read {} bytes total from stdout", all_bytes.len());
    eprintln!("[test] raw bytes: {:?}", String::from_utf8_lossy(&all_bytes));

    // Parse each message and pick the ones we want.
    let mut cursor = std::io::Cursor::new(&all_bytes);
    loop {
        match read_framed_message(&mut cursor) {
            Ok(msg) => {
                eprintln!("[test] parsed msg: id={:?}, method={:?}",
                    msg.get("id"), msg.get("method"));
                if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
                    if id == 1 && init_resp.is_none() {
                        init_resp = Some(msg);
                    } else if id == 2 && shutdown_resp.is_none() {
                        shutdown_resp = Some(msg);
                    }
                }
            }
            Err(e) => {
                eprintln!("[test] parse failed: {}", e);
                break;
            }
        }
    }
    eprintln!("[test] init_resp: {}, shutdown_resp: {}",
        init_resp.is_some(), shutdown_resp.is_some());

    let init_resp = init_resp.expect("initialize response");
    eprintln!("[test] got initialize response");
    let shutdown_resp = shutdown_resp.expect("shutdown response");
    eprintln!("[test] got shutdown response");

    let stderr_text = std::fs::read_to_string("/tmp/xs_lsp_server_stderr.log")
        .unwrap_or_else(|e| format!("(failed to read stderr log: {e})"));

    let mut all_pass = true;

    if status.success() {
        println!("PASS: server exited cleanly (status: {})", status);
    } else {
        println!("FAIL: server did not exit cleanly (status: {})", status);
        all_pass = false;
    }

    let init_ok = init_resp.get("id").and_then(|i| i.as_i64()) == Some(1)
        && init_resp.get("result").and_then(|r| r.get("capabilities")).is_some();
    if init_ok {
        println!("PASS: initialize response received");
    } else {
        println!("FAIL: initialize response missing or malformed:");
        println!("  {}", init_resp);
        all_pass = false;
    }

    let shutdown_ok = shutdown_resp.get("id").and_then(|i| i.as_i64()) == Some(2)
        && (shutdown_resp.get("result").is_some() || shutdown_resp.get("error").is_some());
    if shutdown_ok {
        println!("PASS: shutdown response received");
    } else {
        println!("FAIL: shutdown response missing or malformed:");
        println!("  {}", shutdown_resp);
        all_pass = false;
    }

    let stderr_trimmed = stderr_text.trim();
    let has_panic_signal = stderr_trimmed
        .lines()
        .any(|line| line.contains("panic") || line.contains("FATAL"));

    if stderr_trimmed.is_empty() {
        println!("PASS: no stderr panic (stderr empty)");
    } else if !has_panic_signal {
        println!("PASS: no stderr panic (tracing output only)");
        println!("--- server stderr ---");
        for line in stderr_trimmed.lines() {
            println!("  {line}");
        }
        println!("--- end server stderr ---");
    } else {
        println!("FAIL: stderr contains panic/FATAL signal:");
        for line in stderr_trimmed.lines() {
            println!("  {line}");
        }
        all_pass = false;
    }

    println!(
        "sample initialize response (first 200 chars): {}",
        init_resp.to_string().chars().take(200).collect::<String>()
    );

    if all_pass {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}