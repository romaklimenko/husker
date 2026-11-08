use std::io::{self, IsTerminal, Stdout};
use std::panic::PanicHookInfo;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::config::{Action, Config, Mode};
use crate::i18n::{Language, error_message, tr};
use crate::model::{Status, Task, TaskInput, TaskList};
use crate::planning::{Bucket, date_label};
use crate::query::{Query, parse_date, visible_tasks};
use crate::store::Store;
use crate::text_layout::TextLayout;

type PanicHook = Arc<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

const MESSAGE_LIFETIME: Duration = Duration::from_secs(5);

struct TerminalGuard {
    previous_hook: PanicHook,
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableMouseCapture
        ) {
            restore_terminal();
            return Err(error.into());
        }
        let previous_hook: PanicHook = std::panic::take_hook().into();
        let hook = previous_hook.clone();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            hook(info);
        }));
        Ok(Self { previous_hook })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
        if !std::thread::panicking() {
            let hook = self.previous_hook.clone();
            std::panic::set_hook(Box::new(move |info| hook(info)));
        }
    }
}

pub fn run(store: &mut Store, config: &Config) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(tr(config.language, "terminal_required"));
    }
    let mut app = App::new(store, Local::now().date_naive())?;
    let _guard = TerminalGuard::enter()?;
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut refreshed = Instant::now();
    let mut mouse_captured = true;
    loop {
        app.expire_message(Instant::now());
        terminal.draw(|frame| app.draw(frame, config))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if app.handle_key(key, store, config) {
                        break;
                    }
                }
                Event::Paste(text) => app.paste(&text),
                Event::Mouse(mouse) => {
                    if let Err(error) = app.handle_mouse(mouse, Instant::now(), store, config) {
                        app.set_error(error, config);
                    }
                }
                _ => {}
            }
        }
        if mouse_captured != app.wants_mouse_capture() {
            mouse_captured = app.wants_mouse_capture();
            if mouse_captured {
                execute!(io::stdout(), EnableMouseCapture)?;
            } else {
                execute!(io::stdout(), DisableMouseCapture)?;
            }
        }
        if refreshed.elapsed() >= Duration::from_secs(1) {
            if let Err(error) = app.refresh_changes(store, config, Local::now().date_naive(), false)
            {
                app.set_error(error, config);
            }
            refreshed = Instant::now();
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TextBuffer {
    text: String,
    cursor: usize,
}

impl TextBuffer {
    fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self { text, cursor }
    }

    fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(at, _)| at)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map_or(self.cursor, |grapheme| self.cursor + grapheme.len())
    }

    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |at| self.cursor + at)
    }

    fn move_vertical(&mut self, down: bool) {
        let start = self.line_start();
        let column = self.text[start..self.cursor].chars().count();
        let (target_start, target_end) = if down {
            let end = self.line_end();
            if end == self.text.len() {
                return;
            }
            let next_start = end + 1;
            let next_end = self.text[next_start..]
                .find('\n')
                .map_or(self.text.len(), |at| next_start + at);
            (next_start, next_end)
        } else {
            if start == 0 {
                return;
            }
            let previous_end = start - 1;
            let previous_start = self.text[..previous_end].rfind('\n').map_or(0, |at| at + 1);
            (previous_start, previous_end)
        };
        self.cursor = self.text[target_start..target_end]
            .char_indices()
            .nth(column)
            .map_or(target_end, |(at, _)| target_start + at);
    }

    fn move_wrapped_vertical(&mut self, down: bool, width: usize) {
        let layout = TextLayout::new(&self.text, width);
        let (row, column) = layout.cursor_position(self.cursor);
        let target = if down {
            row.saturating_add(1).min(layout.lines.len() - 1)
        } else {
            row.saturating_sub(1)
        };
        if target == row {
            return;
        }
        self.cursor = layout.nearest_cursor(target, column);
    }

    /// Returns whether the text changed. Cursor movement alone is not an edit.
    fn key(&mut self, key: KeyEvent, multiline: bool) -> bool {
        match key.code {
            KeyCode::Char(c)
                if !c.is_control()
                    && (!key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        || key
                            .modifiers
                            .contains(KeyModifiers::CONTROL | KeyModifiers::ALT)) =>
            {
                self.insert(&c.to_string());
                true
            }
            KeyCode::Enter if multiline => {
                self.insert("\n");
                true
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let previous = self.previous();
                self.text.replace_range(previous..self.cursor, "");
                self.cursor = previous;
                true
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                self.text.replace_range(self.cursor..self.next(), "");
                true
            }
            KeyCode::Left => {
                self.cursor = self.previous();
                false
            }
            KeyCode::Right => {
                self.cursor = self.next();
                false
            }
            KeyCode::Home => {
                self.cursor = self.line_start();
                false
            }
            KeyCode::End => {
                self.cursor = self.line_end();
                false
            }
            KeyCode::Up if multiline => {
                self.move_vertical(false);
                false
            }
            KeyCode::Down if multiline => {
                self.move_vertical(true);
                false
            }
            _ => false,
        }
    }

    fn cursor_position(&self) -> (usize, usize) {
        let before = &self.text[..self.cursor];
        let row = before.bytes().filter(|byte| *byte == b'\n').count();
        let column = UnicodeWidthStr::width(&self.text[self.line_start()..self.cursor]);
        (row, column)
    }

    fn set_cursor_position(&mut self, row: usize, column: usize) {
        let mut start = 0;
        let lines: Vec<&str> = self.text.split('\n').collect();
        let row = row.min(lines.len().saturating_sub(1));
        for line in &lines[..row] {
            start += line.len() + 1;
        }
        let line = lines[row];
        let mut width = 0;
        let mut offset = 0;
        for (index, grapheme) in line.grapheme_indices(true) {
            let next = index + grapheme.len();
            let character_width = UnicodeWidthStr::width(&line[index..next]);
            if width + character_width > column {
                break;
            }
            width += character_width;
            offset = next;
        }
        self.cursor = start + offset;
    }
}

const FIELD_KEYS: [&str; 4] = ["title", "description", "due", "list"];

#[derive(Clone, Debug)]
struct TaskEditor {
    original: Option<Task>,
    fields: [TextBuffer; 4],
    active: usize,
}

impl TaskEditor {
    fn new(task: Option<Task>, list_name: &str) -> Self {
        let fields = if let Some(task) = &task {
            [
                TextBuffer::new(&task.title),
                TextBuffer::new(&task.description),
                TextBuffer::new(task.due.map(|date| date.to_string()).unwrap_or_default()),
                TextBuffer::new(list_name),
            ]
        } else {
            [
                TextBuffer::default(),
                TextBuffer::default(),
                TextBuffer::default(),
                TextBuffer::new(list_name),
            ]
        };
        let active = if task.is_some() { 1 } else { 0 };
        Self {
            original: task,
            fields,
            active,
        }
    }

