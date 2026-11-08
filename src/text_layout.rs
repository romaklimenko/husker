use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A visual line and the original byte offsets represented by its cell boundaries.
#[derive(Debug)]
pub(crate) struct VisualLine {
    pub text: String,
    start: usize,
    end: usize,
    boundaries: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub(crate) struct TextLayout {
    pub lines: Vec<VisualLine>,
}

impl TextLayout {
    pub fn new(text: &str, width: usize) -> Self {
        let width = width.max(1);
        let mut lines = Vec::new();
        let mut paragraph_start = 0;
        for paragraph in text.split('\n') {
            let glyphs: Vec<(usize, &str)> = paragraph.grapheme_indices(true).collect();
            if glyphs.is_empty() {
                lines.push(VisualLine {
                    text: String::new(),
                    start: paragraph_start,
                    end: paragraph_start,
                    boundaries: vec![(paragraph_start, 0)],
                });
            }
            let mut start = 0;
            while start < glyphs.len() {
                let mut end = start;
                let mut cells = 0;
                let mut last_break = None;
                let mut saw_word = false;
                while end < glyphs.len() {
                    let glyph = glyphs[end].1;
                    let next = cells + glyph_width(glyph, cells);
                    if next > width && end > start {
                        break;
                    }
                    cells = next;
                    end += 1;
                    if is_break_space(glyph) {
                        if saw_word {
                            last_break = Some(end);
                        }
                    } else {
                        saw_word = true;
                    }
                    if cells > width {
                        break;
                    }
                }
                let mut visible_end = end;
                if end < glyphs.len() {
                    if is_break_space(glyphs[end].1) {
                        // Keep soft-break whitespace in the source range, not on a new blank row.
                        while end < glyphs.len() && is_break_space(glyphs[end].1) {
                            end += 1;
                        }
                    } else if let Some(boundary) = last_break {
                        end = boundary;
                        visible_end = end;
                    }
                    while visible_end > start && is_break_space(glyphs[visible_end - 1].1) {
                        visible_end -= 1;
                    }
                }
                let start_byte = paragraph_start + glyphs[start].0;
                let end_byte = paragraph_start
                    + glyphs
                        .get(end)
                        .map_or(paragraph.len(), |(offset, _)| *offset);
                let mut rendered = String::new();
                let mut boundaries = vec![(start_byte, 0)];
                let mut column = 0;
                for (index, (offset, glyph)) in glyphs.iter().enumerate().take(end).skip(start) {
                    if index < visible_end {
                        let size = glyph_width(glyph, column);
                        if *glyph == "\t" {
                            rendered.extend(std::iter::repeat_n(' ', size));
                        } else {
                            rendered.push_str(glyph);
                        }
                        column += size;
                    }
                    boundaries.push((paragraph_start + offset + glyph.len(), column));
                }
                lines.push(VisualLine {
                    text: rendered,
                    start: start_byte,
                    end: end_byte,
                    boundaries,
                });
                start = end;
            }
            paragraph_start += paragraph.len() + 1;
        }
        Self { lines }
    }

    pub fn cursor_position(&self, byte: usize) -> (usize, usize) {
        let row = self
            .lines
            .iter()
            .rposition(|line| line.start <= byte)
            .unwrap_or(0);
        let column = self.lines[row]
            .boundaries
            .iter()
            .rfind(|(offset, _)| *offset <= byte)
            .map_or(0, |(_, column)| *column);
        (row, column)
    }

    pub fn cursor_at(&self, row: usize, column: usize) -> usize {
        let row = row.min(self.lines.len().saturating_sub(1));
        let line = &self.lines[row];
        let wraps_to_next = self
            .lines
            .get(row + 1)
            .is_some_and(|next| next.start == line.end);
        line.boundaries
            .iter()
            .filter(|(byte, cell)| *cell <= column && (!wraps_to_next || *byte < line.end))
            .next_back()
            .map_or(line.start, |(byte, _)| *byte)
    }

    pub fn nearest_cursor(&self, row: usize, column: usize) -> usize {
        let row = row.min(self.lines.len().saturating_sub(1));
        let line = &self.lines[row];
        let wraps_to_next = self
            .lines
            .get(row + 1)
            .is_some_and(|next| next.start == line.end);
        line.boundaries
            .iter()
            .filter(|(byte, _)| !wraps_to_next || *byte < line.end)
            .min_by_key(|(_, cell)| cell.abs_diff(column))
            .map_or(line.start, |(byte, _)| *byte)
    }

