use tower_lsp::lsp_types::Position;

/// Converts between UTF-8 byte offsets used by LALRPOP and UTF-16 positions
/// used by the language server protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut line_starts = vec![0];
        let mut offset = 0;

        while offset < bytes.len() {
            match bytes[offset] {
                b'\r' if bytes.get(offset + 1) == Some(&b'\n') => {
                    offset += 2;
                    line_starts.push(offset);
                }
                b'\r' | b'\n' => {
                    offset += 1;
                    line_starts.push(offset);
                }
                _ => offset += 1,
            }
        }

        Self { line_starts }
    }

    pub(crate) fn position(&self, text: &str, offset: usize) -> Option<Position> {
        if offset > text.len() || !text.is_char_boundary(offset) {
            return None;
        }

        let line = self
            .line_starts
            .partition_point(|line_start| *line_start <= offset)
            .checked_sub(1)?;
        let (line_start, line_end) = self.line_bounds(text, line)?;
        let character = text
            .get(line_start..offset.min(line_end))?
            .encode_utf16()
            .count();

        Some(Position {
            line: u32::try_from(line).ok()?,
            character: u32::try_from(character).ok()?,
        })
    }

    pub(crate) fn offset(&self, text: &str, position: Position) -> Option<usize> {
        let line = usize::try_from(position.line).ok()?;
        let target = usize::try_from(position.character).ok()?;
        let (line_start, line_end) = self.line_bounds(text, line)?;
        let line_text = text.get(line_start..line_end)?;
        let mut utf16_offset = 0;

        for (byte_offset, character) in line_text.char_indices() {
            if utf16_offset == target {
                return Some(line_start + byte_offset);
            }

            let next_offset = utf16_offset + character.len_utf16();
            if target < next_offset {
                // A byte offset cannot represent a position in the middle of a
                // UTF-16 surrogate pair.
                return None;
            }
            utf16_offset = next_offset;
        }

        // LSP positions past the end of a line are clamped to the line end.
        Some(line_end)
    }

    fn line_bounds(&self, text: &str, line: usize) -> Option<(usize, usize)> {
        let line_start = *self.line_starts.get(line)?;
        let mut line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(text.len());
        let bytes = text.as_bytes();

        if line_end > line_start && bytes.get(line_end - 1) == Some(&b'\n') {
            line_end -= 1;
        }
        if line_end > line_start && bytes.get(line_end - 1) == Some(&b'\r') {
            line_end -= 1;
        }

        Some((line_start, line_end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_utf8_offsets_to_utf16_positions() {
        let text = "a😀é\nβ";
        let index = LineIndex::new(text);

        assert_eq!(index.position(text, 0), Some(Position::new(0, 0)));
        assert_eq!(index.position(text, 1), Some(Position::new(0, 1)));
        assert_eq!(index.position(text, 5), Some(Position::new(0, 3)));
        assert_eq!(index.position(text, 7), Some(Position::new(0, 4)));
        assert_eq!(index.position(text, 8), Some(Position::new(1, 0)));
        assert_eq!(index.position(text, 10), Some(Position::new(1, 1)));

        assert_eq!(index.offset(text, Position::new(0, 0)), Some(0));
        assert_eq!(index.offset(text, Position::new(0, 1)), Some(1));
        assert_eq!(index.offset(text, Position::new(0, 3)), Some(5));
        assert_eq!(index.offset(text, Position::new(0, 4)), Some(7));
        assert_eq!(index.offset(text, Position::new(1, 1)), Some(10));
    }

    #[test]
    fn rejects_surrogate_pair_interiors_and_clamps_long_lines() {
        let text = "a😀";
        let index = LineIndex::new(text);

        assert_eq!(index.offset(text, Position::new(0, 2)), None);
        assert_eq!(index.offset(text, Position::new(0, 99)), Some(text.len()));
        assert_eq!(index.offset(text, Position::new(1, 0)), None);
    }

    #[test]
    fn handles_all_lsp_line_endings() {
        let text = "a\r\nb\rc\n";
        let index = LineIndex::new(text);

        assert_eq!(index.position(text, 1), Some(Position::new(0, 1)));
        assert_eq!(index.position(text, 2), Some(Position::new(0, 1)));
        assert_eq!(index.position(text, 3), Some(Position::new(1, 0)));
        assert_eq!(index.position(text, 5), Some(Position::new(2, 0)));
        assert_eq!(index.position(text, 7), Some(Position::new(3, 0)));
    }
}