    fn input(&self, lists: &[TaskList], language: Language) -> Result<TaskInput> {
        let date_text = self.fields[2].text.trim();
        let due = if date_text.is_empty() {
            None
        } else {
            let parsed = parse_date(date_text).with_context(|| tr(language, "invalid_date"))?;
            Some(parsed)
        };
        let list_name = self.fields[3].text.trim().to_lowercase();
        let list_id = lists
            .iter()
            .find(|list| list.name.to_lowercase() == list_name)
            .map(|list| list.id)
            .with_context(|| tr(language, "unknown_list"))?;
        Ok(TaskInput {
            list_id,
            title: self.fields[0].text.clone(),
            description: self.fields[1].text.clone(),
            due,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Lists,
    Tasks { list_id: Option<i64>, trash: bool },
}

#[derive(Debug)]
enum Screen {
    Navigation,
    Editor(Box<TaskEditor>),
    Filter(TextBuffer),
    ListName {
        id: Option<i64>,
        buffer: TextBuffer,
    },
    Help,
    CopyText {
        text: String,
        row: u16,
        column: u16,
        previous: Box<Screen>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowIdentity {
    List(i64),
    Task(i64),
    Trash,
}

#[derive(Clone, Copy, Debug)]
enum MouseTarget {
    Row(usize),
    EditorField {
        index: usize,
        scroll_row: usize,
        scroll_column: usize,
    },
    HelpRow(usize),
}

struct TransientMessage {
    text: String,
    is_error: bool,
    created_at: Instant,
}

struct App {
    view: View,
    screen: Screen,
    lists: Vec<TaskList>,
    tasks: Vec<Task>,
    visible: Vec<Task>,
    query: Query,
    filter: String,
    selection: usize,
    list_state: ListState,
    help_state: ListState,
    today: NaiveDate,
    message: Option<TransientMessage>,
    data_version: i64,
    external_update: bool,
    mouse_enabled: bool,
    mouse_targets: Vec<(Rect, MouseTarget)>,
    last_click: Option<(RowIdentity, Instant)>,
    remembered_list: Option<RowIdentity>,
    description_width: usize,
}

impl App {
    fn new(store: &Store, today: NaiveDate) -> Result<Self> {
        let mut app = Self {
            view: View::Tasks {
                list_id: None,
                trash: false,
            },
            screen: Screen::Navigation,
            lists: Vec::new(),
            tasks: Vec::new(),
            visible: Vec::new(),
            query: Query::default(),
            filter: String::new(),
            selection: 0,
            list_state: ListState::default(),
            help_state: ListState::default(),
            today,
            message: None,
            data_version: store.data_version()?,
            external_update: false,
            mouse_enabled: true,
            mouse_targets: Vec::new(),
            last_click: None,
            remembered_list: None,
            description_width: 79,
        };
        app.refresh(store)?;
        Ok(app)
    }

    fn mode(&self) -> Mode {
        match self.screen {
            Screen::Editor(_) => Mode::Editor,
            Screen::Filter(_) => Mode::Filter,
            Screen::ListName { .. } => Mode::ListName,
            Screen::Help => Mode::Help,
            Screen::CopyText { .. } => Mode::CopyText,
            Screen::Navigation => {
                if self.view == View::Lists {
                    Mode::Lists
                } else {
                    Mode::Tasks
                }
            }
        }
    }

    fn refresh(&mut self, store: &Store) -> Result<()> {
        let data_version = store.data_version()?;
        self.external_update |= data_version != self.data_version;
        let selected_task = self.selected_task().map(|task| task.id);
        let selected_list = self.lists.get(self.selection).map(|list| list.id);
        let selected_trash = self.view == View::Lists && self.selection == self.lists.len();
        let lists = store.lists()?;
        let tasks = store.tasks()?;
        self.lists = lists;
        self.tasks = tasks;
        if let View::Tasks { list_id, trash } = self.view {
            self.visible = visible_tasks(
                &self.tasks,
                &self.lists,
                &self.query,
                list_id,
                trash,
                self.today,
            );
            if let Some(index) = self
                .visible
                .iter()
                .position(|task| Some(task.id) == selected_task)
            {
                self.selection = index;
            }
        } else if selected_trash {
            self.selection = self.lists.len();
        } else if let Some(index) = self
            .lists
            .iter()
            .position(|list| Some(list.id) == selected_list)
        {
            self.selection = index;
        }
        self.clamp_selection();
        self.data_version = data_version;
        Ok(())
    }

    fn refresh_changes(
        &mut self,
        store: &Store,
        config: &Config,
        today: NaiveDate,
        manual: bool,
    ) -> Result<()> {
        let previous_version = self.data_version;
        let external = store.data_version()? != self.data_version;
        let date_changed = self.today != today;
        self.today = today;
        if external || date_changed || manual {
            self.refresh(store)?;
        }
        if self.data_version != previous_version {
            self.external_update = true;
        } else if manual {
            self.external_update = false;
        }
        if manual {
            self.show_message(tr(config.language, "refreshed"), false, Instant::now());
        }
        Ok(())
    }

    fn wants_mouse_capture(&self) -> bool {
        self.mouse_enabled && !matches!(self.screen, Screen::CopyText { .. })
    }

    fn open_copy_text(&mut self) {
        let text = match &self.screen {
            Screen::Editor(editor) => {
                format!("{}\n\n{}", editor.fields[0].text, editor.fields[1].text)
            }
            Screen::Navigation => {
                let Some(task) = self.selected_task() else {
                    return;
                };
                format!("{}\n\n{}", task.title, task.description)
            }
            _ => return,
        };
        let previous = Box::new(std::mem::replace(&mut self.screen, Screen::Navigation));
        self.screen = Screen::CopyText {
            text,
            row: 0,
            column: 0,
            previous,
        };
        self.last_click = None;
    }

    fn row_identity(&self, index: usize) -> Option<RowIdentity> {
        if self.view == View::Lists {
            self.lists
                .get(index)
                .map(|list| RowIdentity::List(list.id))
                .or_else(|| (index == self.lists.len()).then_some(RowIdentity::Trash))
        } else {
            self.visible
                .get(index)
                .map(|task| RowIdentity::Task(task.id))
        }
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        now: Instant,
        store: &mut Store,
        _config: &Config,
    ) -> Result<()> {
        if !self.wants_mouse_capture() || !mouse.modifiers.is_empty() {
            return Ok(());
        }
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            self.last_click = None;
            let down = mouse.kind == MouseEventKind::ScrollDown;
            match self.screen {
                Screen::Navigation => {
                    self.selection = if down {
                        self.selection.saturating_add(1)
                    } else {
                        self.selection.saturating_sub(1)
                    };
                    self.clamp_selection();
                }
                Screen::Help => {
                    let selected = self.help_state.selected().unwrap_or(0);
                    self.help_state.select(Some(if down {
                        selected.saturating_add(1).min(Action::ALL.len() - 1)
                    } else {
                        selected.saturating_sub(1)
                    }));
                }
                _ => {}
            }
            return Ok(());
        }
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Ok(());
        }
        let hit = self
            .mouse_targets
            .iter()
            .find(|(rect, _)| {
                mouse.column >= rect.x
                    && mouse.column < rect.right()
                    && mouse.row >= rect.y
                    && mouse.row < rect.bottom()
            })
            .copied();
        let Some((rect, target)) = hit else {
            self.last_click = None;
            return Ok(());
        };
        match target {
            MouseTarget::Row(index) if matches!(self.screen, Screen::Navigation) => {
                let Some(identity) = self.row_identity(index) else {
                    return Ok(());
                };
                let double_click = self.last_click.is_some_and(|(previous, at)| {
                    previous == identity
                        && now.saturating_duration_since(at) <= Duration::from_millis(400)
                });
                self.selection = index;
                self.clamp_selection();
                if double_click {
                    self.last_click = None;
                    self.open_selection(store)?;
                } else {
                    self.last_click = Some((identity, now));
                }
            }
            MouseTarget::EditorField {
                index,
                scroll_row,
                scroll_column,
            } => {
                self.last_click = None;
                if let Screen::Editor(editor) = &mut self.screen {
                    editor.active = index;
                    let row = scroll_row + usize::from(mouse.row - rect.y);
                    let column = scroll_column + usize::from(mouse.column - rect.x);
                    if index == 1 {
                        let layout = TextLayout::new(
                            &editor.fields[index].text,
                            usize::from(rect.width.saturating_sub(1)),
                        );
                        editor.fields[index].cursor = layout.cursor_at(row, column);
                    } else {
                        editor.fields[index].set_cursor_position(row, column);
                    }
                }
            }
            MouseTarget::HelpRow(index) if matches!(self.screen, Screen::Help) => {
                self.last_click = None;
                self.help_state.select(Some(index));
            }
            _ => {}
        }
        Ok(())
    }

    fn row_count(&self) -> usize {
        if self.view == View::Lists {
            self.lists.len() + 1
        } else {
            self.visible.len()
        }
    }

    fn clamp_selection(&mut self) {
        self.selection = self.selection.min(self.row_count().saturating_sub(1));
        if self.view == View::Lists {
            self.list_state.select(Some(self.selection));
        }
    }

    fn selected_task(&self) -> Option<&Task> {
        if self.view == View::Lists {
            None
        } else {
            self.visible.get(self.selection)
        }
    }

    fn show_message(&mut self, text: impl Into<String>, is_error: bool, now: Instant) {
        self.message = Some(TransientMessage {
            text: text.into(),
            is_error,
            created_at: now,
        });
    }

    fn expire_message(&mut self, now: Instant) {
        if self.message.as_ref().is_some_and(|message| {
            now.saturating_duration_since(message.created_at) >= MESSAGE_LIFETIME
        }) {
            self.message = None;
        }
    }

    fn set_error(&mut self, error: anyhow::Error, config: &Config) {
        self.show_message(
            format!(
                "{}: {}",
                tr(config.language, "error"),
                error_message(config.language, &error)
            ),
            true,
            Instant::now(),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, store: &mut Store, config: &Config) -> bool {
        match self.dispatch_key(key, store, config) {
            Ok(quit) => quit,
            Err(error) => {
                self.set_error(error, config);
                false
            }
        }
    }

    fn dispatch_key(&mut self, key: KeyEvent, store: &mut Store, config: &Config) -> Result<bool> {
        let action = config.action(key, self.mode());
        match action {
            Some(Action::Refresh) => {
                self.refresh_changes(store, config, Local::now().date_naive(), true)?;
                return Ok(false);
            }
            Some(Action::ToggleMouse) => {
                self.mouse_enabled = !self.mouse_enabled;
                self.last_click = None;
                self.show_message(
                    tr(
                        config.language,
                        if self.mouse_enabled {
                            "mouse_on"
                        } else {
                            "mouse_off"
                        },
                    ),
                    false,
                    Instant::now(),
                );
                return Ok(false);
            }
            Some(Action::CopyText) => {
                self.open_copy_text();
                return Ok(false);
            }
            _ => {}
        }
        match &mut self.screen {
            Screen::CopyText {
                text, row, column, ..
            } => {
                match action {
                    Some(Action::Close) => {
                        let Screen::CopyText { previous, .. } =
                            std::mem::replace(&mut self.screen, Screen::Navigation)
                        else {
                            unreachable!()
                        };
                        self.screen = *previous;
                        self.refresh(store)?;
                    }
                    Some(Action::Up) => *row = row.saturating_sub(1),
                    Some(Action::Down) => {
                        *row = row.saturating_add(1).min(
                            text.lines()
                                .count()
                                .saturating_sub(1)
                                .min(u16::MAX as usize) as u16,
                        );
                    }
                    Some(Action::ScrollLeft) => *column = column.saturating_sub(1),
                    Some(Action::ScrollRight) => {
                        let widest = text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
                        *column = column
                            .saturating_add(1)
                            .min(widest.saturating_sub(1).min(u16::MAX as usize) as u16);
                    }
                    _ => {}
                }
                return Ok(false);
            }
            Screen::Editor(editor) => {
                match action {
                    Some(Action::Save) => self.save_editor(store, config)?,
                    Some(Action::Discard) => {
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                    }
                    Some(Action::NextField) => {
                        editor.active = (editor.active + 1) % FIELD_KEYS.len()
                    }
                    Some(Action::PreviousField) => {
                        editor.active = (editor.active + FIELD_KEYS.len() - 1) % FIELD_KEYS.len()
                    }
                    _ => {
                        if editor.active == 1 && matches!(key.code, KeyCode::Up | KeyCode::Down) {
                            editor.fields[1].move_wrapped_vertical(
                                key.code == KeyCode::Down,
                                self.description_width,
                            );
                        } else {
                            editor.fields[editor.active].key(key, editor.active == 1);
                        }
                    }
                }
                return Ok(false);
            }
            Screen::Filter(buffer) => {
                match action {
                    Some(Action::Apply) => {
                        let query = Query::parse(&buffer.text)?;
                        self.filter = buffer.text.clone();
                        self.query = query;
                        self.selection = 0;
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                        self.message = None;
                    }
                    Some(Action::Discard | Action::Back) => {
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                    }
                    _ => {
                        buffer.key(key, false);
                    }
                }
                return Ok(false);
            }
            Screen::ListName { id, buffer } => {
                match action {
                    Some(Action::Apply) => {
                        let list = if let Some(id) = id {
                            store.rename_list(*id, &buffer.text)?
                        } else {
                            store.create_list(&buffer.text)?
                        };
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                        if let Some(index) = self.lists.iter().position(|value| value.id == list.id)
                        {
                            self.selection = index;
                            self.clamp_selection();
                        }
                        self.show_message(tr(config.language, "saved"), false, Instant::now());
                    }
                    Some(Action::Discard | Action::Back) => {
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                    }
                    _ => {
                        buffer.key(key, false);
                    }
                }
                return Ok(false);
            }
            Screen::Help => {
                match action {
                    Some(Action::Close | Action::Back) => {
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                    }
                    Some(Action::Up) => self.help_state.select(Some(
                        self.help_state.selected().unwrap_or(0).saturating_sub(1),
                    )),
                    Some(Action::Down) => self.help_state.select(Some(
                        (self.help_state.selected().unwrap_or(0) + 1).min(Action::ALL.len() - 1),
                    )),
                    _ => {}
                }
                return Ok(false);
            }
            Screen::Navigation => {}
        }
        let Some(action) = action else {
            return Ok(false);
        };
        match action {
            Action::Quit => return Ok(true),
            Action::Up => {
                self.selection = self.selection.saturating_sub(1);
                self.clamp_selection();
            }
            Action::Down => {
                self.selection = self.selection.saturating_add(1);
                self.clamp_selection();
            }
            Action::Lists => self.set_view(View::Lists, store)?,
            Action::Planned => self.set_view(
                View::Tasks {
                    list_id: None,
                    trash: false,
                },
                store,
            )?,
            Action::Back
                if matches!(
                    self.view,
                    View::Tasks {
                        list_id: Some(_),
                        ..
                    } | View::Tasks { trash: true, .. }
                ) =>
            {
                self.set_view(View::Lists, store)?
            }
            Action::Open => self.open_selection(store)?,
            Action::NewTask if !matches!(self.view, View::Tasks { trash: true, .. }) => {
                self.new_task()
            }
            Action::NewList if self.view == View::Lists => {
                self.screen = Screen::ListName {
                    id: None,
                    buffer: TextBuffer::default(),
                }
            }
            Action::RenameList if self.view == View::Lists => {
                if let Some(list) = self.lists.get(self.selection) {
                    self.screen = Screen::ListName {
                        id: Some(list.id),
                        buffer: TextBuffer::new(&list.name),
                    };
                }
            }
            Action::MoveUp | Action::MoveDown if self.view == View::Lists => {
                if let Some(list) = self.lists.get(self.selection) {
                    store.move_list(list.id, action == Action::MoveDown)?;
                    self.refresh(store)?;
                }
            }
            Action::Filter if self.view != View::Lists => {
                self.screen = Screen::Filter(TextBuffer::new(&self.filter))
            }
            Action::Help => {
                self.screen = Screen::Help;
                self.help_state.select(Some(0));
            }
            _ => self.task_action(action, store, config)?,
        }
        Ok(false)
    }

    fn paste(&mut self, text: &str) {
        match &mut self.screen {
            Screen::Editor(editor) => {
                let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                let normalized = if editor.active == 1 {
                    normalized
                } else {
                    normalized.replace('\n', " ")
                };
                editor.fields[editor.active].insert(&normalized);
            }
            Screen::Filter(buffer) | Screen::ListName { buffer, .. } => {
                buffer.insert(&text.replace(['\r', '\n'], " "))
            }
            _ => {}
        }
    }

    fn set_view(&mut self, view: View, store: &Store) -> Result<()> {
        self.remembered_list = match self.view {
            View::Lists => self
                .lists
                .get(self.selection)
                .map(|list| RowIdentity::List(list.id))
                .or(Some(RowIdentity::Trash)),
            View::Tasks { trash: true, .. } => Some(RowIdentity::Trash),
            View::Tasks {
                list_id: Some(id), ..
            } => Some(RowIdentity::List(id)),
            _ => self.remembered_list,
        };
        self.view = view;
        self.selection = 0;
        self.list_state = ListState::default();
        self.visible.clear();
        self.last_click = None;
        self.filter.clear();
        self.query = Query::default();
        self.refresh(store)?;
        if view == View::Lists {
            self.selection = match self.remembered_list {
                Some(RowIdentity::List(id)) => self
                    .lists
                    .iter()
                    .position(|list| list.id == id)
                    .unwrap_or(0),
                Some(RowIdentity::Trash) => self.lists.len(),
                _ => 0,
            };
            self.clamp_selection();
        }
        Ok(())
    }

    fn open_selection(&mut self, store: &Store) -> Result<()> {
        if self.view == View::Lists {
            let view = if let Some(list) = self.lists.get(self.selection) {
                View::Tasks {
                    list_id: Some(list.id),
                    trash: false,
                }
            } else {
                View::Tasks {
                    list_id: None,
                    trash: true,
                }
            };
            self.set_view(view, store)?;
        } else if let Some(task) = self.selected_task().filter(|task| !task.deleted).cloned() {
            let list_name = self
                .lists
                .iter()
                .find(|list| list.id == task.list_id)
                .map_or("", |list| list.name.as_str());
            self.screen = Screen::Editor(Box::new(TaskEditor::new(Some(task), list_name)));
            self.message = None;
        }
        Ok(())
    }

    fn new_task(&mut self) {
        let list = match self.view {
            View::Tasks {
                list_id: Some(id), ..
            } => self.lists.iter().find(|list| list.id == id),
            View::Lists => self
                .lists
                .get(self.selection)
                .or_else(|| self.lists.iter().min_by_key(|list| list.id)),
            _ => self.lists.iter().min_by_key(|list| list.id),
        };
        if let Some(list) = list {
            self.screen = Screen::Editor(Box::new(TaskEditor::new(None, &list.name)));
            self.message = None;
        }
    }

    fn save_editor(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Screen::Editor(editor) = &self.screen else {
            return Ok(());
        };
        let input = editor.input(&store.lists()?, config.language)?;
        let task = if let Some(original) = &editor.original {
            store.update_task(original.id, original.version, &input)?
        } else {
            store.create_task(&input)?
        };
        self.screen = Screen::Navigation;
        self.refresh(store)?;
        if let Some(index) = self
            .visible
            .iter()
            .position(|visible| visible.id == task.id)
        {
            self.selection = index;
            self.clamp_selection();
        }
        self.show_message(tr(config.language, "saved"), false, Instant::now());
        Ok(())
    }

    fn task_action(&mut self, action: Action, store: &mut Store, config: &Config) -> Result<()> {
        let Some(task) = self.selected_task().cloned() else {
            return Ok(());
        };
        if task.deleted && !matches!(action, Action::Restore) {
            return Ok(());
        }
        match action {
            Action::ToggleDone => {
                let status = if task.status == Status::Done {
                    Status::Open
                } else {
                    Status::Done
                };
                store.set_status(task.id, task.version, status)?;
            }
            Action::ToggleCancelled => {
                let status = if task.status == Status::Cancelled {
                    Status::Open
                } else {
                    Status::Cancelled
                };
                store.set_status(task.id, task.version, status)?;
            }
            Action::Trash => {
                store.set_deleted(task.id, task.version, true)?;
            }
            Action::Restore if task.deleted => {
                store.set_deleted(task.id, task.version, false)?;
            }
            Action::MoveUp | Action::MoveDown => {
                let index = if matches!(action, Action::MoveUp) {
                    self.selection.checked_sub(1)
                } else {
                    self.selection.checked_add(1)
                };
                let Some(other) = index.and_then(|index| self.visible.get(index)) else {
                    return Ok(());
                };
                if task.due != other.due {
                    return Ok(());
                }
                store.swap_positions(&task, other)?;
            }
            _ => return Ok(()),
        }
        self.refresh(store)?;
        self.show_message(tr(config.language, "updated"), false, Instant::now());
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>, config: &Config) {
        self.mouse_targets.clear();
        let area = frame.area();
        let language = config.language;
        if area.width < 38
            || area.height < 10
            || (matches!(self.screen, Screen::Editor(_)) && area.height < 18)
        {
            frame.render_widget(
                Paragraph::new(tr(language, "terminal_too_small")).wrap(Wrap { trim: false }),
                area,
            );
            return;
        }
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(area);
        let view_title = match self.view {
            View::Lists => tr(language, "lists").to_owned(),
            View::Tasks { trash: true, .. } => tr(language, "trash").to_owned(),
            View::Tasks {
                list_id: Some(id), ..
            } => self.lists.iter().find(|list| list.id == id).map_or_else(
                || tr(language, "planned").to_owned(),
                |list| list.name.clone(),
            ),
            _ => tr(language, "planned").to_owned(),
        };
        let header = format!(
            "Husker  |  {view_title}  |  {}",
            date_label(self.today, language)
        );
        frame.render_widget(
            Paragraph::new(header)
                .block(Block::default().borders(Borders::BOTTOM))
                .style(Style::default().fg(Color::Cyan)),
            sections[0],
        );
        match &self.screen {
            Screen::Editor(editor) => {
                self.mouse_targets = draw_editor(frame, sections[1], editor, language);
                if let Some((rect, _)) = self
                    .mouse_targets
                    .iter()
                    .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 1, .. }))
                {
                    self.description_width = usize::from(rect.width.saturating_sub(1));
                }
            }
            Screen::Help => {
                let help_area = draw_help(frame, sections[1], config, &mut self.help_state);
                for (row, index) in (self.help_state.offset()..Action::ALL.len())
                    .take(usize::from(help_area.height))
                    .enumerate()
                {
                    self.mouse_targets.push((
                        Rect::new(help_area.x, help_area.y + row as u16, help_area.width, 1),
                        MouseTarget::HelpRow(index),
                    ));
                }
            }
            Screen::CopyText {
                text, row, column, ..
            } => {
                frame.render_widget(
                    Paragraph::new(text.as_str()).scroll((*row, *column)),
                    sections[1],
                );
            }
            Screen::ListName { id, buffer } => {
                let title = tr(
                    language,
                    if id.is_some() {
                        "rename_list"
                    } else {
                        "new_list"
                    },
                );
                let _ = draw_input(
                    frame,
                    Rect {
                        height: 3,
                        ..sections[1]
                    },
                    title,
                    buffer,
                    true,
                );
            }
            _ => {
                let body = if matches!(self.screen, Screen::Filter(_)) || !self.filter.is_empty() {
                    let parts = Layout::vertical([Constraint::Length(3), Constraint::Min(1)])
                        .split(sections[1]);
                    if let Screen::Filter(buffer) = &self.screen {
                        let _ = draw_input(frame, parts[0], tr(language, "filter"), buffer, true);
                    } else {
                        frame.render_widget(
                            Paragraph::new(self.filter.as_str()).block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title(tr(language, "filter")),
                            ),
                            parts[0],
                        );
                    }
                    parts[1]
                } else {
                    sections[1]
                };
                self.draw_navigation(frame, body, language);
            }
        }
        let hint = match self.screen {
            Screen::Editor(_) => format!(
                "{} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Save),
                tr(language, "action_save"),
                config.keys_for(Action::Discard),
                tr(language, "action_discard"),
                config.keys_for(Action::CopyText),
                tr(language, "action_copy_text")
            ),
            Screen::CopyText { .. } => format!(
                "{}  |  {} {}",
                tr(language, "copy_hint"),
                config.keys_for(Action::Close),
                tr(language, "action_close"),
            ),
            Screen::Filter(_) | Screen::ListName { .. } => format!(
                "{} {}  |  {} {}",
                config.keys_for(Action::Apply),
                tr(language, "action_apply"),
                config.keys_for(Action::Back),
                tr(language, "action_discard")
            ),
            Screen::Help => format!(
                "{} / {}  |  {} {}",
                config.keys_for(Action::Up),
                config.keys_for(Action::Down),
                config.keys_for(Action::Close),
                tr(language, "action_close")
            ),
            Screen::Navigation if self.view == View::Lists => format!(
                "{} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Help),
                tr(language, "help"),
                config.keys_for(Action::NewList),
                tr(language, "new_list"),
                config.keys_for(Action::Quit),
                tr(language, "action_quit")
            ),
            Screen::Navigation if matches!(self.view, View::Tasks { trash: true, .. }) => format!(
                "{} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Help),
                tr(language, "help"),
                config.keys_for(Action::Restore),
                tr(language, "action_restore"),
                config.keys_for(Action::Quit),
                tr(language, "action_quit")
            ),
            _ => format!(
                "{} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Help),
                tr(language, "help"),
                config.keys_for(Action::NewTask),
                tr(language, "new_task"),
                config.keys_for(Action::Quit),
                tr(language, "action_quit")
            ),
        };
        let mut footer = vec![Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        ))];
        let external_notice = format!(
            "{}  {} {}",
            tr(language, "external_update"),
            config.keys_for(Action::Refresh),
            tr(language, "action_refresh")
        );
        let notice = if self
            .message
            .as_ref()
            .is_some_and(|message| message.is_error)
        {
            self.message
                .as_ref()
                .map(|message| (message.text.as_str(), message.is_error))
        } else if self.external_update {
            Some((external_notice.as_str(), false))
        } else {
            self.message
                .as_ref()
                .map(|message| (message.text.as_str(), message.is_error))
        };
        if let Some((message, error)) = notice {
            footer.push(Line::from(Span::styled(
                message,
                Style::default().fg(if error { Color::Red } else { Color::Green }),
            )));
        }
        frame.render_widget(
            Paragraph::new(footer).block(Block::default().borders(Borders::TOP)),
            sections[2],
        );
    }

    fn draw_navigation(&mut self, frame: &mut Frame<'_>, area: Rect, language: Language) {
        if self.view == View::Lists {
            let mut rows: Vec<ListItem<'_>> = self
                .lists
                .iter()
                .map(|list| ListItem::new(format!("{} ({})", list.name, list.count)))
                .collect();
            rows.push(ListItem::new(format!(
                "{} ({})",
                tr(language, "trash"),
                self.tasks.iter().filter(|task| task.deleted).count()
            )));
            frame.render_stateful_widget(
                List::new(rows)
                    .highlight_style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> "),
                area,
                &mut self.list_state,
            );
            for (row, index) in (self.list_state.offset()..self.row_count())
                .take(usize::from(area.height))
                .enumerate()
            {
                self.mouse_targets.push((
                    Rect::new(area.x, area.y + row as u16, area.width, 1),
                    MouseTarget::Row(index),
                ));
            }
            return;
        }
        if self.visible.is_empty() {
            self.list_state.select(None);
            frame.render_widget(Paragraph::new(tr(language, "no_tasks")), area);
            return;
        }
        let trash = matches!(self.view, View::Tasks { trash: true, .. });
        let mut previous_bucket = None;
        let mut rows = Vec::new();
        let mut row_tasks = Vec::new();
        for (index, task) in self.visible.iter().enumerate() {
            let bucket = Bucket::for_task(task, self.today);
            if !trash && previous_bucket != Some(bucket) {
                rows.push(ListItem::new(Line::from(Span::styled(
                    tr(language, bucket.key()),
                    Style::default().fg(Color::Yellow),
                ))));
                row_tasks.push(None);
            }
            previous_bucket = Some(bucket);
            if index == self.selection {
                self.list_state.select(Some(rows.len()));
            }
            let marker = match task.status {
                Status::Open => "[ ]",
                Status::Done => "[x]",
                Status::Cancelled => "[-]",
            };
            let date = task
                .due
                .map(|date| format!("  {}", date_label(date, language)))
                .unwrap_or_default();
            let title = task.title.replace(['\r', '\n'], " ");
            rows.push(ListItem::new(Line::from(format!("{marker} {title}{date}"))));
            row_tasks.push(Some(index));
        }
        frame.render_stateful_widget(
            List::new(rows)
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> "),
            area,
            &mut self.list_state,
        );
        for (row, task_index) in row_tasks
            .iter()
            .skip(self.list_state.offset())
            .take(usize::from(area.height))
            .enumerate()
        {
            if let Some(index) = task_index {
                self.mouse_targets.push((
                    Rect::new(area.x, area.y + row as u16, area.width, 1),
                    MouseTarget::Row(*index),
                ));
            }
        }
    }
}

