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

use serde_json::json;
use tower_lsp::lsp_types::Url;

// Reference LSP messages. Kept as constants so the framing and the body
// can never drift apart at the test site.
const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"capabilities":{},"trace":"off","rootUri":null,"workspaceFolders":null}}"#;
const INITIALIZED: &str =
    r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
const DID_OPEN: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/test.xs","languageId":"xs","version":1,"text":"rule test\nminInterval 5\nactive\n{\n   aiEcho(\"hello\");\n}"}}}"#;
const DID_CHANGE: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///tmp/test.xs","version":2},"contentChanges":[{"text":"rule test2\nactive\n{\n   aiEcho(\"changed\");\n}"}]}}"#;
const DID_OPEN_BAD: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/bad.xs","languageId":"xs","version":1,"text":"rule brokenRule\nminInterval 5\nactive\n{\n   int x = ;\n   aiEcho(\"hello\n}\n\nvoid unclosed(int a\n{\n}\n"}}}"#;
const COMPLETION_AI: &str = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///tmp/test.xs"},"position":{"line":4,"character":6}}}"#;
// Hover and go-to-definition at the same position — col 6 is on the 'c'
// of "aiEcho" in the test file (line 4 = `   aiEcho("hello");`).
const HOVER_AI: &str = r#"{"jsonrpc":"2.0","id":4,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tmp/test.xs"},"position":{"line":4,"character":6}}}"#;
const DEFINITION_AI: &str = r#"{"jsonrpc":"2.0","id":5,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///tmp/test.xs"},"position":{"line":4,"character":6}}}"#;
// Week 3: open a SECOND file with workspace symbols (a rule, a function,
// a constant). Hover/definition on the workspace symbol should resolve to
// a real file:line location in the same file (not the xs-stub:// virtual
// URI used for engine API).
//
// File content:
//   1: rule doStuff
//   2: minInterval 5
//   3: active
//   4: {
//   5:    aiEcho("called");
//   6: }
//   7:
//   8: int helper(int a, int b)
//   9: {
//   10:    return a + b;
//   11: }
//   12:
//   13: const int cMagic = 42;
//
// Hover at line 7 col 6 is on 'h' in `helper`. Definition at the same
// place should jump to line 7 col 5.
const DID_OPEN_WORKSPACE: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/workspace.xs","languageId":"xs","version":1,"text":"rule doStuff\nminInterval 5\nactive\n{\n   aiEcho(\"called\");\n}\n\nint helper(int a, int b)\n{\n   return a + b;\n}\n\nconst int cMagic = 42;\n"}}}"#;
const HOVER_WORKSPACE: &str = r#"{"jsonrpc":"2.0","id":6,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tmp/workspace.xs"},"position":{"line":7,"character":6}}}"#;
const DEFINITION_WORKSPACE: &str = r#"{"jsonrpc":"2.0","id":7,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///tmp/workspace.xs"},"position":{"line":7,"character":6}}}"#;
const HOVER_CONSTANT: &str = r#"{"jsonrpc":"2.0","id":8,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///tmp/workspace.xs"},"position":{"line":12,"character":11}}}"#;
const DOCUMENT_SYMBOL: &str = r#"{"jsonrpc":"2.0","id":9,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///tmp/workspace.xs"}}}"#;
// Week 4: refs.xs has `helper` declared once and called twice (3 total
// occurrences). Line 7 col 5 is on the 'l' of `helper` inside `   helper(1);`.
const DID_OPEN_REFS: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/refs.xs","languageId":"xs","version":1,"text":"int helper(int a)\n{\n   return a;\n}\n\nvoid caller()\n{\n   helper(1);\n   helper(2);\n}\n"}}}"#;
const REFERENCES: &str = r#"{"jsonrpc":"2.0","id":10,"method":"textDocument/references","params":{"textDocument":{"uri":"file:///tmp/refs.xs"},"position":{"line":7,"character":5},"context":{"includeDeclaration":true}}}"#;
const RENAME: &str = r#"{"jsonrpc":"2.0","id":11,"method":"textDocument/rename","params":{"textDocument":{"uri":"file:///tmp/refs.xs"},"position":{"line":7,"character":5},"newName":"helperRenamed"}}"#;
const PREPARE_RENAME: &str = r#"{"jsonrpc":"2.0","id":12,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":"file:///tmp/refs.xs"},"position":{"line":7,"character":5}}}"#;
// prepareRename on the engine-API symbol `aiEcho` in /tmp/test.xs (line 4
// col 6 is on 'c'). Must return null.
const PREPARE_RENAME_ENGINE: &str = r#"{"jsonrpc":"2.0","id":13,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":"file:///tmp/test.xs"},"position":{"line":4,"character":6}}}"#;
// Week 5: types.xs contains 2 valid + 2 invalid engine API calls.
//   Line 0: void test()
//   Line 1: {
//   Line 2:    aiEcho("valid");                  <-- ok
//   Line 3:    aiEchoCategory(0, "warning");    <-- ok
//   Line 4:    aiEcho("too", "many");            <-- arg count error (1 expected)
//   Line 5:    aiEcho(42);                       <-- arg type error (expected string, got int)
//   Line 6: }
const DID_OPEN_TYPES: &str = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/types.xs","languageId":"xs","version":1,"text":"void test()\n{\n   aiEcho(\"valid\");\n   aiEchoCategory(0, \"warning\");\n   aiEcho(\"too\", \"many\");\n   aiEcho(42);\n}\n"}}}"#;
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

/// Derive the repository root from the crate manifest dir
/// (`tools/xs-language-server/` -> repo root) so we can pass `docs/` as the
/// game folder. `docs/doxygen_retail.7z` is committed in the repo.
fn resolve_test_game_path() -> std::path::PathBuf {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // tools/xs-language-server -> tools/ -> aom_retold_mod/
    let repo_root = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root from CARGO_MANIFEST_DIR");
    let path = repo_root.join("docs");
    std::fs::canonicalize(&path).unwrap_or(path)
}

