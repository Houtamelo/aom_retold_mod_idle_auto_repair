//! Source-span to LSP range conversion.
//!
//! LSP positions are expressed as (zero-based line, zero-based character)
//! where `character` is counted in **UTF-16 code units** (LSP 3.17 default).
//! These helpers build a line-offset table once and translate byte spans
//! and cursor positions accordingly.

use tower_lsp_server::ls_types::{Position, Range};

/// A precomputed table of byte offsets where each source line begins and
/// where the line text (excluding the line terminator) ends.
#[derive(Debug, Clone)]
struct LineTable {
    starts: Vec<usize>,
    ends: Vec<usize>,
}

impl LineTable {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        let mut ends = Vec::new();
        let bytes = source.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let newline_len = if bytes[i] == b'\n' {
                1
            } else if bytes[i] == b'\r' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    2
                } else {
                    1
                }
            } else {
                0
            };

            if newline_len > 0 {
                ends.push(i);
                starts.push(i + newline_len);
                i += newline_len;
            } else {
                i += 1;
            }
        }
        ends.push(source.len());
        Self { starts, ends }
    }

    fn len(&self) -> usize { self.starts.len() }

    fn line_start(&self, line: usize) -> Option<usize> { self.starts.get(line).copied() }

    /// Exclusive end of the line text, i.e. the byte offset of the line
    /// terminator or `source.len()` for the final line.
    fn line_end(&self, line: usize) -> Option<usize> { self.ends.get(line).copied() }
}

/// Convert a byte `span` in `source` into an LSP `Range`.
///
/// * Line endings are recognised for both LF and CRLF.
/// * Multi-byte UTF-8 characters are translated to UTF-16 code-unit counts.
/// * Empty spans (`start == end`) produce a zero-width range.
/// * Out-of-bounds spans are clamped to the source length.
pub fn span_to_range(source: &str, span: std::ops::Range<usize>) -> Range {
    let table = LineTable::new(source);
    Range::new(
        byte_offset_to_position(source, &table, span.start),
        byte_offset_to_position(source, &table, span.end),
    )
}

/// Convert a byte offset in `source` to an LSP `Position`.
fn byte_offset_to_position(source: &str, table: &LineTable, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let line = line_index(table, offset);
    let line_start = table.line_start(line).unwrap_or(0);
    let line_start_byte = line_start.min(source.len());
    let line_text = &source[line_start_byte..offset.min(source.len())];
    let character = utf16_len(line_text);
    Position::new(line as u32, character)
}

/// Find the zero-based line index containing `offset` using the line table.
fn line_index(table: &LineTable, offset: usize) -> usize {
    match table.starts.binary_search(&offset) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    }
}

/// Count the number of UTF-16 code units in `text`.
fn utf16_len(text: &str) -> u32 {
    text.chars().map(|c| c.len_utf16() as u32).sum()
}

/// Convert an LSP `(line, character)` position to a byte offset in `source`.
///
/// Returns `None` if the position is beyond the end of the document or
/// behind an unrecognised line boundary.
pub fn position_to_byte_offset(source: &str, line: u32, character: u32) -> Option<usize> {
    let table = LineTable::new(source);
    let line = line as usize;
    if line >= table.len() {
        return None;
    }
    let line_start = table.line_start(line)?;
    let line_end = table.line_end(line)?;
    if line_start > line_end {
        return None;
    }
    let line_text = &source[line_start..line_end];
    let mut utf16_count: u32 = 0;
    for (byte_idx, c) in line_text.char_indices() {
        if utf16_count >= character {
            return Some(line_start + byte_idx);
        }
        utf16_count += c.len_utf16() as u32;
    }
    if utf16_count == character {
        Some(line_start + line_text.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_to_range_lf() {
        let source = "abc\ndef";
        let range = span_to_range(source, 4..7);
        assert_eq!(range.start, Position::new(1, 0));
        assert_eq!(range.end, Position::new(1, 3));
    }

    #[test]
    fn span_to_range_crlf() {
        let source = "abc\r\ndef";
        let range = span_to_range(source, 5..8);
        assert_eq!(range.start, Position::new(1, 0));
        assert_eq!(range.end, Position::new(1, 3));
    }

    #[test]
    fn span_to_range_multibyte() {
        // Two emoji: each is 4 UTF-8 bytes and 2 UTF-16 code units.
        let source = "😀😁";
        let range = span_to_range(source, 0..8);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 4));
    }

    #[test]
    fn span_to_range_empty() {
        let source = "int x;";
        let range = span_to_range(source, 3..3);
        assert_eq!(range.start, range.end);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 3);
    }

    #[test]
    fn span_to_range_clamps_out_of_bounds() {
        let source = "short";
        let range = span_to_range(source, 10..20);
        assert_eq!(range.start, Position::new(0, 5));
        assert_eq!(range.end, Position::new(0, 5));
    }

    #[test]
    fn position_to_byte_offset_basic() {
        let source = "abc\ndef";
        assert_eq!(position_to_byte_offset(source, 0, 0), Some(0));
        assert_eq!(position_to_byte_offset(source, 1, 0), Some(4));
        assert_eq!(position_to_byte_offset(source, 1, 2), Some(6));
    }

    #[test]
    fn position_to_byte_offset_crlf() {
        let source = "abc\r\ndef";
        assert_eq!(position_to_byte_offset(source, 1, 0), Some(5));
        assert_eq!(position_to_byte_offset(source, 1, 3), Some(8));
    }

    #[test]
    fn position_to_byte_offset_multibyte() {
        // Two emoji: each is 4 UTF-8 bytes and 2 UTF-16 code units.
        let source = "😀😁";
        assert_eq!(position_to_byte_offset(source, 0, 2), Some(4));
        assert_eq!(position_to_byte_offset(source, 0, 4), Some(8));
    }
}