fn draw_input(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    buffer: &TextBuffer,
    active: bool,
) -> (Rect, usize, usize) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .title(title)
        .border_style(Style::default().fg(if active { Color::Cyan } else { Color::DarkGray }));
    let inner = block.inner(area);
    let (row, column) = buffer.cursor_position();
    let scroll_y = row.saturating_sub(inner.height.saturating_sub(1) as usize);
    let scroll_x = column.saturating_sub(inner.width.saturating_sub(1) as usize);
    frame.render_widget(
        Paragraph::new(buffer.text.as_str()).block(block).scroll((
            scroll_y.min(u16::MAX as usize) as u16,
            scroll_x.min(u16::MAX as usize) as u16,
        )),
        area,
    );
    if active && inner.width > 0 && inner.height > 0 {
        frame.set_cursor_position((
            inner.x + (column - scroll_x) as u16,
            inner.y + (row - scroll_y) as u16,
        ));
    }
    (inner, scroll_y, scroll_x)
}

fn draw_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    editor: &TaskEditor,
    language: Language,
) -> Vec<(Rect, MouseTarget)> {
    let fields = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .split(area);
    let mut targets = Vec::with_capacity(FIELD_KEYS.len());
    for (index, key) in FIELD_KEYS.iter().enumerate() {
        let (inner, scroll_row, scroll_column) = if index == 1 {
            draw_description(
                frame,
                fields[index],
                tr(language, key),
                &editor.fields[index],
                index == editor.active,
            )
        } else {
            draw_input(
                frame,
                fields[index],
                tr(language, key),
                &editor.fields[index],
                index == editor.active,
            )
        };
        targets.push((
            inner,
            MouseTarget::EditorField {
                index,
                scroll_row,
                scroll_column,
            },
        ));
    }
    targets
}