/// Run the original Phase 1 message sequence unchanged.
fn run_baseline() -> bool {
    let server_path = locate_server_binary();
    let game_path = resolve_test_game_path();

    let mut child = Command::new(&server_path)
        .arg("--game-path")
        .arg(&game_path)
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
    for msg in &[INITIALIZE, INITIALIZED, DID_OPEN, DID_OPEN_BAD, COMPLETION_AI, HOVER_AI, DEFINITION_AI, DID_OPEN_WORKSPACE, HOVER_WORKSPACE, DEFINITION_WORKSPACE, HOVER_CONSTANT, DOCUMENT_SYMBOL, DID_OPEN_REFS, REFERENCES, RENAME, PREPARE_RENAME, PREPARE_RENAME_ENGINE, DID_OPEN_TYPES, SHUTDOWN, EXIT] {
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
    let mut completion_resp = None;
    let mut hover_resp = None;
    let mut definition_resp = None;
    let mut hover_workspace_resp = None;
    let mut definition_workspace_resp = None;
    let mut hover_constant_resp = None;
    let mut document_symbol_resp = None;
    let mut references_resp = None;
    let mut rename_resp = None;
    let mut prepare_rename_resp = None;
    let mut prepare_rename_engine_resp = None;
    let mut types_diag_raw: Option<String> = None;

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
                    } else if id == 3 && completion_resp.is_none() {
                        completion_resp = Some(msg);
                    } else if id == 4 && hover_resp.is_none() {
                        hover_resp = Some(msg);
                    } else if id == 5 && definition_resp.is_none() {
                        definition_resp = Some(msg);
                    } else if id == 6 && hover_workspace_resp.is_none() {
                        hover_workspace_resp = Some(msg);
                    } else if id == 7 && definition_workspace_resp.is_none() {
                        definition_workspace_resp = Some(msg);
                    } else if id == 8 && hover_constant_resp.is_none() {
                        hover_constant_resp = Some(msg);
                    } else if id == 9 && document_symbol_resp.is_none() {
                        document_symbol_resp = Some(msg);
                    } else if id == 10 && references_resp.is_none() {
                        references_resp = Some(msg);
                    } else if id == 11 && rename_resp.is_none() {
                        rename_resp = Some(msg);
                    } else if id == 12 && prepare_rename_resp.is_none() {
                        prepare_rename_resp = Some(msg);
                    } else if id == 13 && prepare_rename_engine_resp.is_none() {
                        prepare_rename_engine_resp = Some(msg);
                    }
                } else if msg.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                {
                    // Week 5: capture the publishDiagnostics notification
                    // for /tmp/types.xs so we can assert on the typecheck
                    // output.
                    let uri = msg
                        .get("params")
                        .and_then(|p| p.get("uri"))
                        .and_then(|u| u.as_str());
                    if uri == Some("file:///tmp/types.xs") && types_diag_raw.is_none() {
                        types_diag_raw = Some(msg.to_string());
                    }
                }
            }
            Err(e) => {
                eprintln!("[test] parse failed: {}", e);
                break;
            }
        }
    }
    eprintln!("[test] init_resp: {}, shutdown_resp: {}, completion_resp: {}, hover_resp: {}, definition_resp: {}, hover_workspace: {}, definition_workspace: {}, hover_constant: {}, document_symbol: {}, references: {}, rename: {}, prepare_rename: {}, prepare_rename_engine: {}",
        init_resp.is_some(), shutdown_resp.is_some(), completion_resp.is_some(),
        hover_resp.is_some(), definition_resp.is_some(),
        hover_workspace_resp.is_some(), definition_workspace_resp.is_some(),
        hover_constant_resp.is_some(), document_symbol_resp.is_some(),
        references_resp.is_some(), rename_resp.is_some(),
        prepare_rename_resp.is_some(), prepare_rename_engine_resp.is_some());

    let init_resp = init_resp.expect("initialize response");
    eprintln!("[test] got initialize response");
    let shutdown_resp = shutdown_resp.expect("shutdown response");
    eprintln!("[test] got shutdown response");
    let completion_resp = completion_resp.expect("completion response");
    eprintln!("[test] got completion response");
    let hover_resp = hover_resp.expect("hover response");
    eprintln!("[test] got hover response");
    let definition_resp = definition_resp.expect("definition response");
    eprintln!("[test] got definition response");
    let hover_workspace_resp = hover_workspace_resp.expect("hover workspace response");
    eprintln!("[test] got hover workspace response");
    let definition_workspace_resp = definition_workspace_resp.expect("definition workspace response");
    eprintln!("[test] got definition workspace response");
    let hover_constant_resp = hover_constant_resp.expect("hover constant response");
    eprintln!("[test] got hover constant response");
    let document_symbol_resp = document_symbol_resp.expect("document symbol response");
    eprintln!("[test] got document symbol response");
    let references_resp = references_resp.expect("references response");
    eprintln!("[test] got references response");
    let rename_resp = rename_resp.expect("rename response");
    eprintln!("[test] got rename response");
    let prepare_rename_resp = prepare_rename_resp.expect("prepare rename response");
    eprintln!("[test] got prepare rename response");
    let prepare_rename_engine_resp = prepare_rename_engine_resp.expect("prepare rename engine response");
    eprintln!("[test] got prepare rename engine response");

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

    // Look for at least one publishDiagnostics notification in the raw bytes.
    // (We parsed only id-tagged messages above; notifications don't have ids
    // so they were skipped. The raw bytes should still contain them.)
    let raw = String::from_utf8_lossy(&all_bytes);
    let has_clean_diag = raw.contains("\"file:///tmp/test.xs\"")
        && raw.contains("textDocument/publishDiagnostics");
    let has_bad_diag = raw.contains("\"file:///tmp/bad.xs\"")
        && raw.contains("Parse error");

    if has_clean_diag {
        println!("PASS: clean-file diagnostics published (expected empty)");
    } else {
        println!("FAIL: missing diagnostics notification for /tmp/test.xs");
        all_pass = false;
    }

    if has_bad_diag {
        println!("PASS: malformed-file diagnostics published with parse errors");
    } else {
        println!("FAIL: missing diagnostics notification for /tmp/bad.xs");
        all_pass = false;
    }

    // Completion: the request asked for items at line 4 col 13 in /tmp/test.xs,
    // which is after `aiEcho("hel` (the partial token at that position is "aiE"
    // since the file content's line 4 reads "   aiEcho(\"hello\");", col 13 is
    // around the `c` in "aiEcho"). Look for the expected engine-API matches.
    let raw = String::from_utf8_lossy(&all_bytes);
    let completion_count = raw.matches("\"aiEcho\"").count()
        + raw.matches("\"aiEchoCategory\"").count()
        + raw.matches("\"aiEchoWarning\"").count();
    if completion_count >= 3 {
        println!("PASS: completion returned aiEcho family items ({completion_count} occurrences)");
    } else {
        println!("FAIL: completion did not return expected aiEcho* items (count={completion_count})");
        println!("  completion response: {}", completion_resp);
        all_pass = false;
    }

    // Hover: the response should contain a `contents.value` field with
    // "aiEcho" (the identifier name), a Markdown code fence (the signature),
    // AND a recognizable XS return type or primitive type name.
    let hover_raw = hover_resp.to_string();
    let has_hover_name = hover_raw.contains("aiEcho");
    let has_hover_signature = hover_raw.contains("```xs");
    // Accept any common XS type token as a proxy for "the signature rendered
    // something". aiEcho's actual signature is `void aiEcho(string text)`.
    let has_hover_type = hover_raw.contains("void ")
        || hover_raw.contains("int ")
        || hover_raw.contains("string ")
        || hover_raw.contains("bool ")
        || hover_raw.contains("float ")
        || hover_raw.contains("vector ");
    if has_hover_name && has_hover_signature && has_hover_type {
        println!("PASS: hover returned aiEcho signature (Markdown with name + code fence + type token)");
    } else {
        println!("FAIL: hover did not return expected content (name={has_hover_name}, fence={has_hover_signature}, type={has_hover_type})");
        println!("  hover response: {}", hover_resp);
        all_pass = false;
    }

    // Definition on an engine-API symbol (`aiEcho`): should return null
    // because engine symbols have no source location.
    let def_result = definition_resp.get("result");
    let is_null = def_result.map_or(false, |v| v.is_null());
    let no_stub_uri = !definition_resp.to_string().contains("xs-stub://");
    if is_null && no_stub_uri {
        println!("PASS: definition returned null for engine-API symbol (aiEcho)");
    } else {
        println!("FAIL: definition did not return null for engine-API symbol aiEcho (is_null={is_null}, no_stub={no_stub_uri})");
        println!("  definition response: {}", definition_resp);
        all_pass = false;
    }

    // Hover on a workspace function (`helper`): should return workspace
    // hover (not engine API), with the function name, kind, and detail.
    let hover_workspace_raw = hover_workspace_resp.to_string();
    let has_workspace_name = hover_workspace_raw.contains("helper");
    let has_workspace_kind = hover_workspace_raw.contains("function");
    let has_workspace_detail = hover_workspace_raw.contains("int helper")
        || hover_workspace_raw.contains("helper(int a, int b)");
    if has_workspace_name && has_workspace_kind && has_workspace_detail {
        println!("PASS: hover returned workspace function (helper with kind/detail)");
    } else {
        println!("FAIL: hover workspace did not return expected content (name={has_workspace_name}, kind={has_workspace_kind}, detail={has_workspace_detail})");
        println!("  hover workspace response: {}", hover_workspace_resp);
        all_pass = false;
    }

    // Definition on a workspace function (`helper`): should return a
    // Location pointing to the REAL file, NOT a xs-stub:// virtual URI.
    let def_workspace_raw = definition_workspace_resp.to_string();
    let has_real_file = def_workspace_raw.contains("file:///tmp/workspace.xs");
    let no_stub_uri = !def_workspace_raw.contains("xs-stub://");
    // The function is defined at line 7 col 5 (0-indexed line 7 = "int helper").
    let has_correct_line = def_workspace_raw.contains("\"line\":7")
        || def_workspace_raw.contains("\"line\": 7");
    if has_real_file && no_stub_uri && has_correct_line {
        println!("PASS: definition returned real file:line for workspace function");
    } else {
        println!("FAIL: definition workspace did not return real location (file={has_real_file}, no_stub={no_stub_uri}, line={has_correct_line})");
        println!("  definition workspace response: {}", definition_workspace_resp);
        all_pass = false;
    }

    // Hover on a workspace CONSTANT (`cMagic` at line 12 col 11).
    let hover_constant_raw = hover_constant_resp.to_string();
    let has_constant_name = hover_constant_raw.contains("cMagic");
    let has_constant_kind = hover_constant_raw.contains("constant");
    let has_constant_detail = hover_constant_raw.contains("42");
    if has_constant_name && has_constant_kind && has_constant_detail {
        println!("PASS: hover returned workspace constant (cMagic with kind/value)");
    } else {
        println!("FAIL: hover constant did not return expected content (name={has_constant_name}, kind={has_constant_kind}, detail={has_constant_detail})");
        println!("  hover constant response: {}", hover_constant_resp);
        all_pass = false;
    }

    // Document symbol outline: should return >= 3 symbols (rule, function,
    // constant). Each symbol is a JSON object with `name` and `kind`.
    let doc_sym_raw = document_symbol_resp.to_string();
    let sym_count_rule = doc_sym_raw.matches("\"name\":\"doStuff\"").count();
    let sym_count_func = doc_sym_raw.matches("\"name\":\"helper\"").count();
    let sym_count_const = doc_sym_raw.matches("\"name\":\"cMagic\"").count();
    if sym_count_rule >= 1 && sym_count_func >= 1 && sym_count_const >= 1 {
        println!("PASS: document symbol returned 3 symbols (rule={sym_count_rule}, function={sym_count_func}, constant={sym_count_const})");
    } else {
        println!("FAIL: document symbol did not return all 3 expected symbols (rule={sym_count_rule}, func={sym_count_func}, const={sym_count_const})");
        println!("  document symbol response: {}", document_symbol_resp);
        all_pass = false;
    }

    // References: helper is declared once and called twice = 3 occurrences.
    // The response is an array of Locations, each with a `uri` field.
    let references_raw = references_resp.to_string();
    let refs_uri_count = references_raw.matches("\"uri\":\"file:///tmp/refs.xs\"").count()
        + references_raw.matches("\"uri\": \"file:///tmp/refs.xs\"").count();
    if refs_uri_count >= 3 {
        println!("PASS: references returned 3+ locations for helper (count={refs_uri_count})");
    } else {
        println!("FAIL: references did not return expected locations (count={refs_uri_count})");
        println!("  references response: {}", references_resp);
        all_pass = false;
    }

    // Rename: WorkspaceEdit with `changes` map containing 3 TextEdits,
    // each replacing "helper" with "helperRenamed".
    let rename_raw = rename_resp.to_string();
    let rename_edits_count = rename_raw.matches("\"helperRenamed\"").count();
    let has_changes_field = rename_raw.contains("\"changes\"");
    if rename_edits_count >= 3 && has_changes_field {
        println!("PASS: rename returned WorkspaceEdit with 3+ edits (count={rename_edits_count}, has_changes={has_changes_field})");
    } else {
        println!("FAIL: rename did not return expected WorkspaceEdit (count={rename_edits_count}, has_changes={has_changes_field})");
        println!("  rename response: {}", rename_resp);
        all_pass = false;
    }

    // prepareRename on a workspace symbol: should return a Range, which
    // serializes as `{start: {...}, end: {...}}` (PrepareRenameResponse::Range
    // is `#[serde(untagged)]`-ish — the variant tag is dropped).
    let prepare_rename_raw = prepare_rename_resp.to_string();
    let has_start = prepare_rename_raw.contains("\"start\"")
        && prepare_rename_raw.contains("\"end\"")
        && prepare_rename_raw.contains("\"line\":7");
    let is_not_null = !prepare_rename_raw.contains("\"result\":null")
        && !prepare_rename_raw.contains("\"result\": null");
    if has_start && is_not_null {
        println!("PASS: prepareRename returned Range for workspace symbol (line=7)");
    } else {
        println!("FAIL: prepareRename did not return Range (has_start={has_start}, is_not_null={is_not_null})");
        println!("  prepare rename response: {}", prepare_rename_resp);
        all_pass = false;
    }

    // prepareRename on an engine-API symbol: should return null.
    let prepare_rename_engine_raw = prepare_rename_engine_resp.to_string();
    let has_id_13 = prepare_rename_engine_raw.contains("\"id\":13")
        || prepare_rename_engine_raw.contains("\"id\": 13");
    let has_null_result = prepare_rename_engine_raw.contains("\"result\":null")
        || prepare_rename_engine_raw.contains("\"result\": null");
    if has_id_13 && has_null_result {
        println!("PASS: prepareRename returned null for engine-API symbol (aiEcho)");
    } else {
        println!("FAIL: prepareRename did not return null for engine-API symbol (id13={has_id_13}, null={has_null_result})");
        println!("  prepare rename engine response: {}", prepare_rename_engine_resp);
        all_pass = false;
    }

    // Week 5: type checking. /tmp/types.xs has 2 valid + 2 invalid calls.
    //   * aiEcho("valid")            -- ok
    //   * aiEchoCategory(0, "warning")-- ok
    //   * aiEcho("too", "many")      -- 1 expected, 2 got  (count error)
    //   * aiEcho(42)                 -- expected string, got int (type error)
    let types_diag_raw = types_diag_raw.clone().unwrap_or_default();
    let has_count_err = types_diag_raw.contains("expected 1 argument(s) to `aiEcho`");
    let has_type_err = types_diag_raw
        .contains("expected argument 1 of type `string` for `aiEcho`, got `int`");
    // The valid calls should NOT produce type errors. The "got `string`" case
    // would be a false positive, so verify it's absent.
    let has_no_err_on_valid_calls = !types_diag_raw
        .contains("expected argument 1 of type `string` for `aiEcho`, got `string`");
    if has_count_err && has_type_err && has_no_err_on_valid_calls {
        println!("PASS: typecheck flagged wrong count + wrong type on /tmp/types.xs");
    } else {
        println!(
            "FAIL: typecheck incomplete (count_err={has_count_err}, type_err={has_type_err}, no_false_positive={has_no_err_on_valid_calls})"
        );
        println!("  raw diagnostics: {types_diag_raw}");
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

    all_pass
}

