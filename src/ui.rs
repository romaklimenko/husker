use std::io::{self, IsTerminal, Stdout};
use std::panic::PanicHookInfo;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
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
use unicode_width::UnicodeWidthStr;

use crate::config::{Action, Config, Mode};
use crate::i18n::{Language, error_message, tr};
use crate::model::{Status, Task, TaskInput, TaskList};
use crate::planning::{Bucket, date_label};
use crate::query::{Query, parse_date, visible_tasks};
use crate::store::Store;

type PanicHook = Arc<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

struct TerminalGuard {
    previous_hook: PanicHook,
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    );
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste) {
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
    loop {
        terminal.draw(|frame| app.draw(frame, config))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if app.handle_key(key, store, config) {
                        break;
                    }
                }
                Event::Paste(text) => app.paste(&text),
                _ => {}
            }
        }
        if refreshed.elapsed() >= Duration::from_secs(1) {
            if matches!(app.screen, Screen::Navigation) {
                app.today = Local::now().date_naive();
                if let Err(error) = app.refresh(store) {
                    app.set_error(error, config);
                }
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
            .char_indices()
            .next_back()
            .map_or(0, |(at, _)| at)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8())
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
}

const FIELD_KEYS: [&str; 6] = ["title", "description", "due", "list", "tags", "readiness"];

#[derive(Clone, Debug)]
struct TaskEditor {
    original: Option<Task>,
    fields: [TextBuffer; 6],
    active: usize,
    readiness_changed: bool,
}

