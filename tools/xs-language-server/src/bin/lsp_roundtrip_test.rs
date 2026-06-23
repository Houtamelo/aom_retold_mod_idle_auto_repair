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
//  10:    return a + b;
//  11: }
//  12:
//  13: const int cMagic = 42;
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
    for msg in &[INITIALIZE, INITIALIZED, DID_OPEN, DID_OPEN_BAD, COMPLETION_AI, HOVER_AI, DEFINITION_AI, DID_OPEN_WORKSPACE, HOVER_WORKSPACE, DEFINITION_WORKSPACE, HOVER_CONSTANT, DOCUMENT_SYMBOL, DID_OPEN_REFS, REFERENCES, RENAME, PREPARE_RENAME, PREPARE_RENAME_ENGINE, SHUTDOWN, EXIT] {
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

    // Definition: the response should be a `GotoDefinitionResponse::Scalar`
    // whose `Location.uri` is "xs-stub://engine/aiEcho".
    let def_raw = definition_resp.to_string();
    let has_stub_uri = def_raw.contains("xs-stub://engine/aiEcho");
    if has_stub_uri {
        println!("PASS: definition returned xs-stub://engine/aiEcho");
    } else {
        println!("FAIL: definition did not return xs-stub://engine/aiEcho");
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