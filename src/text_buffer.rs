use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

use crate::text_layout::TextLayout;

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TextBuffer {
    pub(crate) text: String,
    pub(crate) cursor: usize,
    anchor: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl TextBuffer {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self {
            text,
            cursor,
            ..Self::default()
        }
    }

    pub(crate) fn selection(&self) -> Option<Range<usize>> {
        self.anchor
            .filter(|anchor| *anchor != self.cursor)
            .map(|anchor| anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    pub(crate) fn set_cursor(&mut self, byte: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = self.boundary_before(byte);
    }

    pub(crate) fn insert(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let range = self.selection().unwrap_or(self.cursor..self.cursor);
        self.replace(range, text)
    }

    pub(crate) fn replace_all(&mut self, text: &str) -> bool {
        if self.text == text {
            return false;
        }
        self.replace(0..self.text.len(), text)
    }

    fn replace(&mut self, range: Range<usize>, replacement: &str) -> bool {
        let changed = &self.text[range.clone()] != replacement;
        if changed {
            let before = self.snapshot();
            push_snapshot(&mut self.undo, before);
            self.redo.clear();
            self.text.replace_range(range.clone(), replacement);
        }
        // An edit can join neighboring graphemes, such as a base and combining mark.
        self.cursor = self.boundary_after(range.start + replacement.len());
        self.anchor = None;
        changed
    }

    fn boundary_before(&self, byte: usize) -> usize {
        if byte >= self.text.len() {
            return self.text.len();
        }
        self.text
            .grapheme_indices(true)
            .take_while(|(offset, _)| *offset <= byte)
            .last()
            .map_or(0, |(offset, _)| offset)
    }

    fn boundary_after(&self, byte: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find(|(offset, _)| *offset >= byte)
            .map_or(self.text.len(), |(offset, _)| offset)
    }

    fn previous(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .take_while(|(offset, _)| *offset < self.cursor)
            .last()
            .map_or(0, |(offset, _)| offset)
    }

    fn next(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .find(|(offset, _)| *offset > self.cursor)
            .map_or(self.text.len(), |(offset, _)| offset)
    }

    fn previous_word(&self) -> usize {
        let mut previous = self.text[..self.cursor].grapheme_indices(true).rev();
        let Some((mut start, grapheme)) = previous
            .by_ref()
            .find(|(_, grapheme)| word_class(grapheme) != WordClass::Whitespace)
        else {
            return 0;
        };
        let class = word_class(grapheme);
        for (offset, grapheme) in previous {
            if word_class(grapheme) != class {
                break;
            }
            start = offset;
        }
        start
    }

    fn next_word(&self) -> usize {
        let mut following = self.text[self.cursor..].grapheme_indices(true).peekable();
        let Some((_, first)) = following.peek() else {
            return self.cursor;
        };
        let class = word_class(first);
        let mut end = self.cursor;
        while let Some((offset, grapheme)) = following.peek().copied() {
            if word_class(grapheme) != class {
                break;
            }
            end = self.cursor + offset + grapheme.len();
            following.next();
        }
        while let Some((offset, grapheme)) = following.peek().copied() {
            if word_class(grapheme) != WordClass::Whitespace {
                break;
            }
            end = self.cursor + offset + grapheme.len();
            following.next();
        }
        end
    }

    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |at| self.cursor + at)
    }

    pub(crate) fn move_wrapped_vertical(&mut self, down: bool, width: usize, extend: bool) {
        let layout = TextLayout::new(&self.text, width);
        let (row, column) = layout.cursor_position(self.cursor);
        let target = if down {
            row.saturating_add(1).min(layout.lines.len() - 1)
        } else {
            row.saturating_sub(1)
        };
        if target == row {
            if !extend {
                self.anchor = None;
            }
            return;
        }
        self.set_cursor(layout.nearest_cursor(target, column), extend);
    }

    /// Returns whether the text changed. Cursor movement alone is not an edit.
    pub(crate) fn key(&mut self, key: KeyEvent, multiline: bool) -> bool {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let control = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT);
        if control {
            match key.code {
                KeyCode::Char('a' | 'A') => {
                    self.anchor = Some(0);
                    self.cursor = self.text.len();
                    return false;
                }
                KeyCode::Char('z' | 'Z') => {
                    return if shift { self.redo() } else { self.undo() };
                }
                KeyCode::Char('y' | 'Y') => return self.redo(),
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char(character)
                if !character.is_control()
                    && (!key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        || key
                            .modifiers
                            .contains(KeyModifiers::CONTROL | KeyModifiers::ALT)) =>
            {
                self.insert(&character.to_string())
            }
            KeyCode::Enter if multiline => self.insert("\n"),
            KeyCode::Backspace => {
                let range = self.selection().unwrap_or_else(|| {
                    let start = if control {
                        self.previous_word()
                    } else {
                        self.previous()
                    };
                    start..self.cursor
                });
                self.replace(range, "")
            }
            KeyCode::Delete => {
                let range = self.selection().unwrap_or_else(|| {
                    let end = if control {
                        self.next_word()
                    } else {
                        self.next()
                    };
                    self.cursor..end
                });
                self.replace(range, "")
            }
            KeyCode::Left | KeyCode::Right => {
                let left = key.code == KeyCode::Left;
                let target = if let Some(range) = self.selection().filter(|_| !shift) {
                    if left { range.start } else { range.end }
                } else if control {
                    if left {
                        self.previous_word()
                    } else {
                        self.next_word()
                    }
                } else if left {
                    self.previous()
                } else {
                    self.next()
                };
                self.set_cursor(target, shift);
                false
            }
            KeyCode::Home | KeyCode::End => {
                let home = key.code == KeyCode::Home;
                let target = match (home, control) {
                    (true, true) => 0,
                    (false, true) => self.text.len(),
                    (true, false) => self.line_start(),
                    (false, false) => self.line_end(),
                };
                self.set_cursor(target, shift);
                false
            }
            KeyCode::Up | KeyCode::Down if multiline => {
                self.move_wrapped_vertical(key.code == KeyCode::Down, usize::MAX, shift);
                false
            }
            _ => false,
        }
    }

    pub(crate) fn cursor_position(&self) -> (usize, usize) {
        TextLayout::new(&self.text, usize::MAX).cursor_position(self.cursor)
    }

    pub(crate) fn set_cursor_position(&mut self, row: usize, column: usize) {
        let layout = TextLayout::new(&self.text, usize::MAX);
        self.set_cursor(layout.cursor_at(row, column), false);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.cursor = snapshot.cursor;
        self.anchor = snapshot.anchor;
    }

    fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        push_snapshot(&mut self.redo, current);
        self.restore(snapshot);
        true
    }

    fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        push_snapshot(&mut self.undo, current);
        self.restore(snapshot);
        true
    }
}

