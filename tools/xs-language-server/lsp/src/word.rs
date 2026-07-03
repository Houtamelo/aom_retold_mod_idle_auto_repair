//! Tiny helpers for finding the identifier the user is hovering over or
//! jumping to the definition of. Shared between hover and definition
//! handlers (and, if we ever need it, go-to-references).
//!
//! Currently we don't walk the AST to verify that the cursor is actually on
//! an identifier — we just scan the line over `[A-Za-z0-9_]`. Good enough
//! for the spike; we'll add an AST-aware pass in a later week.

/// Return the full identifier that the cursor is on, or `None` if the
/// cursor is on whitespace, punctuation, or past the end of the line.
///
/// Cursor must be ON an identifier character. We intentionally do NOT
/// snap back when the cursor is just past an identifier — that's a UX
/// choice that some clients make, but for our server the LSP client
/// decides where to put the cursor, and we'll return None if it's not
/// actually on an identifier.
///
/// This differs from `completion::prefix_at_cursor`, which scans only
/// backwards and returns everything from the identifier start up to the
/// cursor (the prefix being typed). For hover/definition we want the
/// full token; for completion we want the partial token.
pub fn identifier_at_cursor(text: &str, line: u32, character: u32) -> Option<String> {
    let line_str = text.lines().nth(line as usize)?;
    let col = (character as usize).min(line_str.len());

    // Cursor must be ON an identifier char.
    let on_ident = line_str[col..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    if !on_ident {
        return None;
    }

    // Walk back from col to find the start of the identifier.
    let prefix_start = line_str[..col]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map(|(i, _)| i)
        .unwrap_or(col);

    // Walk forward from col to find the end of the identifier.
    let suffix_end = line_str[col..]
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map(|(i, c)| col + i + c.len_utf8())
        .unwrap_or(col);

    if prefix_start >= suffix_end {
        return None;
    }
    Some(line_str[prefix_start..suffix_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test file: "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n"
    // .lines() strips the trailing \n on each line, so line 3 is
    // "   aiEcho(\"hi\");", chars 0..15:
    //   0=' '  1=' '  2=' '  3='a'  4='i'  5='E'  6='c'  7='h'
    //   8='o'  9='('  10='"' 11='h' 12='i' 13='"' 14=')' 15=';'

    #[test]
    fn cursor_at_start_of_word() {
        let text = "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n";
        // Col 3 = 'a' (start of aiEcho)
        assert_eq!(identifier_at_cursor(text, 3, 3).as_deref(), Some("aiEcho"));
    }

    #[test]
    fn cursor_in_middle_of_word() {
        let text = "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n";
        // Col 5 = 'E' (middle of aiEcho)
        assert_eq!(identifier_at_cursor(text, 3, 5).as_deref(), Some("aiEcho"));
    }

    #[test]
    fn cursor_at_end_of_word() {
        let text = "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n";
        // Col 8 = 'o' (last char of aiEcho)
        assert_eq!(identifier_at_cursor(text, 3, 8).as_deref(), Some("aiEcho"));
    }

    #[test]
    fn cursor_on_open_paren_after_word() {
        let text = "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n";
        // Col 9 = '(' (right after aiEcho) — NOT an identifier char
        assert!(identifier_at_cursor(text, 3, 9).is_none());
    }

    #[test]
    fn cursor_on_whitespace_between_words() {
        let text = "rule test\nactive\n{\n   aiEcho trEcho;\n}\n";
        // Line 3 = "   aiEcho trEcho;" (no trailing \n due to .lines()).
        // Col 9 = ' ' between aiEcho and trEcho.
        assert!(identifier_at_cursor(text, 3, 9).is_none());
    }

    #[test]
    fn cursor_on_leading_whitespace() {
        let text = "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n";
        // Col 2 = last leading space
        assert!(identifier_at_cursor(text, 3, 2).is_none());
    }

    #[test]
    fn cursor_past_end_of_line() {
        let text = "rule test\nactive\n{\n   aiEcho(\"hi\");\n}\n";
        // Col 16 = past the ';' (line is 16 chars long)
        assert!(identifier_at_cursor(text, 3, 16).is_none());
    }
}