/// Phase 2: test virtual-project ownership, file replacement, include
/// resolution scoping, watched-file registration, and dynamic workspace-folder
/// add/remove.
fn run_workspace_tests() -> bool {
    let pid = std::process::id();
    let tmp = std::env::temp_dir();

    let game_root = tmp.join(format!("aomr_lsp_game_{pid}"));
    let mod_a = tmp.join(format!("aomr_lsp_mod_a_{pid}"));
    let mod_b = tmp.join(format!("aomr_lsp_mod_b_{pid}"));
    let unowned = tmp.join(format!("aomr_lsp_unowned_{pid}.xs"));

    // Clean up any leftovers from previous runs.
    let _ = std::fs::remove_dir_all(&game_root);
    let _ = std::fs::remove_dir_all(&mod_a);
    let _ = std::fs::remove_dir_all(&mod_b);
    let _ = std::fs::remove_file(&unowned);

    // Copy the committed doxygen archive into the synthetic game folder.
    let doxy_src = resolve_test_game_path().join("doxygen_retail.7z");
    std::fs::create_dir_all(&game_root).expect("create game root");
    std::fs::copy(&doxy_src, game_root.join("doxygen_retail.7z"))
        .expect("copy doxygen archive");

    // Vanilla game files.
    let vanilla_core = game_root.join("game").join("ai").join("core").join("core.xs");
    std::fs::create_dir_all(vanilla_core.parent().unwrap()).unwrap();
    std::fs::write(&vanilla_core, "void vanillaCore() {}\n").unwrap();

    // Mod A overlay: shadows the vanilla core and provides a main file that
    // includes it.
    let mod_a_main = mod_a
        .join("game")
        .join("ai")
        .join("human_assist")
        .join("human_assist.xs");
    let mod_a_core = mod_a.join("game").join("ai").join("core").join("core.xs");
    std::fs::create_dir_all(mod_a_main.parent().unwrap()).unwrap();
    std::fs::create_dir_all(mod_a_core.parent().unwrap()).unwrap();
    std::fs::write(
        &mod_a_main,
        "include \"core/core.xs\"\n\nvoid mainAssistance() { modCore(); }\n",
    )
    .unwrap();
    std::fs::write(&mod_a_core, "void modCore() {}\n").unwrap();

    // Mod B overlay: a simple file used to test dynamic add.
    let mod_b_file = mod_b.join("game").join("ai").join("foo.xs");
    std::fs::create_dir_all(mod_b_file.parent().unwrap()).unwrap();
    std::fs::write(&mod_b_file, "void modBFoo() {}\n").unwrap();

    std::fs::write(&unowned, "void orphan() {}\n").unwrap();

    let game_root = std::fs::canonicalize(&game_root).unwrap();
    let mod_a = std::fs::canonicalize(&mod_a).unwrap();
    let mod_b = std::fs::canonicalize(&mod_b).unwrap();
    let unowned = std::fs::canonicalize(&unowned).unwrap();

    let mod_a_uri = Url::from_file_path(&mod_a).unwrap();
    let mod_b_uri = Url::from_file_path(&mod_b).unwrap();
    let mod_a_main_uri = Url::from_file_path(&mod_a_main).unwrap();
    let mod_b_file_uri = Url::from_file_path(&mod_b_file).unwrap();
    let unowned_uri = Url::from_file_path(&unowned).unwrap();
    let vanilla_core_uri = Url::from_file_path(&vanilla_core).unwrap();

    let server_path = locate_server_binary();

    let mut child = Command::new(&server_path)
        .arg("--game-path")
        .arg(&game_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xs-language-server for workspace tests");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    let with_uri = |uri: &Url| uri.to_string();

    let init = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {
                "workspace": {
                    "workspaceFolders": true,
                    "didChangeWatchedFiles": { "dynamicRegistration": true }
                }
            },
            "trace": "off",
            "rootUri": null,
            "workspaceFolders": [{ "uri": mod_a_uri, "name": "mod_a" }]
        }
    })
    .to_string();

    let initialized = json!({"jsonrpc":"2.0","method":"initialized","params":{}}).to_string();

    let did_open_unowned = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": unowned_uri,
                "languageId": "xs",
                "version": 1,
                "text": "void orphan() {}\n"
            }
        }
    })
    .to_string();

    let did_open_mod_a = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": mod_a_main_uri,
                "languageId": "xs",
                "version": 1,
                "text": "include \"core/core.xs\"\n\nvoid mainAssistance() { modCore(); }\n"
            }
        }
    })
    .to_string();

    let workspace_symbol_mod_a = json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "workspace/symbol",
        "params": { "query": "Core" }
    })
    .to_string();

    let watched_change = json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWatchedFiles",
        "params": {
            "changes": [{
                "uri": vanilla_core_uri,
                "type": 2 // Changed
            }]
        }
    })
    .to_string();

    let change_folders = json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWorkspaceFolders",
        "params": {
            "event": {
                "added": [{ "uri": mod_b_uri, "name": "mod_b" }],
                "removed": []
            }
        }
    })
    .to_string();

    let did_open_mod_b = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": mod_b_file_uri,
                "languageId": "xs",
                "version": 1,
                "text": "void modBFoo() {}\n"
            }
        }
    })
    .to_string();

    let workspace_symbol_mod_b = json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "workspace/symbol",
        "params": { "query": "foo" }
    })
    .to_string();

    let shutdown = json!({"jsonrpc":"2.0","id":103,"method":"shutdown"}).to_string();
    let exit = json!({"jsonrpc":"2.0","method":"exit"}).to_string();

    for msg in &[
        init,
        initialized,
        did_open_unowned,
        did_open_mod_a,
        workspace_symbol_mod_a,
        watched_change,
        change_folders,
        did_open_mod_b,
        workspace_symbol_mod_b,
        shutdown,
        exit,
    ] {
        stdin.write_all(frame(msg).as_bytes()).unwrap();
        stdin.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    drop(stdin);

    let status = child.wait().expect("wait on workspace-test child");
    let mut stderr_text = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr_text)
        .unwrap();

    let mut all_bytes = Vec::new();
    stdout.read_to_end(&mut all_bytes).unwrap();
    let raw = String::from_utf8_lossy(&all_bytes);

    let mut all_pass = true;

    if status.success() {
        println!("PASS (workspace): server exited cleanly");
    } else {
        println!("FAIL (workspace): server exited with {}", status);
        all_pass = false;
    }

    // The server should have requested dynamic watched-file registration.
    if raw.contains("client/registerCapability") && raw.contains("workspace/didChangeWatchedFiles") {
        println!("PASS (workspace): server registered didChangeWatchedFiles watcher");
    } else {
        println!("FAIL (workspace): missing client/registerCapability for watched files");
        all_pass = false;
    }

    // An unowned file should trigger the engine-API-only warning.
    if raw.contains("window/showMessage")
        && raw.contains("File not part of any registered mod")
    {
        println!("PASS (workspace): unowned file warning emitted");
    } else {
        println!("FAIL (workspace): missing unowned-file warning");
        all_pass = false;
    }

    // A file inside mod_a should receive diagnostics.
    if raw.contains(&with_uri(&mod_a_main_uri))
        && raw.contains("textDocument/publishDiagnostics")
    {
        println!("PASS (workspace): diagnostics published for mod_a file");
    } else {
        println!("FAIL (workspace): missing diagnostics for mod_a file");
        all_pass = false;
    }

    // workspace/symbol scoped to mod_a should expose modCore (from the mod's
    // shadow core.xs) and NOT vanillaCore from the game folder.
    let symbol_resp = read_response_with_id(&mut std::io::Cursor::new(&all_bytes), 101).ok();
    let symbol_raw = symbol_resp.map(|v| v.to_string()).unwrap_or_default();
    if symbol_raw.contains("modCore") && !symbol_raw.contains("vanillaCore") {
        println!("PASS (workspace): workspace symbol scoped to mod_a shows overlay, not vanilla");
    } else {
        println!("FAIL (workspace): workspace symbol did not respect overlay (resp={})", symbol_raw);
        all_pass = false;
    }

    // After adding mod_b and opening its file, there should be diagnostics for
    // it and no additional unowned-file warnings for it.
    if raw.contains(&with_uri(&mod_b_file_uri)) && raw.contains("textDocument/publishDiagnostics")
    {
        println!("PASS (workspace): mod_b file diagnosed after didChangeWorkspaceFolders add");
    } else {
        println!("FAIL (workspace): mod_b file not diagnosed after add");
        all_pass = false;
    }

    // workspace/symbol scoped to mod_b should find modBFoo.
    let symbol_resp_b = read_response_with_id(&mut std::io::Cursor::new(&all_bytes), 102).ok();
    let symbol_raw_b = symbol_resp_b.map(|v| v.to_string()).unwrap_or_default();
    if symbol_raw_b.contains("modBFoo") {
        println!("PASS (workspace): workspace symbol scoped to mod_b finds modBFoo");
    } else {
        println!("FAIL (workspace): workspace symbol did not find mod_b symbol (resp={})", symbol_raw_b);
        all_pass = false;
    }

    let has_panic = stderr_text.lines().any(|l| l.contains("panic") || l.contains("FATAL"));
    if has_panic {
        println!("FAIL (workspace): stderr contains panic/FATAL");
        for line in stderr_text.lines() {
            println!("  {line}");
        }
        all_pass = false;
    } else {
        println!("PASS (workspace): no stderr panic");
    }

    // Cleanup.
    let _ = std::fs::remove_dir_all(&game_root);
    let _ = std::fs::remove_dir_all(&mod_a);
    let _ = std::fs::remove_dir_all(&mod_b);
    let _ = std::fs::remove_file(&unowned);

    all_pass
}