fn push_snapshot(history: &mut Vec<Snapshot>, snapshot: Snapshot) {
    if history.len() == HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(snapshot);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WordClass {
    Word,
    Whitespace,
    Punctuation,
}

fn word_class(grapheme: &str) -> WordClass {
    if grapheme.chars().all(char::is_whitespace) {
        WordClass::Whitespace
    } else if grapheme
        .chars()
        .any(|character| character.is_alphanumeric() || character == '_')
    {
        WordClass::Word
    } else {
        WordClass::Punctuation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(buffer: &mut TextBuffer, code: KeyCode, modifiers: KeyModifiers) -> bool {
        buffer.key(KeyEvent::new(code, modifiers), true)
    }

    fn control(buffer: &mut TextBuffer, code: KeyCode) -> bool {
        press(buffer, code, KeyModifiers::CONTROL)
    }

    fn assert_grapheme_cursor(buffer: &TextBuffer) {
        let boundaries: Vec<usize> = buffer
            .text
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .chain(std::iter::once(buffer.text.len()))
            .collect();
        assert!(boundaries.contains(&buffer.cursor));
        if let Some(anchor) = buffer.anchor {
            assert!(boundaries.contains(&anchor));
        }
    }

    #[test]
    fn words_punctuation_whitespace_and_unicode_have_distinct_stops() {
        let text = "  æble_2,👩‍💻  次\n";
        let stops = [0, 2, "  æble_2".len(), "  æble_2,👩‍💻  ".len(), text.len()];
        let mut buffer = TextBuffer::new(text);
        buffer.set_cursor(0, false);
        for expected in &stops[1..] {
            control(&mut buffer, KeyCode::Right);
            assert_eq!(buffer.cursor, *expected);
            assert_grapheme_cursor(&buffer);
        }
        let previous = ["  æble_2,👩‍💻  ".len(), "  æble_2".len(), 2, 0];
        for expected in previous {
            control(&mut buffer, KeyCode::Left);
            assert_eq!(buffer.cursor, expected);
            assert_grapheme_cursor(&buffer);
        }
        let mut buffer = TextBuffer::new("Køb æbler");
        assert!(control(&mut buffer, KeyCode::Backspace));
        assert_eq!(buffer.text, "Køb ");
        buffer.set_cursor(0, false);
        assert!(control(&mut buffer, KeyCode::Delete));
        assert!(buffer.text.is_empty());
    }

    #[test]
    fn selection_replaces_whole_graphemes_and_undo_restores_its_direction() {
        let original = "a👨‍👩‍👧‍👦e\u{301}界";
        let mut buffer = TextBuffer::new(original);
        for _ in 0..3 {
            press(&mut buffer, KeyCode::Left, KeyModifiers::SHIFT);
        }
        assert_eq!(buffer.selection(), Some(1..original.len()));
        assert_eq!(buffer.cursor, 1);
        assert_grapheme_cursor(&buffer);
        assert!(buffer.insert("replacement text"));
        assert_eq!(buffer.text, "areplacement text");
        assert!(buffer.selection().is_none());
        assert!(control(&mut buffer, KeyCode::Char('z')));
        assert_eq!(buffer.text, original);
        assert_eq!(buffer.cursor, 1);
        assert_eq!(buffer.anchor, Some(original.len()));
        assert!(control(&mut buffer, KeyCode::Char('y')));
        assert_eq!(buffer.text, "areplacement text");
        assert_eq!(buffer.cursor, buffer.text.len());
        assert!(buffer.selection().is_none());
    }

    #[test]
    fn whole_field_replacement_and_clear_restore_original_selection_on_undo() {
        let mut buffer = TextBuffer::new("æ2026-");
        buffer.set_cursor(buffer.text.len(), false);
        buffer.set_cursor("æ".len(), true);
        let original = buffer.snapshot();
        assert!(buffer.replace_all("2026-11-09"));
        assert_eq!(buffer.text, "2026-11-09");
        assert_eq!(buffer.cursor, buffer.text.len());
        assert_eq!(buffer.selection(), None);
        assert_eq!(buffer.undo.len(), 1);
        let replaced = buffer.snapshot();
        assert!(control(&mut buffer, KeyCode::Char('z')));
        assert_eq!(buffer.snapshot(), original);
        assert!(!control(&mut buffer, KeyCode::Char('z')));
        assert!(control(&mut buffer, KeyCode::Char('y')));
        assert_eq!(buffer.snapshot(), replaced);
        buffer.set_cursor(4, false);
        buffer.set_cursor(7, true);
        let before_clear = buffer.snapshot();
        assert!(buffer.replace_all(""));
        assert!(buffer.text.is_empty());
        assert_eq!(buffer.cursor, 0);
        assert_eq!(buffer.selection(), None);
        assert!(control(&mut buffer, KeyCode::Char('z')));
        assert_eq!(buffer.snapshot(), before_clear);
        assert!(control(&mut buffer, KeyCode::Char('y')));
        assert!(buffer.text.is_empty());
    }

    #[test]
    fn identical_whole_field_replacements_preserve_every_buffer_state() {
        let mut buffer = TextBuffer::new("2026-11-08");
        assert!(buffer.replace_all("2026-11-09"));
        assert!(control(&mut buffer, KeyCode::Char('z')));
        buffer.set_cursor(7, false);
        buffer.set_cursor(4, true);
        let original = buffer.clone();
        assert!(!buffer.replace_all("2026-11-08"));
        assert_eq!(buffer, original);
        assert!(control(&mut buffer, KeyCode::Char('y')));
        assert_eq!(buffer.text, "2026-11-09");
        assert!(control(&mut buffer, KeyCode::Char('z')));
        assert!(buffer.replace_all(""));
        assert!(!control(&mut buffer, KeyCode::Char('y')));
        let empty = buffer.clone();
        assert!(!buffer.replace_all(""));
        assert_eq!(buffer, empty);
    }

    #[test]
    fn selection_keys_cover_lines_fields_and_word_boundaries() {
        let mut buffer = TextBuffer::new("alpha,beta\n界 next");
        press(&mut buffer, KeyCode::Home, KeyModifiers::SHIFT);
        assert_eq!(&buffer.text[buffer.selection().unwrap()], "界 next");
        control(&mut buffer, KeyCode::Home);
        assert_eq!(buffer.cursor, 0);
        assert!(buffer.selection().is_none());
        press(
            &mut buffer,
            KeyCode::Right,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(buffer.selection(), Some(0..5));
        press(&mut buffer, KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(buffer.cursor, 5);
        assert!(buffer.selection().is_none());
        press(
            &mut buffer,
            KeyCode::End,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(buffer.selection(), Some(5..buffer.text.len()));
        control(&mut buffer, KeyCode::Char('a'));
        assert_eq!(buffer.selection(), Some(0..buffer.text.len()));
        assert!(press(&mut buffer, KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(buffer.text, "x");
        control(&mut buffer, KeyCode::Char('z'));
        assert_eq!(buffer.selection(), Some(0..buffer.text.len()));
        assert!(press(&mut buffer, KeyCode::Delete, KeyModifiers::NONE));
        assert!(buffer.text.is_empty());
        assert!(buffer.selection().is_none());
    }

    #[test]
    fn wrapped_vertical_selection_and_mouse_use_display_columns() {
        let mut buffer = TextBuffer::new("abcde fghij klmno");
        buffer.set_cursor(8, false);
        buffer.move_wrapped_vertical(false, 5, true);
        assert_eq!(buffer.cursor, 2);
        assert_eq!(buffer.selection(), Some(2..8));
        buffer.move_wrapped_vertical(true, 5, true);
        assert_eq!(buffer.cursor, 8);
        assert!(buffer.selection().is_none());
        buffer.set_cursor(7, true);
        buffer.move_wrapped_vertical(false, 5, false);
        assert!(buffer.selection().is_none());

        let mut buffer = TextBuffer::new("abc\n\t界x");
        buffer.set_cursor(3, false);
        buffer.move_wrapped_vertical(true, 8, true);
        assert_eq!(buffer.cursor_position(), (1, 4));
        assert_eq!(buffer.selection(), Some(3..5));
        buffer.set_cursor_position(1, 6);
        assert_eq!(buffer.cursor_position(), (1, 6));
        assert!(buffer.selection().is_none());
    }

    #[test]
    fn wrapped_row_boundaries_do_not_move_inside_hidden_spaces() {
        let mut buffer = TextBuffer::new("hello   world");
        buffer.set_cursor(7, false);
        buffer.move_wrapped_vertical(false, 5, true);
        assert_eq!(buffer.cursor, 7);
        assert!(buffer.selection().is_none());
        buffer.set_cursor(buffer.text.len(), false);
        buffer.move_wrapped_vertical(true, 5, false);
        assert_eq!(buffer.cursor, buffer.text.len());
    }

    #[test]
    fn edits_that_join_graphemes_leave_a_valid_cursor() {
        let mut buffer = TextBuffer::new("a\n\u{301}");
        buffer.set_cursor(1, false);
        assert!(press(&mut buffer, KeyCode::Delete, KeyModifiers::NONE));
        assert_eq!(buffer.text, "a\u{301}");
        assert_eq!(buffer.cursor, buffer.text.len());
        assert_grapheme_cursor(&buffer);
        control(&mut buffer, KeyCode::Char('z'));
        assert_eq!(buffer.text, "a\n\u{301}");
        assert_eq!(buffer.cursor, 1);

        let mut buffer = TextBuffer::new("👩👩");
        buffer.set_cursor("👩".len(), false);
        buffer.insert("\u{200d}");
        assert_eq!(buffer.text, "👩‍👩");
        assert_eq!(buffer.cursor, buffer.text.len());
        assert_grapheme_cursor(&buffer);
        press(&mut buffer, KeyCode::Backspace, KeyModifiers::NONE);
        assert!(buffer.text.is_empty());
    }

    #[test]
    fn paste_is_one_undo_step_and_noops_preserve_redo() {
        let mut buffer = TextBuffer::new("original");
        buffer.insert(" long pasted text\nsecond line");
        assert!(control(&mut buffer, KeyCode::Char('z')));
        assert_eq!(buffer.text, "original");
        assert!(!control(&mut buffer, KeyCode::Char('z')));
        assert!(!buffer.insert(""));
        assert!(!press(&mut buffer, KeyCode::Delete, KeyModifiers::NONE));
        press(&mut buffer, KeyCode::Left, KeyModifiers::SHIFT);
        assert!(press(
            &mut buffer,
            KeyCode::Char('Z'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(buffer.text, "original long pasted text\nsecond line");
        control(&mut buffer, KeyCode::Char('z'));
        buffer.insert("!");
        assert!(!control(&mut buffer, KeyCode::Char('y')));
        assert_eq!(buffer.text, "origina!");
    }

    #[test]
    fn history_is_bounded_and_belongs_to_each_buffer() {
        let mut first = TextBuffer::default();
        let mut second = TextBuffer::new("untouched");
        for _ in 0..101 {
            first.insert("x");
        }
        assert_eq!(first.undo.len(), HISTORY_LIMIT);
        for _ in 0..100 {
            assert!(control(&mut first, KeyCode::Char('z')));
        }
        assert_eq!(first.text, "x");
        assert!(!control(&mut first, KeyCode::Char('z')));
        assert!(!control(&mut second, KeyCode::Char('z')));
        assert_eq!(second.text, "untouched");
        for _ in 0..100 {
            assert!(control(&mut first, KeyCode::Char('y')));
        }
        assert_eq!(first.text.len(), 101);
        assert!(!control(&mut first, KeyCode::Char('y')));
    }

    #[test]
    fn altgr_printable_keys_do_not_invoke_control_shortcuts() {
        let mut buffer = TextBuffer::new("text");
        for character in ['a', 'z', '@', '\\'] {
            assert!(press(
                &mut buffer,
                KeyCode::Char(character),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ));
        }
        assert_eq!(buffer.text, "textaz@\\");
        assert!(!press(
            &mut buffer,
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(buffer.text, "textaz@\\");
    }
}