impl TaskEditor {
    fn new(task: Option<Task>, list_name: &str) -> Self {
        let fields = if let Some(task) = &task {
            [
                TextBuffer::new(&task.title),
                TextBuffer::new(&task.description),
                TextBuffer::new(task.due.map(|date| date.to_string()).unwrap_or_default()),
                TextBuffer::new(list_name),
                TextBuffer::new(task.tags.join(" ")),
                TextBuffer::new(if task.ready { "anytime" } else { "maybe" }),
            ]
        } else {
            [
                TextBuffer::default(),
                TextBuffer::default(),
                TextBuffer::default(),
                TextBuffer::new(list_name),
                TextBuffer::default(),
                TextBuffer::new("maybe"),
            ]
        };
        let active = if task.is_some() { 1 } else { 0 };
        Self {
            original: task,
            fields,
            active,
            readiness_changed: false,
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
        let mut ready = match self.fields[5].text.trim() {
            "maybe" => false,
            "anytime" => true,
            _ => bail!(tr(language, "invalid_readiness")),
        };
        if self
            .original
            .as_ref()
            .is_some_and(|task| task.due.is_some())
            && due.is_none()
            && !self.readiness_changed
        {
            ready = true;
        }
        Ok(TaskInput {
            list_id,
            title: self.fields[0].text.clone(),
            description: self.fields[1].text.clone(),
            due,
            ready,
            tags: self.fields[4]
                .text
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
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
    ListName { id: Option<i64>, buffer: TextBuffer },
    Help,
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
    message: Option<(String, bool)>,
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
        let selected_task = self.selected_task().map(|task| task.id);
        let selected_list = self.lists.get(self.selection).map(|list| list.id);
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
        } else if let Some(index) = self
            .lists
            .iter()
            .position(|list| Some(list.id) == selected_list)
        {
            self.selection = index;
        }
        self.clamp_selection();
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
        self.list_state
            .select((self.row_count() > 0).then_some(self.selection));
    }

    fn selected_task(&self) -> Option<&Task> {
        if self.view == View::Lists {
            None
        } else {
            self.visible.get(self.selection)
        }
    }

    fn set_error(&mut self, error: anyhow::Error, config: &Config) {
        self.message = Some((
            format!(
                "{}: {}",
                tr(config.language, "error"),
                error_message(config.language, &error)
            ),
            true,
        ));
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
        match &mut self.screen {
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
                        let changed = editor.fields[editor.active].key(key, editor.active == 1);
                        editor.readiness_changed |= editor.active == 5 && changed;
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
                        if let Some(id) = id {
                            store.rename_list(*id, &buffer.text)?;
                        } else {
                            store.create_list(&buffer.text)?;
                        }
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                        self.message = Some((tr(config.language, "saved").to_owned(), false));
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
                editor.readiness_changed |= editor.active == 5 && !normalized.is_empty();
            }
            Screen::Filter(buffer) | Screen::ListName { buffer, .. } => {
                buffer.insert(&text.replace(['\r', '\n'], " "))
            }
            _ => {}
        }
    }

    fn set_view(&mut self, view: View, store: &Store) -> Result<()> {
        self.view = view;
        self.selection = 0;
        self.list_state = ListState::default();
        self.filter.clear();
        self.query = Query::default();
        self.refresh(store)
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
                .or_else(|| self.lists.first()),
            _ => self.lists.first(),
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
        self.message = Some((tr(config.language, "saved").to_owned(), false));
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
            Action::Anytime | Action::Maybe => {
                let input = TaskInput {
                    list_id: task.list_id,
                    title: task.title,
                    description: task.description,
                    due: None,
                    ready: matches!(action, Action::Anytime),
                    tags: task.tags,
                };
                store.update_task(task.id, task.version, &input)?;
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
                if task.due != other.due || (task.due.is_none() && task.ready != other.ready) {
                    return Ok(());
                }
                store.swap_positions(&task, other)?;
            }
            _ => return Ok(()),
        }
        self.refresh(store)?;
        self.message = Some((tr(config.language, "updated").to_owned(), false));
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>, config: &Config) {
        let area = frame.area();
        let language = config.language;
        if area.width < 38
            || area.height < 10
            || (matches!(self.screen, Screen::Editor(_)) && area.height < 24)
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
            Screen::Editor(editor) => draw_editor(frame, sections[1], editor, language),
            Screen::Help => draw_help(frame, sections[1], config, &mut self.help_state),
            Screen::ListName { id, buffer } => {
                let title = tr(
                    language,
                    if id.is_some() {
                        "rename_list"
                    } else {
                        "new_list"
                    },
                );
                draw_input(
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
                        draw_input(frame, parts[0], tr(language, "filter"), buffer, true);
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
                "{} {}  |  {} {}  |  {} / {}",
                config.keys_for(Action::Save),
                tr(language, "action_save"),
                config.keys_for(Action::Discard),
                tr(language, "action_discard"),
                config.keys_for(Action::NextField),
                config.keys_for(Action::PreviousField)
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
        if let Some((message, error)) = &self.message {
            footer.push(Line::from(Span::styled(
                message.as_str(),
                Style::default().fg(if *error { Color::Red } else { Color::Green }),
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
            return;
        }
        if self.visible.is_empty() {
            frame.render_widget(Paragraph::new(tr(language, "no_tasks")), area);
            return;
        }
        let columns = if area.width >= 100 {
            Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).split(area)
        } else {
            Layout::horizontal([Constraint::Percentage(100)]).split(area)
        };
        let trash = matches!(self.view, View::Tasks { trash: true, .. });
        let mut previous_bucket = None;
        let rows: Vec<ListItem<'_>> = self
            .visible
            .iter()
            .map(|task| {
                let bucket = Bucket::for_task(task, self.today);
                let mut lines = Vec::new();
                if !trash && previous_bucket != Some(bucket) {
                    lines.push(Line::from(Span::styled(
                        tr(language, bucket.key()),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                previous_bucket = Some(bucket);
                let marker = match task.status {
                    Status::Open => "[ ]",
                    Status::Done => "[x]",
                    Status::Cancelled => "[-]",
                };
                let date = task
                    .due
                    .map(|date| format!("  {}", date_label(date, language)))
                    .unwrap_or_default();
                lines.push(Line::from(format!("{marker} {}{date}", task.title)));
                ListItem::new(lines)
            })
            .collect();
        frame.render_stateful_widget(
            List::new(rows)
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> "),
            columns[0],
            &mut self.list_state,
        );
        if columns.len() > 1
            && let Some(task) = self.selected_task()
        {
            let list_name = self
                .lists
                .iter()
                .find(|list| list.id == task.list_id)
                .map_or("", |list| list.name.as_str());
            let tags = task
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" ");
            let text = format!(
                "{}\n{}: {}\n{}\n\n{}",
                task.title,
                tr(language, "list"),
                list_name,
                tags,
                task.description
            );
            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::LEFT)),
                columns[1],
            );
        }
    }
}

fn draw_input(frame: &mut Frame<'_>, area: Rect, title: &str, buffer: &TextBuffer, active: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
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
}

fn draw_editor(frame: &mut Frame<'_>, area: Rect, editor: &TaskEditor, language: Language) {
    let fields = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .split(area);
    for (index, key) in FIELD_KEYS.iter().enumerate() {
        draw_input(
            frame,
            fields[index],
            tr(language, key),
            &editor.fields[index],
            index == editor.active,
        );
    }
}

fn draw_help(frame: &mut Frame<'_>, area: Rect, config: &Config, state: &mut ListState) {
    let rows: Vec<ListItem<'_>> = Action::ALL
        .iter()
        .map(|action| {
            let label = tr(config.language, &format!("action_{}", action.name()));
            ListItem::new(format!("{:<20} {label}", config.keys_for(*action)))
        })
        .collect();
    frame.render_stateful_widget(
        List::new(rows)
            .block(Block::default().title(tr(config.language, "help")))
            .highlight_style(Style::default().fg(Color::Cyan))
            .highlight_symbol("> "),
        area,
        state,
    );
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
            ready: false,
            tags: Vec::new(),
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
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(store.tasks().unwrap().is_empty());
        if let Screen::Editor(editor) = &mut app.screen {
            editor.fields[2] = TextBuffer::new("2026-11-09");
        }
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
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
    fn clearing_a_due_date_makes_a_task_ready_unless_maybe_was_explicit() {
        let (_directory, mut store, config) = fixture();
        let list = store.lists().unwrap()[0].clone();
        let mut task_input = input(list.id, "Milk");
        task_input.due = Some(today());
        let task = store.create_task(&task_input).unwrap();
        let mut editor = TaskEditor::new(Some(task), &list.name);
        editor.fields[2] = TextBuffer::default();
        assert!(
            editor
                .input(std::slice::from_ref(&list), config.language)
                .unwrap()
                .ready
        );
        editor.readiness_changed = true;
        assert!(!editor.input(&[list], config.language).unwrap().ready);
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
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert_eq!(store.task(task.id).unwrap().title, "External");
        assert!(app.message.as_ref().unwrap().1);
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
        assert!(last.contains(tr(config.language, "action_maybe")));
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
}