/// Phase 3: semantic diagnostics via synthetic mod fixtures.
///
/// For each fixture we start a fresh server, register a mod workspace folder
/// containing the fixture files, `didOpen` the file of interest, then inspect
/// the `textDocument/publishDiagnostics` notifications.
fn run_semantic_tests() -> bool {
    let fixtures = [
        (
            "forward_decl_ok",
            vec![("forward_decl_ok.xs", include_str!("../semantic_fixtures/forward_decl_ok.xs"))],
            vec!["forward_decl_ok.xs"],
            Vec::<&str>::new(),
            vec!["before declaration"],
        ),
        (
            "forward_decl_missing",
            vec![("forward_decl_missing.xs", include_str!("../semantic_fixtures/forward_decl_missing.xs"))],
            vec!["forward_decl_missing.xs"],
            vec!["before declaration"],
            Vec::<&str>::new(),
        ),
        (
            "mutable_ok",
            vec![("mutable_ok.xs", include_str!("../semantic_fixtures/mutable_ok.xs"))],
            vec!["mutable_ok.xs"],
            Vec::<&str>::new(),
            vec!["different signature"],
        ),
        (
            "mutable_different_sig",
            vec![("mutable_different_sig.xs", include_str!("../semantic_fixtures/mutable_different_sig.xs"))],
            vec!["mutable_different_sig.xs"],
            vec!["different signature"],
            Vec::<&str>::new(),
        ),
        (
            "int_to_float_widening",
            vec![("int_to_float_widening.xs", include_str!("../semantic_fixtures/int_to_float_widening.xs"))],
            vec!["int_to_float_widening.xs"],
            Vec::<&str>::new(),
            vec!["expected argument"],
        ),
        (
            "float_to_int_loss",
            vec![("float_to_int_loss.xs", include_str!("../semantic_fixtures/float_to_int_loss.xs"))],
            vec!["float_to_int_loss.xs"],
            vec!["expected argument 1 of type `int`"],
            Vec::<&str>::new(),
        ),
        (
            "extern_collision",
            vec![
                ("extern_collision_a.xs", include_str!("../semantic_fixtures/extern_collision_a.xs")),
                ("extern_collision_b.xs", include_str!("../semantic_fixtures/extern_collision_b.xs")),
            ],
            vec!["extern_collision_b.xs"],
            vec!["extern collision"],
            Vec::<&str>::new(),
        ),
    ];

    let mut all_pass = true;
    for (name, files, open_files, expected, forbidden) in fixtures {
        let ok = run_semantic_fixture(name, &files, &open_files, &expected, &forbidden);
        if !ok {
            all_pass = false;
        }
    }
    all_pass
}

