//! Per-file diagnostic breakdown for the two PR-E outlier files.
//!
//! Run with:
//!     AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
//!         cargo run -p xs-parser --example outlier_breakdown -- \
//!             game/ai/core/godpowers/godpowers.xs
//!
//! Writes:
//!     /tmp/outlier_breakdown.txt

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use xs_parser::ast::TypeTable;
use xs_parser::parser::Parser;

#[derive(Debug, Clone)]
struct Hit {
    line: usize,
    col: usize,
    message: String,
    source_line: String,
    context: String,
}

fn walk_xs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_xs(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("xs") {
            out.push(path);
        }
    }
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            starts.push((i + 1).min(source.len()));
            i += 1;
        } else if bytes[i] == b'\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                starts.push((i + 2).min(source.len()));
                i += 2;
            } else {
                starts.push((i + 1).min(source.len()));
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    starts
}

fn byte_to_line_col(source: &str, starts: &[usize], offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let line = starts
        .iter()
        .enumerate()
        .rfind(|&(_, &s)| s <= offset)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let line_start = starts.get(line).copied().unwrap_or(0).min(source.len());
    let col = source[line_start..offset].chars().count();
    (line + 1, col + 1)
}

fn context_lines(source: &str, starts: &[usize], line: usize) -> String {
    let start = line.saturating_sub(5).max(1);
    let end = (line + 5).min(starts.len());
    let mut out = String::new();
    for l in start..=end {
        let s = starts.get(l - 1).copied().unwrap_or(0);
        let e = starts.get(l).copied().unwrap_or(source.len());
        let text = &source[s..e.min(source.len())];
        out.push_str(&format!("{:4} | {}", l, text.trim_end_matches('\n')));
        if l < end {
            out.push('\n');
        }
    }
    out
}

fn extract_class_names(source: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut in_block_comment = false;
    for line in source.lines() {
        let mut i = 0;
        let bytes = line.as_bytes();
        let line_end = bytes.len();
        while i < line_end {
            if in_block_comment {
                if let Some(pos) = line[i..].find("*/") {
                    i += pos + 2;
                    in_block_comment = false;
                    continue;
                }
                break;
            }
            if line[i..].starts_with("/*") {
                in_block_comment = true;
                i += 2;
                continue;
            }
            if line[i..].starts_with("//") {
                break;
            }
            let rest = &line[i..];
            if rest.len() >= 5
                && rest.as_bytes()[0..5] == *b"class"
                && (rest.len() == 5 || !rest.as_bytes()[5].is_ascii_alphanumeric() && rest.as_bytes()[5] != b'_')
            {
                let after = rest[5..].trim_start();
                if let Some(word_end) = after.find(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                    let name = &after[..word_end];
                    if !name.is_empty() {
                        names.insert(name.to_string());
                    }
                }
                break;
            }
            i += 1;
        }
    }
    names
}

fn classify_source_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return "<empty line>".to_string();
    }
    let code_no_comment = trimmed.split("//").next().unwrap_or(trimmed).trim();
    if code_no_comment.is_empty() {
        return "<comment-only line>".to_string();
    }
    let first = code_no_comment.split_whitespace().next().unwrap_or("");
    if first == "}" {
        return "}".to_string();
    }
    if first.starts_with("#") {
        return format!("preprocessor directive `{}`", first);
    }
    if first == "{" {
        return "Top-level compound statement `{ ... }`".to_string();
    }
    if code_no_comment.contains("/*") || code_no_comment.contains("*/") {
        return "C-style block comment edge case".to_string();
    }
    if code_no_comment.contains('[') && code_no_comment.contains(']') {
        return "Array-typed declaration `Type[] name`".to_string();
    }
    if code_no_comment.contains("[") && code_no_comment.contains("](") {
        return "Lambda expression `[...](...) {}`".to_string();
    }
    "<general statement>".to_string()
}

fn classify_hit(source: &str, starts: &[usize], line: usize, message: &str) -> String {
    let idx = line.saturating_sub(1);
    let start = starts.get(idx).copied().unwrap_or(0);
    let end = starts.get(idx + 1).copied().unwrap_or(source.len());
    let source_line = &source[start..end.min(source.len())];
    if message.starts_with("invalid syntax, expected one of:") {
        let mut base = classify_source_line(source_line);
        if base == "<general statement>" {
            base = format!("expected-one-of error on: {}", source_line.trim().chars().take(70).collect::<String>());
        }
        return base;
    }
    if message == "invalid syntax" {
        return classify_source_line(source_line);
    }
    format!("{} near: {}", message, source_line.trim().chars().take(70).collect::<String>())
}