fn draw_description(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    buffer: &TextBuffer,
    active: bool,
) -> (Rect, usize, usize) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .title(title)
        .border_style(Style::default().fg(if active { Color::Cyan } else { Color::DarkGray }));
    let inner = block.inner(area);
    // Keep one cell for an insertion cursor after a full visual line.
    let layout = TextLayout::new(&buffer.text, usize::from(inner.width.saturating_sub(1)));
    let (row, column) = layout.cursor_position(buffer.cursor);
    let scroll_row = row.saturating_sub(usize::from(inner.height.saturating_sub(1)));
    let lines: Vec<Line<'_>> = layout
        .lines
        .iter()
        .skip(scroll_row)
        .take(usize::from(inner.height))
        .map(|line| Line::from(line.text.as_str()))
        .collect();
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
    if active && inner.width > 0 && inner.height > 0 {
        frame.set_cursor_position((
            inner.x + column.min(usize::from(inner.width - 1)) as u16,
            inner.y + (row - scroll_row) as u16,
        ));
    }
    (inner, scroll_row, 0)
}

fn draw_help(frame: &mut Frame<'_>, area: Rect, config: &Config, state: &mut ListState) -> Rect {
    let rows: Vec<ListItem<'_>> = Action::ALL
        .iter()
        .map(|action| {
            let label = tr(config.language, &format!("action_{}", action.name()));
            ListItem::new(format!("{:<20} {label}", config.keys_for(*action)))
        })
        .collect();
    let block = Block::default().title(tr(config.language, "help"));
    let inner = block.inner(area);
    frame.render_stateful_widget(
        List::new(rows)
            .block(block)
            .highlight_style(Style::default().fg(Color::Cyan))
            .highlight_symbol("> "),
        area,
        state,
    );
    inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use tempfile::TempDir;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 8).unwrap()
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn fixture() -> (TempDir, Store, Config) {
        let directory = TempDir::new().unwrap();
        let store = Store::open(&directory.path().join("tasks.db")).unwrap();
        let config = Config::load(&directory.path().join("config.toml"), false).unwrap();
        (directory, store, config)
    }

    fn input(list_id: i64, title: &str) -> TaskInput {
        TaskInput {
            list_id,
            title: title.to_owned(),
            description: String::new(),
            due: None,
        }
    }

    fn rendered(app: &mut App, config: &Config, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| app.draw(frame, config)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn action_message_expires_at_five_seconds_without_input() {
        let (_directory, store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        let now = Instant::now();
        app.show_message(tr(config.language, "saved"), false, now);
        app.expire_message(now + Duration::from_millis(4_999));
        assert!(rendered(&mut app, &config, 100, 24).contains(tr(config.language, "saved")));
        app.expire_message(now + Duration::from_secs(5));
        assert!(app.message.is_none());
        assert!(!rendered(&mut app, &config, 100, 24).contains(tr(config.language, "saved")));
    }

    #[test]
    fn replacing_a_message_starts_its_own_timeout() {
        let (_directory, store, _config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        let now = Instant::now();
        app.show_message("Saved", false, now);
        app.show_message("Updated", false, now + Duration::from_secs(4));
        app.expire_message(now + Duration::from_secs(5));
        assert_eq!(app.message.as_ref().unwrap().text, "Updated");
        app.expire_message(now + Duration::from_millis(8_999));
        assert!(app.message.is_some());
        app.expire_message(now + Duration::from_secs(9));
        assert!(app.message.is_none());
    }

    #[test]
    fn navigation_does_not_extend_message_timeout_or_change_on_expiry() {
        let (_directory, mut store, config) = fixture();
        let list = store.create_list("Work").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let now = Instant::now();
        app.show_message("Saved", false, now);
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.message.as_ref().unwrap().created_at, now);
        let view = app.view;
        let selection = app.selection;
        assert_eq!(
            view,
            View::Tasks {
                list_id: Some(list.id),
                trash: false
            }
        );
        app.expire_message(now + Duration::from_secs(5));
        assert!(app.message.is_none());
        assert_eq!(app.view, view);
        assert_eq!(app.selection, selection);
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn external_notice_survives_routine_and_error_message_expiration() {
        let (_directory, store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        let now = Instant::now();
        app.external_update = true;
        app.show_message("Saved", false, now);
        app.expire_message(now + Duration::from_secs(5));
        assert!(app.external_update);
        assert!(
            rendered(&mut app, &config, 120, 24).contains(tr(config.language, "external_update"))
        );
        app.show_message("Temporary error", true, now + Duration::from_secs(6));
        app.expire_message(now + Duration::from_millis(10_999));
        let screen = rendered(&mut app, &config, 120, 24);
        assert!(screen.contains("Temporary error"));
        assert!(!screen.contains(tr(config.language, "external_update")));
        app.expire_message(now + Duration::from_secs(11));
        assert!(app.message.is_none());
        assert!(
            rendered(&mut app, &config, 120, 24).contains(tr(config.language, "external_update"))
        );
        app.expire_message(now + Duration::from_secs(60));
        assert!(app.external_update);
    }

    #[test]
    fn expiring_invalid_date_error_preserves_every_editor_field() {
        let (_directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!("Expected the task editor");
        };
        editor.fields[0] = TextBuffer::new("Changed title");
        editor.fields[1] = TextBuffer::new("Unsaved description\nSecond line");
        editor.fields[2] = TextBuffer::new("not-a-date");
        editor.active = 2;
        editor.fields[2].cursor = 3;
        let draft = editor.clone();
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let message = app.message.as_ref().unwrap();
        assert!(message.is_error);
        let now = message.created_at;
        app.expire_message(now + Duration::from_secs(5));
        assert!(app.message.is_none());
        let Screen::Editor(editor) = &app.screen else {
            panic!("Expected the task editor to remain open");
        };
        assert_eq!(editor.active, draft.active);
        assert_eq!(editor.original, draft.original);
        for (field, original) in editor.fields.iter().zip(&draft.fields) {
            assert_eq!(field.text, original.text);
            assert_eq!(field.cursor, original.cursor);
        }
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn unicode_cursor_edits_complete_characters() {
        let mut buffer = TextBuffer::new("æø界");
        buffer.key(key(KeyCode::Left), false);
        buffer.key(key(KeyCode::Backspace), false);
        assert_eq!(buffer.text, "æ界");
        assert_eq!(buffer.cursor, "æ".len());
        buffer.key(key(KeyCode::Delete), false);
        buffer.key(key(KeyCode::Char('å')), false);
        assert_eq!(buffer.text, "æå");
        assert_eq!(buffer.cursor_position(), (0, 2));
    }

    #[test]
    fn printable_altgr_characters_are_kept_without_inserting_control_shortcuts() {
        let mut buffer = TextBuffer::default();
        buffer.key(
            KeyEvent::new(
                KeyCode::Char('@'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            false,
        );
        buffer.key(
            KeyEvent::new(
                KeyCode::Char('\\'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            false,
        );
        buffer.key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            false,
        );
        buffer.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT), false);
        assert_eq!(buffer.text, "@\\");
    }

    #[test]
    fn multiline_cursor_moves_by_characters_and_handles_short_lines() {
        let mut buffer = TextBuffer::new("æble\nø\nbanan");
        buffer.key(key(KeyCode::Up), true);
        assert_eq!(buffer.cursor, "æble\nø".len());
        buffer.key(key(KeyCode::Up), true);
        assert_eq!(buffer.cursor, "æ".len());
        buffer.key(key(KeyCode::Home), true);
        buffer.key(key(KeyCode::Char('Å')), true);
        assert!(buffer.text.starts_with("Åæble"));
    }

    #[test]
    fn editor_saves_multiline_text_and_invalid_date_keeps_buffer() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Køb mælk");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.paste("To kartoner\nHavremælk");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.paste("2026-02-30");
        app.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(store.tasks().unwrap().is_empty());
        if let Screen::Editor(editor) = &mut app.screen {
            editor.fields[2] = TextBuffer::new("2026-11-09");
        }
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Navigation));
        let task = &store.tasks().unwrap()[0];
        assert_eq!(task.title, "Køb mælk");
        assert_eq!(task.description, "To kartoner\nHavremælk");
        assert_eq!(task.due, today().succ_opt());
    }

    #[test]
    fn discard_does_not_create_and_navigation_letters_are_text_in_editor() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        for character in "qlpd".chars() {
            app.handle_key(key(KeyCode::Char(character)), &mut store, &config);
        }
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[0].text == "qlpd"));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Navigation));
        assert!(store.tasks().unwrap().is_empty());
    }

    #[test]
    fn clearing_a_due_date_keeps_the_other_task_fields() {
        let (_directory, mut store, config) = fixture();
        let list = store.lists().unwrap()[0].clone();
        let mut task_input = input(list.id, "Milk");
        task_input.due = Some(today());
        let task = store.create_task(&task_input).unwrap();
        let mut editor = TaskEditor::new(Some(task), &list.name);
        editor.fields[2] = TextBuffer::default();
        let fields = editor.input(&[list], config.language).unwrap();
        assert!(fields.due.is_none());
        assert_eq!(fields.title, "Milk");
    }

    #[test]
    fn stale_save_keeps_editor_and_external_update() {
        let (_directory, mut store, config) = fixture();
        let list_id = store.lists().unwrap()[0].id;
        let task = store.create_task(&input(list_id, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.active == 1));
        store
            .update_task(task.id, task.version, &input(list_id, "External"))
            .unwrap();
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert_eq!(store.task(task.id).unwrap().title, "External");
        assert!(app.message.as_ref().unwrap().is_error);
    }

    #[test]
    fn filter_error_retains_previous_results_and_input() {
        let (_directory, mut store, config) = fixture();
        let list_id = store.lists().unwrap()[0].id;
        store.create_task(&input(list_id, "Milk")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('/')), &mut store, &config);
        app.paste("\"unfinished");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Filter(buffer) if buffer.text == "\"unfinished"));
        assert_eq!(app.visible.len(), 1);
        assert!(app.filter.is_empty());
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert!(app.filter.is_empty());
    }

    #[test]
    fn help_scroll_reaches_all_effective_shortcuts() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        let first = rendered(&mut app, &config, 80, 12);
        assert!(first.contains(tr(config.language, "action_up")));
        for _ in 0..Action::ALL.len() {
            app.handle_key(key(KeyCode::Down), &mut store, &config);
        }
        let last = rendered(&mut app, &config, 80, 12);
        assert!(last.contains(tr(config.language, "action_scroll_right")));
        assert_eq!(app.help_state.selected(), Some(Action::ALL.len() - 1));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn list_forms_create_rename_and_discard() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        app.handle_key(key(KeyCode::Char('N')), &mut store, &config);
        app.paste("Ærinder");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.selection = app
            .lists
            .iter()
            .position(|list| list.name == "Ærinder")
            .unwrap();
        app.handle_key(key(KeyCode::Char('R')), &mut store, &config);
        if let Screen::ListName { buffer, .. } = &mut app.screen {
            *buffer = TextBuffer::new("Shopping");
        }
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(
            store
                .lists()
                .unwrap()
                .iter()
                .any(|list| list.name == "Shopping")
        );
        app.handle_key(key(KeyCode::Char('N')), &mut store, &config);
        app.paste("Discarded");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert!(
            !store
                .lists()
                .unwrap()
                .iter()
                .any(|list| list.name == "Discarded")
        );
    }

    #[test]
    fn navigation_scroll_and_reordering_preserve_date_boundaries() {
        let (_directory, mut store, config) = fixture();
        let list_id = store.lists().unwrap()[0].id;
        for index in 0..30 {
            let mut value = input(list_id, &format!("Task {index:02}"));
            value.due = Some(if index < 2 {
                today()
            } else {
                today().succ_opt().unwrap()
            });
            store.create_task(&value).unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        for _ in 0..29 {
            app.handle_key(key(KeyCode::Down), &mut store, &config);
        }
        assert!(rendered(&mut app, &config, 80, 12).contains("Task 29"));
        app.selection = 1;
        let moving = app.visible[1].id;
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.visible[0].id, moving);
        assert_eq!(app.selection, 0);
        app.selection = 2;
        let boundary = app.visible[2].clone();
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.visible[2].id, boundary.id);
        assert_eq!(store.task(boundary.id).unwrap().version, boundary.version);
    }

    #[test]
    fn trash_rejects_status_changes_and_restores_original_task() {
        let (_directory, mut store, config) = fixture();
        let list_id = store.lists().unwrap()[0].id;
        let task = store.create_task(&input(list_id, "Milk")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(app.visible.is_empty());
        app.set_view(
            View::Tasks {
                list_id: None,
                trash: true,
            },
            &store,
        )
        .unwrap();
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(store.task(task.id).unwrap().status, Status::Open);
        app.handle_key(key(KeyCode::Char('r')), &mut store, &config);
        assert!(!store.task(task.id).unwrap().deleted);
        assert!(app.visible.is_empty());
    }

    #[test]
    fn renderer_localizes_labels_and_handles_small_windows() {
        let (_directory, store, mut config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        assert!(rendered(&mut app, &config, 80, 24).contains(tr(config.language, "no_tasks")));
        config.language = Language::Da;
        app.set_view(View::Lists, &store).unwrap();
        assert!(rendered(&mut app, &config, 80, 24).contains("Papirkurv"));
        let small = rendered(&mut app, &config, 20, 5);
        assert!(!small.trim().is_empty());
        app.new_task();
        let editor = rendered(&mut app, &config, 80, 24);
        for field in FIELD_KEYS {
            assert!(editor.contains(tr(config.language, field)));
        }
    }

    #[test]
    fn refresh_preserves_selection_and_reclassifies_at_midnight() {
        let (_directory, mut store, _config) = fixture();
        let list_id = store.lists().unwrap()[0].id;
        let mut value = input(list_id, "Today");
        value.due = Some(today());
        let task = store.create_task(&value).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        store.create_task(&input(list_id, "New task")).unwrap();
        app.today = today().succ_opt().unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(app.selected_task().unwrap().id, task.id);
        assert_eq!(
            Bucket::for_task(&app.visible[0], app.today).key(),
            "overdue"
        );
        assert_eq!(app.visible.len(), 2);
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn escape_discards_existing_and_new_task_edits() {
        let (_directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Keep this title")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Unsaved description");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Discard new task");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(store.tasks().unwrap(), vec![task]);
    }

    #[test]
    fn list_selection_tracks_identity_after_back_rename_and_external_insert() {
        let (_directory, mut store, config) = fixture();
        store.create_list("Zulu").unwrap();
        let selected = store.create_list("Private").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::Lists, &store).unwrap();
        app.selection = app
            .lists
            .iter()
            .position(|list| list.id == selected.id)
            .unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        store.rename_list(selected.id, "Home").unwrap();
        store.create_list("AAA").unwrap();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.lists[app.selection].id, selected.id);
        app.selection = app.lists.len();
        store.create_list("Another").unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(app.selection, app.lists.len());
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.selection, app.lists.len());
        app.set_view(
            View::Tasks {
                list_id: None,
                trash: false,
            },
            &store,
        )
        .unwrap();
        app.new_task();
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[3].text == "Inbox"));
    }

    #[test]
    fn mouse_hits_visible_tasks_after_scrolling_and_ignores_group_headings() {
        let (_directory, mut store, config) = fixture();
        for index in 0..12 {
            let mut value = input(1, &format!("Task {index:02}"));
            value.due = Some(today() + chrono::Duration::days(index / 3));
            store.create_task(&value).unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        rendered(&mut app, &config, 80, 12);
        let (first, _) = app.mouse_targets[0];
        app.selection = 1;
        let now = Instant::now();
        app.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                first.x,
                first.y - 1,
            ),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.selection, 1);
        app.selection = 11;
        app.clamp_selection();
        let screen = rendered(&mut app, &config, 80, 12);
        assert!(app.list_state.offset() > 0);
        let (rect, MouseTarget::Row(index)) = app.mouse_targets[0] else {
            panic!()
        };
        let expected = app.visible[index].clone();
        assert!(
            screen
                .chars()
                .skip(usize::from(rect.y) * 80)
                .take(80)
                .collect::<String>()
                .contains(&expected.title)
        );
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x + 5, rect.y),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.selected_task().unwrap().id, expected.id);
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x + 5, rect.y),
            now + Duration::from_millis(200),
            &mut store,
            &config,
        )
        .unwrap();
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.original.as_ref().unwrap().id == expected.id)
        );
    }

    #[test]
    fn mouse_supports_list_open_wheel_editor_cursor_and_capture_toggle() {
        let (_directory, mut store, config) = fixture();
        let mut value = input(1, "First");
        value.description = "Æble\n界x".into();
        store.create_task(&value).unwrap();
        store.create_task(&input(1, "Second")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::Lists, &store).unwrap();
        rendered(&mut app, &config, 80, 24);
        let (rect, _) = app.mouse_targets[0];
        let now = Instant::now();
        for elapsed in [0, 100] {
            app.handle_mouse(
                mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
                now + Duration::from_millis(elapsed),
                &mut store,
                &config,
            )
            .unwrap();
        }
        assert_eq!(
            app.view,
            View::Tasks {
                list_id: Some(1),
                trash: false
            }
        );
        app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, 0, 0),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.selection, 1);
        app.handle_key(key(KeyCode::F(2)), &mut store, &config);
        app.handle_mouse(
            mouse(MouseEventKind::ScrollUp, 0, 0),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.selection, 1);
        app.handle_key(key(KeyCode::F(2)), &mut store, &config);
        app.handle_mouse(
            mouse(MouseEventKind::ScrollUp, 0, 0),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        rendered(&mut app, &config, 80, 24);
        let (rect, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 1, .. }))
            .copied()
            .unwrap();
        app.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                rect.x + 2,
                rect.y + 1,
            ),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        app.paste("!");
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.active == 1 && editor.fields[1].text == "Æble\n界!x")
        );
    }

    #[test]
    fn copy_view_has_plain_lines_and_keeps_unsaved_text_and_mouse_setting() {
        let (_directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Title")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("First line\nSecond line");
        app.handle_key(key(KeyCode::F(4)), &mut store, &config);
        assert!(!app.wants_mouse_capture());
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(!screen.contains('│'));
        assert!(
            matches!(&app.screen, Screen::CopyText { text, .. } if text == "Title\n\nFirst line\nSecond line")
        );
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        assert!(matches!(
            &app.screen,
            Screen::CopyText {
                row: 1,
                column: 1,
                ..
            }
        ));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(app.wants_mouse_capture());
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == "First line\nSecond line")
        );
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::F(2)), &mut store, &config);
        app.handle_key(key(KeyCode::F(4)), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(!app.wants_mouse_capture());
    }

    #[test]
    fn external_refresh_keeps_editor_version_and_reports_changes() {
        let (directory, mut store, mut config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Unsaved local text");
        external
            .update_task(task.id, task.version, &input(1, "Changed elsewhere"))
            .unwrap();
        app.refresh_changes(&store, &config, today(), true).unwrap();
        assert!(app.external_update);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == "Unsaved local text" && editor.original.as_ref().unwrap().version == task.version)
        );
        config.language = Language::Da;
        assert!(
            rendered(&mut app, &config, 120, 24).contains("Databasen er opdateret et andet sted")
        );
        app.refresh_changes(&store, &config, today(), true).unwrap();
        assert!(!app.external_update);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(app.message.as_ref().unwrap().is_error);
        assert_eq!(store.task(task.id).unwrap().title, "Changed elsewhere");
    }

    #[test]
    fn external_refresh_and_midnight_update_visible_tasks_without_losing_selection() {
        let (directory, mut store, config) = fixture();
        let first = store.create_task(&input(1, "First")).unwrap();
        let second = store.create_task(&input(1, "Second")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.selection = 1;
        external
            .set_status_on(first.id, first.version, Status::Done, today())
            .unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(app.external_update);
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.selected_task().unwrap().id, second.id);
        app.refresh_changes(&store, &config, today().succ_opt().unwrap(), false)
            .unwrap();
        assert_eq!(app.visible.len(), 1);
        assert_eq!(app.selected_task().unwrap().id, second.id);
    }

    #[test]
    fn ordinary_navigation_refresh_does_not_consume_an_external_notice() {
        let (directory, store, _config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        external.create_list("Created elsewhere").unwrap();
        app.set_view(View::Lists, &store).unwrap();
        assert!(app.external_update);
        assert!(
            app.lists
                .iter()
                .any(|list| list.name == "Created elsewhere")
        );
    }

    #[test]
    fn help_mouse_hits_skip_title_and_follow_scrolled_rows() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        app.help_state.select(Some(10));
        let screen = rendered(&mut app, &config, 80, 12);
        let (rect, MouseTarget::HelpRow(index)) = app.mouse_targets[0] else {
            panic!()
        };
        assert!(index > 0);
        let expected_label = tr(
            config.language,
            &format!("action_{}", Action::ALL[index].name()),
        );
        assert!(
            screen
                .chars()
                .skip(usize::from(rect.y) * 80)
                .take(80)
                .collect::<String>()
                .contains(expected_label)
        );
        let now = Instant::now();
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y - 1),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.help_state.selected(), Some(10));
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.help_state.selected(), Some(index));
    }

    #[test]
    fn task_rows_use_full_width_and_group_headings_keep_their_style() {
        let (_directory, mut store, config) = fixture();
        let mut value = input(1, &format!("Long title {} visible end", "x".repeat(90)));
        value.description = "Description must not appear beside tasks".into();
        store.create_task(&value).unwrap();
        store.create_task(&input(1, "Second task")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(140, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame, &config)).unwrap();
        let buffer = terminal.backend().buffer();
        let screen: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(screen.contains("visible end"));
        assert!(!screen.contains(&value.description));
        let (rect, _) = app.mouse_targets[0];
        assert_eq!(rect.width, 140);
        let heading = &buffer[(rect.x + 2, rect.y - 1)];
        let selected = &buffer[(rect.x + 2, rect.y)];
        assert_eq!(heading.fg, Color::Yellow);
        assert!(!heading.modifier.contains(Modifier::BOLD));
        assert_eq!(selected.fg, Color::Cyan);
        assert!(selected.modifier.contains(Modifier::BOLD));
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        terminal.draw(|frame| app.draw(frame, &config)).unwrap();
        let heading = &terminal.backend().buffer()[(rect.x + 2, rect.y - 1)];
        assert_eq!(heading.fg, Color::Yellow);
        assert!(!heading.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn description_wraps_visual_cursor_navigation_and_mouse_without_changing_text() {
        let (_directory, mut store, config) = fixture();
        let description = format!("{} four five six seven", "A".repeat(30));
        let mut value = input(1, "Wrapped description");
        value.description = description.clone();
        let task = store.create_task(&value).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        if let Screen::Editor(editor) = &mut app.screen {
            editor.fields[1].cursor = 41;
        }
        let screen = rendered(&mut app, &config, 38, 24);
        assert!(screen.contains("five six seven"));
        app.handle_key(key(KeyCode::Up), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].cursor == 5));
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].cursor == 41));
        let (rect, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 1, .. }))
            .copied()
            .unwrap();
        app.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                rect.x + 7,
                rect.y + 1,
            ),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].cursor == 43));
        app.paste("!");
        let expected = description.replacen("six", "si!x", 1);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == expected));
        rendered(&mut app, &config, 120, 24);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(store.task(task.id).unwrap().description, expected);
        assert!(!expected.contains('\n'));
    }

    #[test]
    fn wrapped_description_scrolls_to_cursor_and_keeps_tabs_and_graphemes() {
        let (_directory, mut store, config) = fixture();
        let description = format!(
            "Header\n\n{}\nEnd\t界e\u{301}👨‍👩‍👧‍👦",
            "several words on a wrapped line ".repeat(25)
        );
        let mut value = input(1, "Long description");
        value.description = description.clone();
        let task = store.create_task(&value).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let screen = rendered(&mut app, &config, 38, 18);
        assert!(screen.contains("End "));
        let (_, MouseTarget::EditorField { scroll_row, .. }) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 1, .. }))
            .copied()
            .unwrap()
        else {
            panic!()
        };
        assert!(scroll_row > 0);
        app.handle_key(key(KeyCode::Left), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].cursor == description.len() - "👨‍👩‍👧‍👦".len())
        );
        app.handle_key(key(KeyCode::Left), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].cursor == description.len() - "e\u{301}👨‍👩‍👧‍👦".len())
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(store.task(task.id).unwrap().description, description);
    }

    #[test]
    fn wrapped_vertical_boundaries_keep_the_original_byte_offset() {
        let mut buffer = TextBuffer::new("hello   world");
        buffer.cursor = 7;
        buffer.move_wrapped_vertical(false, 5);
        assert_eq!(buffer.cursor, 7);
        buffer.cursor = buffer.text.len();
        buffer.move_wrapped_vertical(true, 5);
        assert_eq!(buffer.cursor, buffer.text.len());
    }

    #[test]
    fn list_shortcuts_keep_default_and_trash_fixed_and_preserve_selected_id() {
        let (_directory, mut store, config) = fixture();
        let first = store.create_list("Work").unwrap();
        let second = store.create_list("Home").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::Lists, &store).unwrap();
        assert_eq!(app.lists[0].id, 1);
        app.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.lists[0].id, 1);
        app.selection = app
            .lists
            .iter()
            .position(|list| list.id == second.id)
            .unwrap();
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(
            app.lists.iter().map(|list| list.id).collect::<Vec<_>>(),
            [1, second.id, first.id]
        );
        assert_eq!(app.lists[app.selection].id, second.id);
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.lists[app.selection].id, second.id);
        assert_eq!(app.selection, 1);
        app.selection = app.lists.len();
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.selection, app.lists.len());
        app.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.selection, app.lists.len());
    }
}