    pub fn selection_columns(&self, row: usize, selection: &Range<usize>) -> Option<Range<usize>> {
        if selection.is_empty() {
            return None;
        }
        let line = self.lines.get(row)?;
        let selected_text = selection.start < line.end && selection.end > line.start;
        let selected_newline = self.lines.get(row + 1).is_some_and(|next| {
            next.start == line.end + 1 && selection.start <= line.end && selection.end > line.end
        });
        if !selected_text && !selected_newline {
            return None;
        }
        let column_at = |byte| {
            line.boundaries
                .iter()
                .rfind(|(offset, _)| *offset <= byte)
                .map_or(0, |(_, column)| *column)
        };
        let start = column_at(selection.start.max(line.start));
        let mut end = column_at(selection.end.min(line.end));
        if selected_newline || start == end {
            end += 1;
        }
        Some(start..end)
    }
}

fn is_break_space(glyph: &str) -> bool {
    glyph.chars().all(|character| {
        character.is_whitespace() && character != '\u{a0}' && character != '\u{202f}'
    })
}

fn glyph_width(glyph: &str, column: usize) -> usize {
    if glyph == "\t" {
        4 - column % 4
    } else {
        UnicodeWidthStr::width(glyph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_rows(layout: &TextLayout) -> Vec<&str> {
        layout.lines.iter().map(|line| line.text.as_str()).collect()
    }

    #[test]
    fn wraps_words_and_preserves_original_byte_offsets() {
        let text = "one two three\nfour\n";
        let layout = TextLayout::new(text, 8);
        assert_eq!(text_rows(&layout), ["one two", "three", "four", ""]);
        assert_eq!(layout.cursor_position(8), (1, 0));
        assert_eq!(layout.cursor_at(1, 2), 10);
        assert_eq!(layout.cursor_position(13), (1, 5));
        assert_eq!(layout.cursor_position(text.len()), (3, 0));
        assert_eq!(layout.cursor_at(3, 99), text.len());
    }

    #[test]
    fn exact_width_words_consume_soft_spaces_without_blank_lines() {
        let text = "hello   world";
        let layout = TextLayout::new(text, 5);
        assert_eq!(text_rows(&layout), ["hello", "world"]);
        assert_eq!(layout.cursor_position(8), (1, 0));
        assert_eq!(layout.cursor_at(0, 99), 7);
        assert_eq!(&text[layout.cursor_at(1, 0)..], "world");
    }

    #[test]
    fn splits_long_words_at_grapheme_boundaries_and_expands_tabs() {
        let text = "a\tb 界界 👨‍👩‍👧‍👦e\u{301}longword";
        let layout = TextLayout::new(text, 6);
        assert_eq!(layout.lines[0].text, "a   b");
        assert_eq!(layout.cursor_at(0, 4), 2);
        assert_eq!(layout.lines[1].text, "界界");
        assert!(layout.lines.iter().any(|line| line.text.contains("👨‍👩‍👧‍👦")));
        assert!(
            layout
                .lines
                .iter()
                .any(|line| line.text.contains("e\u{301}"))
        );
        for (row, line) in layout.lines.iter().enumerate() {
            assert!(UnicodeWidthStr::width(line.text.as_str()) <= 6);
            for column in 0..7 {
                let byte = layout.cursor_at(row, column);
                assert!(text.is_char_boundary(byte));
                assert_eq!(layout.cursor_position(byte).0, row);
            }
        }
    }

    #[test]
    fn empty_paragraphs_and_leading_spaces_remain_visible() {
        let layout = TextLayout::new("\n  abcdefghi\n\n", 5);
        assert_eq!(text_rows(&layout), ["", "  abc", "defgh", "i", "", ""]);
        assert_eq!(layout.cursor_at(0, 0), 0);
        assert_eq!(TextLayout::new("", 0).cursor_at(100, 100), 0);
    }

    #[test]
    fn vertical_cursor_uses_nearest_cell_boundary_across_tabs_and_wide_glyphs() {
        let layout = TextLayout::new("123\n\tb", 8);
        assert_eq!(layout.cursor_at(1, 3), 4);
        assert_eq!(layout.nearest_cursor(1, 3), 5);
        assert_eq!(layout.cursor_position(layout.nearest_cursor(1, 3)), (1, 4));
        let wide = TextLayout::new("abc\n界x", 8);
        assert_eq!(wide.nearest_cursor(1, 1), 4);
        assert_eq!(wide.nearest_cursor(1, 2), 7);
    }

    #[test]
    fn selection_columns_follow_wrapping_tabs_and_newlines() {
        let layout = TextLayout::new("one two three", 7);
        assert_eq!(layout.selection_columns(0, &(4..11)), Some(4..7));
        assert_eq!(layout.selection_columns(1, &(4..11)), Some(0..3));
        assert_eq!(layout.selection_columns(0, &(8..13)), None);
        assert_eq!(layout.selection_columns(0, &(4..4)), None);
        let tabs = TextLayout::new("a\t界e\u{301}\n\nlast", usize::MAX);
        assert_eq!(tabs.selection_columns(0, &(1..2)), Some(1..4));
        assert_eq!(tabs.selection_columns(0, &(2..5)), Some(4..6));
        assert_eq!(tabs.selection_columns(0, &(8..9)), Some(7..8));
        assert_eq!(tabs.selection_columns(1, &(9..10)), Some(0..1));
        assert_eq!(tabs.selection_columns(2, &(9..10)), None);
        let spaces = TextLayout::new("hello   world", 5);
        assert_eq!(spaces.selection_columns(0, &(5..8)), Some(5..6));
    }
}