fn main() {
    let game_dir = std::env::var("AOMR_GAME_PATH")
        .map(PathBuf::from)
        .expect("AOMR_GAME_PATH env var required");
    let game_folder = game_dir.join("game");

    let file_arg = std::env::args().nth(1).expect("usage: cargo run --example outlier_breakdown -- <relative-or-absolute xs file>");
    let file_path = if Path::new(&file_arg).is_absolute() {
        PathBuf::from(file_arg)
    } else {
        game_folder.join(&file_arg)
    };

    // Collect class names from the whole game folder (matches LSP test behaviour).
    let mut all_files = Vec::new();
    walk_xs(&game_folder, &mut all_files);
    let mut class_names = HashSet::new();
    for f in &all_files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        class_names.extend(extract_class_names(&src));
    }
    let base_types = TypeTable::with_primitives();
    let mut game_types = base_types;
    for name in &class_names {
        game_types.insert_class(name);
    }

    let source = std::fs::read_to_string(&file_path)
        .unwrap_or_else(|e| panic!("failed to read {file_path:?}: {e}"));
    let starts = line_starts(&source);

    let mut diags = vec![];
    let _cst = Parser::new_with_context(&source, &mut diags, game_types.clone()).parse(&mut diags);

    let mut msg_count: HashMap<String, usize> = HashMap::new();
    let mut grouped: BTreeMap<String, Vec<Hit>> = BTreeMap::new();
    for d in &diags {
        *msg_count.entry(d.message.clone()).or_insert(0) += 1;
        let span = d.labels.first().map(|l| l.range.clone()).unwrap_or(0..0);
        let offset = span.start;
        let (line, col) = byte_to_line_col(&source, &starts, offset);
        let line_start = starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
        let line_end = starts.get(line).copied().unwrap_or(source.len());
        let source_line = source[line_start..line_end.min(source.len())].to_string();
        let context = context_lines(&source, &starts, line);
        let fingerprint = classify_hit(&source, &starts, line, &d.message);
        grouped.entry(fingerprint).or_default().push(Hit {
            line,
            col,
            message: d.message.clone(),
            source_line: source_line.trim().to_string(),
            context,
        });
    }

    let mut groups: Vec<_> = grouped.into_iter().collect();
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let rel = file_path.strip_prefix(&game_folder).unwrap_or(&file_path).display();
    let mut txt = String::new();
    let rel_str = rel.to_string();
    txt.push_str(&format!("File: {}\n", rel_str));
    txt.push_str(&format!("Total ERROR parse diagnostics: {}\n\n", diags.len()));

    txt.push_str("Top diagnostic messages (by frequency):\n");
    let mut sorted_msgs: Vec<_> = msg_count.iter().collect();
    sorted_msgs.sort_by(|a, b| b.1.cmp(a.1));
    for (msg, count) in sorted_msgs.iter().take(50) {
        txt.push_str(&format!("  {} : {}\n", count, msg));
    }
    txt.push_str("\n");

    txt.push_str("Top source-text fingerprints:\n\n");
    for (rank, (fp, hits)) in groups.iter().enumerate() {
        let hit = &hits[0];
        txt.push_str(&format!(
            "{}. Count: {}\n   Fingerprint: {}\n   Message: {}\n   Source line ({}:{}): {}\n   Context:\n{}\n\n",
            rank + 1,
            hits.len(),
            fp,
            hit.message,
            hit.line,
            hit.col,
            hit.source_line,
            hit.context.replace('\n', "\n      ")
        ));
    }

    let output = "/tmp/outlier_breakdown.txt";
    let mut final_output = txt.clone();
    final_output.push_str("\n--- Raw diagnostics (first 200) ---\n");
    for d in diags.iter().take(200) {
        let span = d.labels.first().map(|l| l.range.clone()).unwrap_or(0..0);
        let (line, col) = byte_to_line_col(&source, &starts, span.start);
        let line_start = starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
        let line_end = starts.get(line).copied().unwrap_or(source.len());
        let source_line = source[line_start..line_end.min(source.len())].to_string();
        final_output.push_str(&format!("{}:{}: {} : {}\n", line, col, d.message, source_line.trim()));
    }

    std::fs::write(output, &final_output).expect("write /tmp");
    println!("{}: {} diagnostics", rel_str, diags.len());
    println!("Wrote {}", output);
    println!("\nTop 10 fingerprints:");
    for (rank, (fp, hits)) in groups.iter().take(10).enumerate() {
        println!("  {}. count={}  {}", rank + 1, hits.len(), fp);
    }
}