fn run_semantic_fixture(
    name: &str,
    files: &[(&str, &str)],
    open_files: &[&str],
    expected: &[&str],
    forbidden: &[&str],
) -> bool {
    let pid = std::process::id();
    let tmp = std::env::temp_dir();
    let game_root = tmp.join(format!("aomr_sem_game_{}_{}", name, pid));
    let mod_root = tmp.join(format!("aomr_sem_mod_{}_{}", name, pid));

    let _ = std::fs::remove_dir_all(&game_root);
    let _ = std::fs::remove_dir_all(&mod_root);

    // Minimal game folder with the doxygen archive.
    let doxy_src = resolve_test_game_path().join("doxygen_retail.7z");
    std::fs::create_dir_all(&game_root).expect("create game root");
    std::fs::copy(&doxy_src, game_root.join("doxygen_retail.7z")).expect("copy doxygen archive");

    // Write the fixture files into the mod overlay.
    let mod_game = mod_root.join("game").join("ai").join("test");
    for (rel, content) in files {
        let path = mod_game.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }

    let game_root = std::fs::canonicalize(&game_root).unwrap();
    let mod_root = std::fs::canonicalize(&mod_root).unwrap();
    let mod_uri = Url::from_file_path(&mod_root).unwrap();

    // Build file URIs for every opened file.
    let file_uris: Vec<(String, Url)> = open_files
        .iter()
        .map(|rel| {
            let path = mod_game.join(rel);
            let uri = Url::from_file_path(&path).unwrap();
            ((*rel).to_string(), uri)
        })
        .collect();

    let server_path = locate_server_binary();
    let mut child = Command::new(&server_path)
        .arg("--game-path")
        .arg(&game_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xs-language-server for semantic tests");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    let init = json!({
        "jsonrpc": "2.0",
        "id": 900,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {},
            "trace": "off",
            "rootUri": null,
            "workspaceFolders": [{ "uri": mod_uri, "name": name }]
        }
    })
    .to_string();

    let initialized = json!({"jsonrpc":"2.0","method":"initialized","params":{}}).to_string();

    stdin.write_all(frame(&init).as_bytes()).unwrap();
    stdin.write_all(frame(&initialized).as_bytes()).unwrap();
    stdin.flush().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));

    let file_content: std::collections::HashMap<&str, &str> =
        files.iter().copied().collect();

    for (rel, uri) in &file_uris {
        let content = file_content.get(rel.as_str()).unwrap();
        let did_open = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "xs",
                    "version": 1,
                    "text": content
                }
            }
        })
        .to_string();
        stdin.write_all(frame(&did_open).as_bytes()).unwrap();
        stdin.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    std::thread::sleep(std::time::Duration::from_millis(200));

    let shutdown = json!({"jsonrpc":"2.0","id":901,"method":"shutdown"}).to_string();
    let exit = json!({"jsonrpc":"2.0","method":"exit"}).to_string();
    stdin.write_all(frame(&shutdown).as_bytes()).unwrap();
    stdin.write_all(frame(&exit).as_bytes()).unwrap();
    stdin.flush().unwrap();
    drop(stdin);

    let status = child.wait().expect("wait on semantic-test child");
    let mut stderr_text = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr_text)
        .unwrap();

    let mut all_bytes = Vec::new();
    stdout.read_to_end(&mut all_bytes).unwrap();

    let mut all_messages: Vec<String> = Vec::new();
    let mut cursor = std::io::Cursor::new(&all_bytes);
    loop {
        match read_framed_message(&mut cursor) {
            Ok(msg) => {
                if msg.get("method").and_then(|m| m.as_str()) == Some("textDocument/publishDiagnostics") {
                    if let Some(params) = msg.get("params") {
                        if let Some(diagnostics) = params.get("diagnostics").and_then(|d| d.as_array()) {
                            for d in diagnostics {
                                if let Some(message) = d.get("message").and_then(|m| m.as_str()) {
                                    all_messages.push(message.to_string());
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }

    let mut pass = true;

    if status.success() {
        println!("PASS (semantic {}): server exited cleanly", name);
    } else {
        println!("FAIL (semantic {}): server exited with {}", name, status);
        pass = false;
    }

    let joined = all_messages.join("\n");
    for exp in expected {
        if joined.contains(exp) {
            println!("PASS (semantic {}): found expected diagnostic '{}'", name, exp);
        } else {
            println!("FAIL (semantic {}): missing expected diagnostic '{}'", name, exp);
            println!("  diagnostics: {:?}", all_messages);
            pass = false;
        }
    }

    for forb in forbidden {
        if !joined.contains(forb) {
            println!("PASS (semantic {}): no spurious '{}' diagnostic", name, forb);
        } else {
            println!("FAIL (semantic {}): unexpected '{}' diagnostic", name, forb);
            println!("  diagnostics: {:?}", all_messages);
            pass = false;
        }
    }

    if !stderr_text.is_empty() {
        println!("--- stderr for semantic {} ---", name);
        for line in stderr_text.lines() {
            println!("  {line}");
        }
        println!("--- end stderr ---");
    }

    let has_panic = stderr_text.lines().any(|l| l.contains("panic") || l.contains("FATAL"));
    if has_panic {
        println!("FAIL (semantic {}): stderr contains panic/FATAL", name);
        pass = false;
    }

    let _ = std::fs::remove_dir_all(&game_root);
    let _ = std::fs::remove_dir_all(&mod_root);

    pass
}

/// Phase 4: cross-include LSP features.
///
/// Each scenario creates a temporary mod with an includer (`main.xs`) and an
/// included file (`util.xs`) under `mod/game/ai/include_test/`. The server is
/// started with a synthetic game folder and the mod as a workspace folder.
fn run_include_tests() -> bool {
    let mut all_pass = true;
    all_pass &= run_completion_across_include();
    all_pass &= run_hover_across_include();
    all_pass &= run_definition_across_include();
    all_pass &= run_cycle_does_not_hang();
    all_pass
}

fn setup_include_mod(
    name: &str,
    main_content: &str,
    util_content: &str,
) -> (std::path::PathBuf, std::path::PathBuf, Url, Url) {
    let pid = std::process::id();
    let tmp = std::env::temp_dir();
    let game_root = tmp.join(format!("aomr_inc_game_{}_{}", name, pid));
    let mod_root = tmp.join(format!("aomr_inc_mod_{}_{}", name, pid));
    let _ = std::fs::remove_dir_all(&game_root);
    let _ = std::fs::remove_dir_all(&mod_root);

    // Copy the committed doxygen archive into the synthetic game folder.
    let doxy_src = resolve_test_game_path().join("doxygen_retail.7z");
    std::fs::create_dir_all(&game_root).expect("create game root");
    std::fs::copy(&doxy_src, game_root.join("doxygen_retail.7z")).expect("copy doxygen archive");

    // Fixture files directly under game/ai/ so include targets resolve
    // relative to the AI include root.
    let test_dir = mod_root.join("game").join("ai");
    let main_path = test_dir.join("main.xs");
    let util_path = test_dir.join("util.xs");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(&main_path, main_content).unwrap();
    std::fs::write(&util_path, util_content).unwrap();

    let game_root = std::fs::canonicalize(&game_root).unwrap();
    let mod_root = std::fs::canonicalize(&mod_root).unwrap();
    let main_uri = Url::from_file_path(&main_path).unwrap();
    let util_uri = Url::from_file_path(&util_path).unwrap();
    (game_root, mod_root, main_uri, util_uri)
}

fn cleanup_include_mod(game_root: &std::path::Path, mod_root: &std::path::Path) {
    let _ = std::fs::remove_dir_all(game_root);
    let _ = std::fs::remove_dir_all(mod_root);
}

fn run_include_request_scenario(
    name: &str,
    main_content: &str,
    util_content: &str,
    request_id: i64,
    request: &dyn Fn(&Url) -> serde_json::Value,
    check: &dyn Fn(&serde_json::Value, &Url) -> bool,
) -> bool {
    let (game_root, mod_root, main_uri, util_uri) =
        setup_include_mod(name, main_content, util_content);

    let server_path = locate_server_binary();
    let mod_uri = Url::from_file_path(&mod_root).unwrap();

    let mut child = Command::new(&server_path)
        .arg("--game-path")
        .arg(&game_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xs-language-server for include tests");

    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = child.stdout.take().expect("child stdout");

    let init = json!({
        "jsonrpc": "2.0",
        "id": 1000,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {},
            "trace": "off",
            "rootUri": null,
            "workspaceFolders": [{ "uri": mod_uri, "name": name }]
        }
    })
    .to_string();

    let initialized = json!({"jsonrpc":"2.0","method":"initialized","params":{}}).to_string();

    let did_open = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": main_uri,
                "languageId": "xs",
                "version": 1,
                "text": main_content
            }
        }
    })
    .to_string();

    let request = request(&main_uri);
    let request = serde_json::to_string(&request).unwrap();
    let shutdown = json!({"jsonrpc":"2.0","id":1001,"method":"shutdown"}).to_string();
    let exit = json!({"jsonrpc":"2.0","method":"exit"}).to_string();

    for msg in &[init, initialized, did_open, request, shutdown, exit] {
        stdin.write_all(frame(msg).as_bytes()).unwrap();
        stdin.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    drop(stdin);

    let status = child.wait().expect("wait on include-test child");
    let mut stderr_text = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr_text)
        .unwrap();

    let mut all_bytes = Vec::new();
    stdout.read_to_end(&mut all_bytes).unwrap();

    let mut response = None;
    let mut main_diagnostics: Vec<String> = Vec::new();
    let mut cursor = std::io::Cursor::new(&all_bytes);
    loop {
        match read_framed_message(&mut cursor) {
            Ok(msg) => {
                if msg.get("id").and_then(|v| v.as_i64()) == Some(request_id) {
                    response = Some(msg);
                } else if msg.get("method").and_then(|m| m.as_str())
                    == Some("textDocument/publishDiagnostics")
                {
                    if let Some(params) = msg.get("params") {
                        if params.get("uri").and_then(|u| u.as_str())
                            == Some(main_uri.as_str())
                        {
                            if let Some(arr) = params.get("diagnostics").and_then(|d| d.as_array())
                            {
                                for d in arr {
                                    if let Some(m) = d.get("message").and_then(|m| m.as_str()) {
                                        main_diagnostics.push(m.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }

    cleanup_include_mod(&game_root, &mod_root);

    let has_panic = stderr_text.lines().any(|l| l.contains("panic") || l.contains("FATAL"));
    let check_ok = response
        .as_ref()
        .map(|resp| check(resp, &util_uri))
        .unwrap_or(false);
    let pass = !has_panic && status.success() && check_ok;

    if pass {
        println!("PASS (include {}): response matched expected content", name);
    } else {
        println!("FAIL (include {}): response did not match expected content", name);
        if let Some(resp) = response {
            println!("  response: {}", resp);
        } else {
            println!("  response: <none>");
        }
        if !main_diagnostics.is_empty() {
            println!("  diagnostics for main.xs:");
            for d in &main_diagnostics {
                println!("    {d}");
            }
        }
        if has_panic || !stderr_text.is_empty() {
            println!("--- stderr for include {} ---", name);
            for line in stderr_text.lines() {
                println!("  {line}");
            }
            println!("--- end stderr ---");
        }
    }
    pass
}

fn run_completion_across_include() -> bool {
    let main = "include \"util.xs\";\n\nvoid caller()\n{\n    included_fn();\n}\n";
    let util = "void included_fn() {\n    aiEcho(\"included\");\n}\n";
    run_include_request_scenario(
        "completion_across_include",
        main,
        util,
        1100,
        &|main_uri| {
            json!({
                "jsonrpc": "2.0",
                "id": 1100,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": main_uri },
                    "position": { "line": 4, "character": 8 }
                }
            })
        },
        &|resp, _util_uri| {
            // The response may be CompletionItem[] or CompletionList.
            let raw = resp.to_string();
            raw.contains("\"included_fn\"") || raw.contains("\"label\":\"included_fn\"")
        },
    )
}

fn run_hover_across_include() -> bool {
    let main = "include \"util.xs\";\n\nvoid caller()\n{\n    included_fn();\n}\n";
    let util = "void included_fn() {\n    aiEcho(\"included\");\n}\n";
    run_include_request_scenario(
        "hover_across_include",
        main,
        util,
        1101,
        &|main_uri| {
            json!({
                "jsonrpc": "2.0",
                "id": 1101,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": main_uri },
                    "position": { "line": 4, "character": 6 }
                }
            })
        },
        &|resp, util_uri| {
            let raw = resp.to_string();
            // The hover markdown shows the signature and a path/URI pointing
            // to the defining util.xs file.
            raw.contains("included_fn")
                && raw.contains("void included_fn()")
                && raw.contains(util_uri.path())
        },
    )
}

fn run_definition_across_include() -> bool {
    let main = "include \"util.xs\";\n\nvoid caller()\n{\n    included_fn();\n}\n";
    let util = "void included_fn() {\n    aiEcho(\"included\");\n}\n";
    run_include_request_scenario(
        "definition_across_include",
        main,
        util,
        1102,
        &|main_uri| {
            json!({
                "jsonrpc": "2.0",
                "id": 1102,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": main_uri },
                    "position": { "line": 4, "character": 6 }
                }
            })
        },
        &|resp, util_uri| {
            let raw = resp.to_string();
            raw.contains(&util_uri.to_string()) && raw.contains("\"line\":0")
        },
    )
}

fn run_cycle_does_not_hang() -> bool {
    let main = "include \"util.xs\";\nvoid aFn() {}\n";
    let util = "include \"main.xs\";\nvoid bFn() {}\n";
    let (game_root, mod_root, main_uri, _util_uri) =
        setup_include_mod("cycle_does_not_hang", main, util);

    let server_path = locate_server_binary();
    let mod_uri = Url::from_file_path(&mod_root).unwrap();

    let mut child = Command::new(&server_path)
        .arg("--game-path")
        .arg(&game_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xs-language-server for cycle test");

    let mut stdin = child.stdin.take().expect("child stdin");
    // Don't take stdout; we won't read it, but the child must still exit.
    let _stdout = child.stdout.take().expect("child stdout");

    let init = json!({
        "jsonrpc": "2.0",
        "id": 1200,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {},
            "trace": "off",
            "rootUri": null,
            "workspaceFolders": [{ "uri": mod_uri, "name": "cycle" }]
        }
    })
    .to_string();

    let initialized = json!({"jsonrpc":"2.0","method":"initialized","params":{}}).to_string();

    let did_open = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": main_uri,
                "languageId": "xs",
                "version": 1,
                "text": main
            }
        }
    })
    .to_string();

    let shutdown = json!({"jsonrpc":"2.0","id":1201,"method":"shutdown"}).to_string();
    let exit = json!({"jsonrpc":"2.0","method":"exit"}).to_string();

    for msg in &[init, initialized, did_open, shutdown, exit] {
        stdin.write_all(frame(msg).as_bytes()).unwrap();
        stdin.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    drop(stdin);

    // The merge must terminate in bounded time even on a cycle.
    let timeout = std::time::Duration::from_secs(5);
    let start = std::time::Instant::now();
    let mut status = None;
    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(s)) => {
                status = Some(s);
                break;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(_) => break,
        }
    }

    cleanup_include_mod(&game_root, &mod_root);

    if status.is_none() {
        let _ = child.kill();
        println!("FAIL (cycle_does_not_hang): server did not exit within 5 seconds");
        return false;
    }

    let status = status.unwrap();
    if status.success() {
        println!("PASS (cycle_does_not_hang): server exited cleanly despite include cycle");
        true
    } else {
        println!("FAIL (cycle_does_not_hang): server exited with {}", status);
        false
    }
}

fn main() {
    let baseline_ok = run_baseline();
    let workspace_ok = run_workspace_tests();
    let semantic_ok = run_semantic_tests();
    let include_ok = run_include_tests();

    if baseline_ok && workspace_ok && semantic_ok && include_ok {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
