use std::io::{self, IsTerminal, Stdout};
use std::panic::PanicHookInfo;
use std::path::Path;
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
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use unicode_width::UnicodeWidthStr;

use crate::clipboard::{SystemClipboard, TextClipboard};
use crate::config::{Action, Config, Mode};
use crate::editor_date::parse_editor_date;
use crate::i18n::{Language, error_message, tr};
use crate::model::{Status, Task, TaskInput, TaskList};
use crate::planning::{DuePreset, date_label, history_group_label, ordinary_group_label};
use crate::query::{Query, visible_history, visible_tasks};
use crate::store::{ListDeletionOutcome, ListDeletionPreview, Store};
use crate::text_buffer::TextBuffer;
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

pub fn run(store: &mut Store, config: &Config, config_path: &Path) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(tr(config.language, "terminal_required"));
    }
    let mut app = App::new(store, Local::now().date_naive())?;
    let mut config = config.clone();
    let _guard = TerminalGuard::enter()?;
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut refreshed = Instant::now();
    let mut mouse_captured = true;
    loop {
        app.expire_message(Instant::now());
        terminal.draw(|frame| app.draw(frame, &config))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if app.handle_key_on_date(key, store, &config, Local::now().date_naive()) {
                        break;
                    }
                }
                Event::Paste(text) => app.paste(&text),
                Event::Mouse(mouse) => {
                    if let Err(error) = app.handle_mouse_on_date(
                        mouse,
                        Instant::now(),
                        store,
                        &config,
                        Local::now().date_naive(),
                    ) {
                        app.set_error(error, &config);
                    }
                }
                _ => {}
            }
            if let Err(error) = app.apply_language_change(&mut config, config_path) {
                app.set_error(error, &config);
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
            if let Err(error) =
                app.refresh_changes(store, &config, Local::now().date_naive(), false)
            {
                app.set_error(error, &config);
            }
            refreshed = Instant::now();
        }
    }
    Ok(())
}

const FIELD_KEYS: [&str; 4] = ["title", "description", "due", "list"];

fn due_preset(action: Action) -> Option<DuePreset> {
    match action {
        Action::DueToday => Some(DuePreset::Today),
        Action::DueTomorrow => Some(DuePreset::Tomorrow),
        Action::DueNextWeek => Some(DuePreset::NextMonday),
        Action::ClearDue => Some(DuePreset::Clear),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct TaskEditor {
    original: Option<Task>,
    fields: [TextBuffer; 4],
    initial_text: [String; 4],
    active: usize,
}

impl TaskEditor {
    fn new(task: Option<Task>, list_id: i64) -> Self {
        let fields = if let Some(task) = &task {
            [
                TextBuffer::new(&task.title),
                TextBuffer::new(&task.description),
                TextBuffer::new(task.due.map(|date| date.to_string()).unwrap_or_default()),
                TextBuffer::new(list_id.to_string()),
            ]
        } else {
            [
                TextBuffer::default(),
                TextBuffer::default(),
                TextBuffer::default(),
                TextBuffer::new(list_id.to_string()),
            ]
        };
        let active = if task.is_some() { 1 } else { 0 };
        Self {
            original: task,
            initial_text: std::array::from_fn(|index| fields[index].text.clone()),
            fields,
            active,
        }
    }

    fn is_dirty(&self) -> bool {
        self.fields
            .iter()
            .zip(&self.initial_text)
            .any(|(field, initial)| field.text != *initial)
    }

    fn selected_list_id(&self) -> Option<i64> {
        self.fields[3].text.parse().ok()
    }

    fn choose_list(&mut self, id: i64) {
        self.fields[3].replace_all(&id.to_string());
    }

    fn focus_field(&mut self, index: usize, today: NaiveDate) {
        if self.active == 2
            && index != 2
            && let Ok(date) = parse_editor_date(&self.fields[2].text, today)
        {
            self.fields[2].replace_all(&date.map(|date| date.to_string()).unwrap_or_default());
        }
        self.active = index;
    }

    fn input(&self, lists: &[TaskList], language: Language, today: NaiveDate) -> Result<TaskInput> {
        let due = parse_editor_date(&self.fields[2].text, today)
            .with_context(|| tr(language, "invalid_date"))?;
        let list_id = lists
            .iter()
            .find(|list| Some(list.id) == self.selected_list_id())
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
    Planned,
    List(i64),
    Trash,
    History(Option<i64>),
}

impl View {
    fn scope(self) -> (Option<i64>, bool) {
        match self {
            Self::Planned => (None, false),
            Self::List(id) => (Some(id), false),
            Self::Trash => (None, true),
            Self::History(list_id) => (list_id, false),
        }
    }
}

#[derive(Debug, Default)]
struct ListPicker {
    selection: usize,
    state: ListState,
    area: Option<Rect>,
}

struct PlannedBookmark {
    filter: String,
    query: Query,
    selected_task: Option<i64>,
    selection: usize,
    list_state: ListState,
}

#[derive(Debug)]
enum Screen {
    Navigation,
    ListPicker,
    DeleteListConfirm {
        preview: Box<ListDeletionPreview>,
        delete: bool,
        changed: bool,
    },
    Editor(Box<TaskEditor>),
    EditorListPicker {
        editor: Box<TaskEditor>,
        picker: ListPicker,
    },
    ConfirmDiscard {
        editor: Box<TaskEditor>,
        discard: bool,
    },
    Filter(TextBuffer),
    ListName {
        id: Option<i64>,
        buffer: TextBuffer,
    },
    Help {
        previous: Box<Screen>,
    },
    Language {
        selected: Language,
        previous: Box<Screen>,
    },
    CopyText {
        text: String,
        row: u16,
        column: u16,
        previous: Box<Screen>,
    },
}

impl Screen {
    fn editor_picker(&self) -> Option<&ListPicker> {
        match self {
            Self::EditorListPicker { picker, .. } => Some(picker),
            Self::Help { previous } | Self::Language { previous, .. } => previous.editor_picker(),
            _ => None,
        }
    }

    fn editor_picker_mut(&mut self) -> Option<&mut ListPicker> {
        match self {
            Self::EditorListPicker { picker, .. } => Some(picker),
            Self::Help { previous } | Self::Language { previous, .. } => {
                previous.editor_picker_mut()
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowIdentity {
    AllTasks,
    List(i64),
    Task(i64),
    Trash,
}

#[derive(Clone, Copy, Debug)]
enum MouseTarget {
    Row(usize),
    PickerRow(usize),
    EditorListRow(usize),
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
    clipboard: Box<dyn TextClipboard>,
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
    picker: ListPicker,
    planned_bookmark: Option<PlannedBookmark>,
    history_origin: Option<(View, PlannedBookmark)>,
    pending_language: Option<Language>,
    description_width: usize,
}

impl App {
    fn new(store: &Store, today: NaiveDate) -> Result<Self> {
        let mut app = Self {
            clipboard: Box::new(SystemClipboard::default()),
            view: View::Planned,
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
            picker: ListPicker::default(),
            planned_bookmark: None,
            history_origin: None,
            pending_language: None,
            description_width: 79,
        };
        app.refresh(store)?;
        Ok(app)
    }

    fn mode(&self) -> Mode {
        match self.screen {
            Screen::ListPicker => Mode::Lists,
            Screen::DeleteListConfirm { .. } => Mode::DeleteListConfirm,
            Screen::Editor(ref editor) if editor.active == 2 => Mode::DueField,
            Screen::Editor(ref editor) if editor.active == 3 => Mode::ListField,
            Screen::Editor(_) => Mode::Editor,
            Screen::EditorListPicker { .. } => Mode::EditorListPicker,
            Screen::ConfirmDiscard { .. } => Mode::ConfirmDiscard,
            Screen::Filter(_) => Mode::Filter,
            Screen::ListName { .. } => Mode::ListName,
            Screen::Help { .. } => Mode::Help,
            Screen::Language { .. } => Mode::Language,
            Screen::CopyText { .. } => Mode::CopyText,
            Screen::Navigation if matches!(self.view, View::History(_)) => Mode::History,
            Screen::Navigation => Mode::Tasks,
        }
    }

    fn refresh(&mut self, store: &Store) -> Result<()> {
        let data_version = store.data_version()?;
        self.external_update |= data_version != self.data_version;
        let mut selected_task = self.selected_task().map(|task| task.id);
        let selected_list = self.picker_identity(self.picker.selection);
        let editor_list = self
            .screen
            .editor_picker()
            .and_then(|picker| self.lists.get(picker.selection))
            .map(|list| list.id);
        let lists = store.lists()?;
        let tasks = store.tasks()?;
        self.lists = lists;
        self.tasks = tasks;
        if let Some(id) = self.view.scope().0
            && !self.lists.iter().any(|list| list.id == id)
        {
            self.history_origin = None;
            let bookmark = self.planned_bookmark.take();
            selected_task = self.restore_view_state(View::Planned, bookmark);
        }
        let (list_id, trash) = self.view.scope();
        self.visible = if matches!(self.view, View::History(_)) {
            visible_history(&self.tasks, &self.lists, &self.query, list_id, self.today)
        } else {
            visible_tasks(
                &self.tasks,
                &self.lists,
                &self.query,
                list_id,
                trash,
                self.today,
            )
        };
        if let Some(index) = self
            .visible
            .iter()
            .position(|task| Some(task.id) == selected_task)
        {
            self.selection = index;
        }
        self.clamp_selection();
        if let Some(identity) = selected_list {
            self.select_picker_identity(identity);
        }
        self.clamp_picker();
        if let Some(picker) = self.screen.editor_picker_mut() {
            if let Some(index) = self
                .lists
                .iter()
                .position(|list| Some(list.id) == editor_list)
            {
                picker.selection = index;
            }
            picker.selection = picker.selection.min(self.lists.len().saturating_sub(1));
            picker
                .state
                .select((!self.lists.is_empty()).then_some(picker.selection));
        }
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
        let outcome_message = if external || date_changed || manual {
            self.refresh(store)?;
            self.refresh_deletion_preview(store, config)?
        } else {
            false
        };
        if self.data_version != previous_version {
            self.external_update = true;
        } else if manual {
            self.external_update = false;
        }
        if manual && !outcome_message {
            self.show_message(tr(config.language, "refreshed"), false, Instant::now());
        }
        Ok(())
    }

    fn refresh_deletion_preview(&mut self, store: &Store, config: &Config) -> Result<bool> {
        let Screen::DeleteListConfirm {
            preview,
            delete,
            changed,
        } = &mut self.screen
        else {
            return Ok(false);
        };
        if !self.lists.iter().any(|list| list.id == preview.list_id) {
            self.screen = Screen::ListPicker;
            self.show_message(
                tr(config.language, "list_delete_missing"),
                true,
                Instant::now(),
            );
            return Ok(true);
        } else {
            let current = store.preview_list_deletion(preview.list_id)?;
            if current != **preview {
                *preview = Box::new(current);
                *delete = false;
                *changed = true;
            }
        }
        Ok(false)
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
        self.visible
            .get(index)
            .map(|task| RowIdentity::Task(task.id))
    }

    fn picker_identity(&self, index: usize) -> Option<RowIdentity> {
        if index == 0 {
            Some(RowIdentity::AllTasks)
        } else if index == self.lists.len() + 1 {
            Some(RowIdentity::Trash)
        } else {
            self.lists
                .get(index - 1)
                .map(|list| RowIdentity::List(list.id))
        }
    }

    fn select_picker_identity(&mut self, identity: RowIdentity) {
        let index = match identity {
            RowIdentity::AllTasks => Some(0),
            RowIdentity::Trash => Some(self.lists.len() + 1),
            RowIdentity::List(id) => self
                .lists
                .iter()
                .position(|list| list.id == id)
                .map(|index| index + 1),
            RowIdentity::Task(_) => None,
        };
        if let Some(index) = index {
            self.picker.selection = index;
        }
        self.clamp_picker();
    }

    fn clamp_picker(&mut self) {
        self.picker.selection = self.picker.selection.min(self.lists.len() + 1);
        self.picker.state.select(Some(self.picker.selection));
    }

    fn open_picker(&mut self) {
        let identity = match self.view {
            View::Planned | View::History(None) => RowIdentity::AllTasks,
            View::List(id) | View::History(Some(id)) => RowIdentity::List(id),
            View::Trash => RowIdentity::Trash,
        };
        self.picker = ListPicker::default();
        self.select_picker_identity(identity);
        self.screen = Screen::ListPicker;
        self.last_click = None;
    }

    fn choose_picker(&mut self, store: &Store) -> Result<()> {
        let view = match self.picker_identity(self.picker.selection) {
            Some(RowIdentity::AllTasks) => View::Planned,
            Some(RowIdentity::List(id)) => View::List(id),
            Some(RowIdentity::Trash) => View::Trash,
            _ => return Ok(()),
        };
        self.set_view(view, store)
    }

    fn open_editor_list_picker(&mut self, store: &Store) -> Result<()> {
        self.refresh(store)?;
        let Screen::Editor(editor) = std::mem::replace(&mut self.screen, Screen::Navigation) else {
            unreachable!()
        };
        let selection = self
            .lists
            .iter()
            .position(|list| Some(list.id) == editor.selected_list_id())
            .unwrap_or(0);
        let picker = ListPicker {
            selection,
            state: ListState::default()
                .with_selected((!self.lists.is_empty()).then_some(selection)),
            area: None,
        };
        self.screen = Screen::EditorListPicker { editor, picker };
        self.last_click = None;
        Ok(())
    }

    fn close_editor_list_picker(&mut self) {
        let Screen::EditorListPicker { editor, .. } =
            std::mem::replace(&mut self.screen, Screen::Navigation)
        else {
            unreachable!()
        };
        self.screen = Screen::Editor(editor);
        self.last_click = None;
    }

    fn choose_editor_list(&mut self, store: &Store, config: &Config) -> Result<()> {
        let Some(id) = self
            .screen
            .editor_picker()
            .and_then(|picker| self.lists.get(picker.selection))
            .map(|list| list.id)
        else {
            return Ok(());
        };
        self.refresh(store)?;
        self.lists
            .iter()
            .find(|list| list.id == id)
            .with_context(|| tr(config.language, "list_unavailable"))?;
        self.close_editor_list_picker();
        if let Screen::Editor(editor) = &mut self.screen {
            editor.choose_list(id);
        }
        Ok(())
    }

    fn handle_mouse_on_date(
        &mut self,
        mouse: MouseEvent,
        now: Instant,
        store: &mut Store,
        config: &Config,
        today: NaiveDate,
    ) -> Result<()> {
        let date_changed = self.today != today;
        self.today = today;
        let result = self.handle_mouse(mouse, now, store, config);
        let refresh = if date_changed {
            self.refresh(store)
        } else {
            Ok(())
        };
        result.and(refresh)
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        now: Instant,
        store: &mut Store,
        config: &Config,
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
            match &mut self.screen {
                Screen::Navigation => {
                    self.selection = if down {
                        self.selection.saturating_add(1)
                    } else {
                        self.selection.saturating_sub(1)
                    };
                    self.clamp_selection();
                }
                Screen::ListPicker => {
                    if self.picker.area.is_some_and(|area| {
                        mouse.column >= area.x
                            && mouse.column < area.right()
                            && mouse.row >= area.y
                            && mouse.row < area.bottom()
                    }) {
                        self.picker.selection = if down {
                            self.picker.selection.saturating_add(1)
                        } else {
                            self.picker.selection.saturating_sub(1)
                        };
                        self.clamp_picker();
                    }
                }
                Screen::EditorListPicker { picker, .. } => {
                    if picker.area.is_some_and(|area| {
                        mouse.column >= area.x
                            && mouse.column < area.right()
                            && mouse.row >= area.y
                            && mouse.row < area.bottom()
                    }) {
                        picker.selection = if down {
                            picker.selection.saturating_add(1)
                        } else {
                            picker.selection.saturating_sub(1)
                        }
                        .min(self.lists.len().saturating_sub(1));
                        picker
                            .state
                            .select((!self.lists.is_empty()).then_some(picker.selection));
                    }
                }
                Screen::Help { .. } => {
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
            MouseTarget::EditorListRow(index)
                if matches!(self.screen, Screen::EditorListPicker { .. }) =>
            {
                let Some(list) = self.lists.get(index) else {
                    return Ok(());
                };
                let identity = RowIdentity::List(list.id);
                let double_click = self.last_click.is_some_and(|(previous, at)| {
                    previous == identity
                        && now.saturating_duration_since(at) <= Duration::from_millis(400)
                });
                if let Screen::EditorListPicker { picker, .. } = &mut self.screen {
                    picker.selection = index;
                    picker.state.select(Some(index));
                }
                if double_click {
                    self.last_click = None;
                    self.choose_editor_list(store, config)?;
                } else {
                    self.last_click = Some((identity, now));
                }
            }
            MouseTarget::PickerRow(index) if matches!(self.screen, Screen::ListPicker) => {
                let Some(identity) = self.picker_identity(index) else {
                    return Ok(());
                };
                let double_click = self.last_click.is_some_and(|(previous, at)| {
                    previous == identity
                        && now.saturating_duration_since(at) <= Duration::from_millis(400)
                });
                self.picker.selection = index;
                self.clamp_picker();
                if double_click {
                    self.last_click = None;
                    self.choose_picker(store)?;
                } else {
                    self.last_click = Some((identity, now));
                }
            }
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
                    editor.focus_field(index, self.today);
                    let row = scroll_row + usize::from(mouse.row - rect.y);
                    let column = scroll_column + usize::from(mouse.column - rect.x);
                    if index == 1 {
                        let layout = TextLayout::new(
                            &editor.fields[index].text,
                            usize::from(rect.width.saturating_sub(1)),
                        );
                        editor.fields[index].set_cursor(layout.cursor_at(row, column), false);
                    } else if index != 3 {
                        editor.fields[index].set_cursor_position(row, column);
                    }
                }
                if index == 3 && matches!(self.screen, Screen::Editor(_)) {
                    self.open_editor_list_picker(store)?;
                }
            }
            MouseTarget::HelpRow(index) if matches!(self.screen, Screen::Help { .. }) => {
                self.last_click = None;
                self.help_state.select(Some(index));
            }
            _ => {}
        }
        Ok(())
    }

    fn row_count(&self) -> usize {
        self.visible.len()
    }

    fn clamp_selection(&mut self) {
        self.selection = self.selection.min(self.row_count().saturating_sub(1));
    }

    fn selected_task(&self) -> Option<&Task> {
        self.visible.get(self.selection)
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

    fn copy_selection(&mut self, config: &Config) -> Result<()> {
        let buffer = match &self.screen {
            Screen::Editor(editor) if editor.active != 3 => &editor.fields[editor.active],
            Screen::Filter(buffer) | Screen::ListName { buffer, .. } => buffer,
            _ => return Ok(()),
        };
        let Some(selection) = buffer.selection() else {
            return Ok(());
        };
        let text = buffer.text[selection].to_owned();
        self.clipboard
            .set_text(&text)
            .with_context(|| tr(config.language, "copy_failed"))?;
        self.show_message(tr(config.language, "copied"), false, Instant::now());
        Ok(())
    }

    fn apply_language_change(&mut self, config: &mut Config, path: &Path) -> Result<()> {
        let Some(language) = self.pending_language.take() else {
            return Ok(());
        };
        config.save_language(path, language)?;
        config.language = language;
        let Screen::Language { previous, .. } =
            std::mem::replace(&mut self.screen, Screen::Navigation)
        else {
            unreachable!()
        };
        self.screen = *previous;
        self.last_click = None;
        self.show_message(tr(language, "language_saved"), false, Instant::now());
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent, store: &mut Store, config: &Config) -> bool {
        self.last_click = None;
        match self.dispatch_key(key, store, config) {
            Ok(quit) => quit,
            Err(error) => {
                self.set_error(error, config);
                false
            }
        }
    }

    fn handle_key_on_date(
        &mut self,
        key: KeyEvent,
        store: &mut Store,
        config: &Config,
        today: NaiveDate,
    ) -> bool {
        let date_changed = self.today != today;
        self.today = today;
        let quit = self.handle_key(key, store, config);
        if date_changed
            && let Err(error) = self.refresh(store)
            && !self
                .message
                .as_ref()
                .is_some_and(|message| message.is_error)
        {
            self.set_error(error, config);
        }
        quit
    }

    fn dispatch_key(&mut self, key: KeyEvent, store: &mut Store, config: &Config) -> Result<bool> {
        let action = config.action(key, self.mode());
        if let Some(preset) = action.and_then(due_preset)
            && let Screen::Editor(editor) = &mut self.screen
            && editor.active == 2
        {
            let value = preset
                .resolve(self.today)?
                .map(|date| date.to_string())
                .unwrap_or_default();
            editor.fields[2].replace_all(&value);
            return Ok(false);
        }
        match action {
            Some(Action::Language) => {
                let previous = Box::new(std::mem::replace(&mut self.screen, Screen::Navigation));
                self.screen = Screen::Language {
                    selected: config.language,
                    previous,
                };
                self.pending_language = None;
                return Ok(false);
            }
            Some(Action::CopySelection) => {
                self.copy_selection(config)?;
                return Ok(false);
            }
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
            Screen::EditorListPicker { picker, .. } => {
                match action {
                    Some(Action::Up | Action::Down) => {
                        picker.selection = if action == Some(Action::Down) {
                            picker.selection.saturating_add(1)
                        } else {
                            picker.selection.saturating_sub(1)
                        }
                        .min(self.lists.len().saturating_sub(1));
                        picker
                            .state
                            .select((!self.lists.is_empty()).then_some(picker.selection));
                    }
                    Some(Action::Open) => self.choose_editor_list(store, config)?,
                    Some(Action::Back | Action::Lists) => {
                        self.close_editor_list_picker();
                        self.refresh(store)?;
                    }
                    Some(Action::Help) => {
                        let previous =
                            Box::new(std::mem::replace(&mut self.screen, Screen::Navigation));
                        self.screen = Screen::Help { previous };
                        self.help_state.select(Some(0));
                    }
                    _ => {}
                }
                return Ok(false);
            }
            Screen::Language { selected, .. } => {
                match action {
                    Some(Action::Up) => *selected = Language::En,
                    Some(Action::Down) => *selected = Language::Da,
                    Some(Action::Apply) => self.pending_language = Some(*selected),
                    Some(Action::Back) => {
                        let Screen::Language { previous, .. } =
                            std::mem::replace(&mut self.screen, Screen::Navigation)
                        else {
                            unreachable!()
                        };
                        self.screen = *previous;
                        self.pending_language = None;
                    }
                    _ => {}
                }
                return Ok(false);
            }
            Screen::ListPicker => {
                match action {
                    Some(Action::Quit) => return Ok(true),
                    Some(Action::Back | Action::Lists) => {
                        self.screen = Screen::Navigation;
                        self.refresh(store)?;
                    }
                    Some(Action::AllTasks) => self.set_view(View::Planned, store)?,
                    Some(Action::Open) => self.choose_picker(store)?,
                    Some(Action::Up) => {
                        self.picker.selection = self.picker.selection.saturating_sub(1);
                        self.clamp_picker();
                    }
                    Some(Action::Down) => {
                        self.picker.selection = self.picker.selection.saturating_add(1);
                        self.clamp_picker();
                    }
                    Some(Action::NewList) => {
                        self.screen = Screen::ListName {
                            id: None,
                            buffer: TextBuffer::default(),
                        };
                    }
                    Some(Action::RenameList) => {
                        if let Some(RowIdentity::List(id)) =
                            self.picker_identity(self.picker.selection)
                            && let Some(list) = self.lists.iter().find(|list| list.id == id)
                        {
                            self.screen = Screen::ListName {
                                id: Some(id),
                                buffer: TextBuffer::new(&list.name),
                            };
                        }
                    }
                    Some(Action::DeleteList) => {
                        if let Some(RowIdentity::List(id)) =
                            self.picker_identity(self.picker.selection)
                        {
                            if self.lists.iter().map(|list| list.id).min() == Some(id) {
                                self.show_message(
                                    tr(config.language, "default_list_cannot_delete"),
                                    false,
                                    Instant::now(),
                                );
                            } else {
                                self.screen = Screen::DeleteListConfirm {
                                    preview: Box::new(store.preview_list_deletion(id)?),
                                    delete: false,
                                    changed: false,
                                };
                            }
                        }
                    }
                    Some(Action::MoveUp | Action::MoveDown) => {
                        if let Some(RowIdentity::List(id)) =
                            self.picker_identity(self.picker.selection)
                        {
                            store.move_list(id, action == Some(Action::MoveDown))?;
                            self.refresh(store)?;
                        }
                    }
                    Some(Action::Help) => {
                        self.screen = Screen::Help {
                            previous: Box::new(Screen::ListPicker),
                        };
                        self.help_state.select(Some(0));
                    }
                    _ => {}
                }
                return Ok(false);
            }
            Screen::DeleteListConfirm { delete, .. } => {
                if action == Some(Action::Back) {
                    self.screen = Screen::ListPicker;
                    self.refresh(store)?;
                } else if key.modifiers.is_empty() {
                    match key.code {
                        KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Tab
                        | KeyCode::BackTab => *delete = !*delete,
                        KeyCode::Enter => self.finish_list_deletion(store, config)?,
                        _ => {}
                    }
                } else if matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                    && key.modifiers == KeyModifiers::SHIFT
                {
                    *delete = !*delete;
                }
                return Ok(false);
            }
            Screen::ConfirmDiscard { discard, .. } => {
                if action == Some(Action::Discard) {
                    self.finish_discard_confirmation(false, store)?;
                } else if key.modifiers.is_empty() {
                    match key.code {
                        KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Tab
                        | KeyCode::BackTab => {
                            *discard = !*discard;
                        }
                        KeyCode::Enter => {
                            let discard = *discard;
                            self.finish_discard_confirmation(discard, store)?;
                        }
                        _ => {}
                    }
                } else if matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                    && key.modifiers == KeyModifiers::SHIFT
                {
                    *discard = !*discard;
                }
                return Ok(false);
            }
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
                    Some(Action::Open | Action::Lists) if editor.active == 3 => {
                        self.open_editor_list_picker(store)?
                    }
                    Some(Action::Save) => self.save_editor(store, config)?,
                    Some(Action::Discard) => {
                        if editor.is_dirty() {
                            let Screen::Editor(editor) =
                                std::mem::replace(&mut self.screen, Screen::Navigation)
                            else {
                                unreachable!()
                            };
                            self.screen = Screen::ConfirmDiscard {
                                editor,
                                discard: false,
                            };
                        } else {
                            self.screen = Screen::Navigation;
                            self.refresh(store)?;
                        }
                    }
                    Some(Action::NextField) => {
                        editor.focus_field((editor.active + 1) % FIELD_KEYS.len(), self.today);
                    }
                    Some(Action::PreviousField) => {
                        editor.focus_field(
                            (editor.active + FIELD_KEYS.len() - 1) % FIELD_KEYS.len(),
                            self.today,
                        );
                    }
                    _ => {
                        if editor.active == 3 {
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && !key.modifiers.contains(KeyModifiers::ALT)
                                && matches!(key.code, KeyCode::Char('z' | 'Z' | 'y' | 'Y'))
                            {
                                editor.fields[3].key(key, false);
                            }
                        } else if editor.active == 1
                            && matches!(key.code, KeyCode::Up | KeyCode::Down)
                        {
                            editor.fields[1].move_wrapped_vertical(
                                key.code == KeyCode::Down,
                                self.description_width,
                                key.modifiers.contains(KeyModifiers::SHIFT),
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
                        self.screen = Screen::ListPicker;
                        self.refresh(store)?;
                        self.select_picker_identity(RowIdentity::List(list.id));
                        self.show_message(tr(config.language, "saved"), false, Instant::now());
                    }
                    Some(Action::Discard | Action::Back) => {
                        self.screen = Screen::ListPicker;
                        self.refresh(store)?;
                    }
                    _ => {
                        buffer.key(key, false);
                    }
                }
                return Ok(false);
            }
            Screen::Help { .. } => {
                match action {
                    Some(Action::Close | Action::Back) => {
                        let Screen::Help { previous } =
                            std::mem::replace(&mut self.screen, Screen::Navigation)
                        else {
                            unreachable!()
                        };
                        self.screen = *previous;
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
            Action::Lists => {
                self.refresh(store)?;
                self.open_picker();
            }
            Action::AllTasks => self.set_view(View::Planned, store)?,
            Action::Back | Action::History if matches!(self.view, View::History(_)) => {
                let origin = self
                    .history_origin
                    .as_ref()
                    .map_or(View::Planned, |(view, _)| *view);
                self.set_view(origin, store)?;
            }
            Action::History if self.view != View::Trash => self.open_history(store)?,
            Action::Back if self.view != View::Planned => self.set_view(View::Planned, store)?,
            Action::Open => self.open_selection(store)?,
            Action::NewTask if matches!(self.view, View::Planned | View::List(_)) => {
                self.new_task()
            }
            Action::Filter => self.screen = Screen::Filter(TextBuffer::new(&self.filter)),
            Action::Help => {
                self.screen = Screen::Help {
                    previous: Box::new(Screen::Navigation),
                };
                self.help_state.select(Some(0));
            }
            _ => self.task_action(action, store, config)?,
        }
        Ok(false)
    }

    fn paste(&mut self, text: &str) {
        match &mut self.screen {
            Screen::Editor(editor) if editor.active != 3 => {
                let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                let normalized = if editor.active == 1 {
                    normalized
                } else {
                    normalized.replace('\n', " ")
                };
                editor.fields[editor.active].insert(&normalized);
            }
            Screen::Filter(buffer) | Screen::ListName { buffer, .. } => {
                buffer.insert(&text.replace(['\r', '\n'], " "));
            }
            _ => {}
        }
    }

    fn finish_discard_confirmation(&mut self, discard: bool, store: &Store) -> Result<()> {
        let Screen::ConfirmDiscard { editor, .. } =
            std::mem::replace(&mut self.screen, Screen::Navigation)
        else {
            unreachable!()
        };
        if discard {
            self.refresh(store)?;
        } else {
            self.screen = Screen::Editor(editor);
        }
        Ok(())
    }

    fn set_view(&mut self, view: View, store: &Store) -> Result<()> {
        self.screen = Screen::Navigation;
        self.last_click = None;
        if view == self.view {
            return self.refresh(store);
        }
        if let Some((origin, bookmark)) = self.history_origin.take() {
            if view == origin {
                let selected_task = self.restore_view_state(view, Some(bookmark));
                self.refresh(store)?;
                if self.view == view {
                    self.select_task_id(selected_task);
                }
                return Ok(());
            }
            if origin == View::Planned {
                self.planned_bookmark = Some(bookmark);
            }
        }
        if self.view == View::Planned {
            self.planned_bookmark = Some(self.bookmark());
        }
        let bookmark = if view == View::Planned {
            self.planned_bookmark.take()
        } else {
            None
        };
        let selected_task = self.restore_view_state(view, bookmark);
        self.refresh(store)?;
        if self.view == view {
            self.select_task_id(selected_task);
        }
        Ok(())
    }

    fn bookmark(&self) -> PlannedBookmark {
        PlannedBookmark {
            filter: self.filter.clone(),
            query: self.query.clone(),
            selected_task: self.selected_task().map(|task| task.id),
            selection: self.selection,
            list_state: self.list_state,
        }
    }

    fn restore_view_state(&mut self, view: View, bookmark: Option<PlannedBookmark>) -> Option<i64> {
        self.view = view;
        self.selection = 0;
        self.list_state = ListState::default();
        self.visible.clear();
        self.filter.clear();
        self.query = Query::default();
        let selected_task = bookmark
            .as_ref()
            .and_then(|bookmark| bookmark.selected_task);
        if let Some(bookmark) = bookmark {
            self.selection = bookmark.selection;
            self.list_state = bookmark.list_state;
            self.filter = bookmark.filter;
            self.query = bookmark.query;
        }
        selected_task
    }

    fn select_task_id(&mut self, selected_task: Option<i64>) {
        if let Some(index) = self
            .visible
            .iter()
            .position(|task| Some(task.id) == selected_task)
        {
            self.selection = index;
        }
        self.clamp_selection();
    }

    fn open_history(&mut self, store: &Store) -> Result<()> {
        self.history_origin = Some((self.view, self.bookmark()));
        self.restore_view_state(View::History(self.view.scope().0), None);
        self.refresh(store)?;
        Ok(())
    }

    fn finish_list_deletion(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Screen::DeleteListConfirm {
            preview, delete, ..
        } = &self.screen
        else {
            return Ok(());
        };
        if !delete {
            self.screen = Screen::ListPicker;
            self.refresh(store)?;
            return Ok(());
        }
        let destination = preview.destination_id;
        match store.delete_list(preview)? {
            ListDeletionOutcome::Deleted => {
                self.screen = Screen::ListPicker;
                self.refresh(store)?;
                self.select_picker_identity(RowIdentity::List(destination));
                self.show_message(tr(config.language, "list_deleted"), false, Instant::now());
            }
            ListDeletionOutcome::Changed(preview) => {
                self.screen = Screen::DeleteListConfirm {
                    preview: Box::new(preview),
                    delete: false,
                    changed: true,
                };
                self.refresh(store)?;
            }
            ListDeletionOutcome::Missing => {
                self.screen = Screen::ListPicker;
                self.refresh(store)?;
                self.show_message(
                    tr(config.language, "list_delete_missing"),
                    true,
                    Instant::now(),
                );
            }
        }
        Ok(())
    }

    fn open_selection(&mut self, _store: &Store) -> Result<()> {
        if let Some(task) = self.selected_task().filter(|task| !task.deleted).cloned() {
            let list_id = task.list_id;
            self.screen = Screen::Editor(Box::new(TaskEditor::new(Some(task), list_id)));
            self.message = None;
        }
        Ok(())
    }

    fn new_task(&mut self) {
        let list = match self.view {
            View::List(id) => self.lists.iter().find(|list| list.id == id),
            _ => self.lists.iter().min_by_key(|list| list.id),
        };
        if let Some(list) = list {
            self.screen = Screen::Editor(Box::new(TaskEditor::new(None, list.id)));
            self.message = None;
        }
    }

    fn save_editor(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Screen::Editor(editor) = &self.screen else {
            return Ok(());
        };
        if let Some(original) = &editor.original
            && store.task(original.id)?.version != original.version
        {
            bail!(
                "Task {} changed in another process. Reload it before saving.",
                original.id
            );
        }
        let input = editor.input(&store.lists()?, config.language, self.today)?;
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
        if let Some(preset) = due_preset(action) {
            let updated = store.set_due(task.id, task.version, preset.resolve(self.today)?)?;
            self.refresh(store)?;
            if updated.version != task.version {
                self.show_message(tr(config.language, "updated"), false, Instant::now());
            }
            return Ok(());
        }
        let closing = task.status == Status::Open
            && matches!(action, Action::ToggleDone | Action::ToggleCancelled);
        let next_open = closing
            .then(|| {
                self.visible
                    .iter()
                    .skip(self.selection + 1)
                    .find(|task| task.status == Status::Open)
                    .or_else(|| {
                        self.visible[..self.selection]
                            .iter()
                            .rev()
                            .find(|task| task.status == Status::Open)
                    })
                    .map(|task| task.id)
            })
            .flatten();
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
            Action::Reopen if matches!(self.view, View::History(_)) => {
                store.set_status(task.id, task.version, Status::Open)?;
            }
            Action::MoveUp | Action::MoveDown => {
                if task.status != Status::Open || matches!(self.view, View::History(_)) {
                    return Ok(());
                }
                let index = if matches!(action, Action::MoveUp) {
                    self.selection.checked_sub(1)
                } else {
                    self.selection.checked_add(1)
                };
                let Some(other) = index.and_then(|index| self.visible.get(index)) else {
                    return Ok(());
                };
                if task.due != other.due
                    || (task.status == Status::Open) != (other.status == Status::Open)
                {
                    return Ok(());
                }
                store.swap_positions(&task, other)?;
            }
            _ => return Ok(()),
        }
        self.refresh(store)?;
        self.select_task_id(next_open);
        self.show_message(
            tr(
                config.language,
                if action == Action::Reopen {
                    "reopened"
                } else {
                    "updated"
                },
            ),
            false,
            Instant::now(),
        );
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>, config: &Config) {
        self.mouse_targets.clear();
        self.picker.area = None;
        if let Some(picker) = self.screen.editor_picker_mut() {
            picker.area = None;
        }
        let area = frame.area();
        let language = config.language;
        if area.width < 38
            || area.height < 10
            || (matches!(
                self.screen,
                Screen::Editor(_)
                    | Screen::EditorListPicker { .. }
                    | Screen::ConfirmDiscard { .. }
                    | Screen::DeleteListConfirm { .. }
            ) && area.height < 18)
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
                Constraint::Length(if matches!(self.screen, Screen::Editor(_)) {
                    3
                } else {
                    2
                }),
                Constraint::Min(1),
                Constraint::Length(if matches!(self.screen, Screen::ListPicker) {
                    4
                } else {
                    3
                }),
            ])
            .split(area);
        let view_title = match self.view {
            View::Planned | View::History(None) => tr(language, "all_tasks").to_owned(),
            View::Trash => format!("{} > {}", tr(language, "all_tasks"), tr(language, "trash")),
            View::List(id) | View::History(Some(id)) => {
                self.lists.iter().find(|list| list.id == id).map_or_else(
                    || tr(language, "all_tasks").to_owned(),
                    |list| {
                        format!(
                            "{} > {}",
                            tr(language, "all_tasks"),
                            list.name.replace(['\r', '\n'], " ")
                        )
                    },
                )
            }
        };
        let view_title = if matches!(self.view, View::History(_)) {
            format!("{view_title} > {}", tr(language, "history"))
        } else {
            view_title
        };
        let editor = match &self.screen {
            Screen::Editor(editor)
            | Screen::ConfirmDiscard { editor, .. }
            | Screen::EditorListPicker { editor, .. } => Some(editor),
            _ => None,
        };
        let header = if let Some(editor) = editor {
            let title = editor.original.as_ref().map_or_else(
                || tr(language, "new_task").to_owned(),
                |task| format!("{} #{}", tr(language, "editor"), task.id),
            );
            format!("Husker  |  {title}  |  {view_title}")
        } else {
            format!(
                "Husker  |  {view_title}  |  {}",
                date_label(self.today, language)
            )
        };
        let mut header_lines = vec![Line::from(header)];
        if matches!(&self.screen, Screen::Editor(editor) if editor.active == 2) {
            header_lines.push(Line::from(format!(
                "{} {} | {} {} | {} {} | {} {}",
                config.keys_for(Action::DueToday),
                tr(language, "today"),
                config.keys_for(Action::DueTomorrow),
                tr(language, "tomorrow"),
                config.keys_for(Action::DueNextWeek),
                tr(language, "next_monday"),
                config.keys_for(Action::ClearDue),
                tr(language, "clear_due"),
            )));
        } else if matches!(&self.screen, Screen::Editor(editor) if editor.active == 3) {
            header_lines.push(Line::from(format!(
                "{} / {} {} | {}",
                config.keys_for(Action::Open),
                config.keys_for(Action::Lists),
                tr(language, "list_picker"),
                tr(language, "list_keys_hint")
            )));
        } else if matches!(self.screen, Screen::Editor(_)) {
            header_lines.push(Line::from(format!(
                "{} {} | {}",
                config.keys_for(Action::CopySelection),
                tr(language, "copy_short"),
                tr(language, "editor_keys_hint"),
            )));
        }
        frame.render_widget(
            Paragraph::new(header_lines)
                .block(Block::default().borders(Borders::BOTTOM))
                .style(Style::default().fg(Color::Cyan)),
            sections[0],
        );
        match &self.screen {
            Screen::EditorListPicker { .. } => {
                self.draw_editor_list_picker(frame, sections[1], language)
            }
            Screen::Language { selected, .. } => {
                draw_language_picker(frame, sections[1], *selected, language)
            }
            Screen::ListPicker => {
                let list_state = self.list_state;
                self.draw_task_view(frame, sections[1], config);
                self.list_state = list_state;
                self.mouse_targets.clear();
                self.draw_list_picker(frame, sections[1], language);
            }
            Screen::DeleteListConfirm {
                preview,
                delete,
                changed,
            } => {
                draw_list_deletion(frame, sections[1], preview, *delete, *changed, language);
            }
            Screen::ConfirmDiscard { editor, discard } => {
                let _ = draw_editor(frame, sections[1], editor, &self.lists, language);
                draw_discard_confirmation(frame, sections[1], *discard, language);
            }
            Screen::Editor(editor) => {
                self.mouse_targets = draw_editor(frame, sections[1], editor, &self.lists, language);
                if let Some((rect, _)) = self
                    .mouse_targets
                    .iter()
                    .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 1, .. }))
                {
                    self.description_width = usize::from(rect.width.saturating_sub(1));
                }
            }
            Screen::Help { .. } => {
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
                self.draw_navigation(frame, body, config);
            }
        }
        let hint = match self.screen {
            Screen::EditorListPicker { .. } => format!(
                "{} {} | {} / {} {} | {} {}",
                config.keys_for(Action::Open),
                tr(language, "action_open"),
                config.keys_for(Action::Back),
                config.keys_for(Action::Lists),
                tr(language, "action_back"),
                config.keys_for(Action::Help),
                tr(language, "help")
            ),
            Screen::Language { .. } => format!(
                "{} / {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Up),
                config.keys_for(Action::Down),
                config.keys_for(Action::Apply),
                tr(language, "action_apply"),
                config.keys_for(Action::Back),
                tr(language, "action_back"),
            ),
            Screen::ListPicker => format!(
                "{} {}  |  {} {}  |  {} {}\n{} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Open),
                tr(language, "action_open"),
                config.keys_for(Action::Back),
                tr(language, "action_back"),
                config.keys_for(Action::NewList),
                tr(language, "new_list"),
                config.keys_for(Action::RenameList),
                tr(language, "rename_list"),
                config.keys_for(Action::DeleteList),
                tr(language, "action_delete_list"),
                config.keys_for(Action::Help),
                tr(language, "help"),
            ),
            Screen::DeleteListConfirm { .. } => format!(
                "{}  |  {} {}",
                tr(language, "discard_confirm_hint"),
                config.keys_for(Action::Back),
                tr(language, "action_back"),
            ),
            Screen::ConfirmDiscard { .. } => format!(
                "{}  |  {} {}",
                tr(language, "discard_confirm_hint"),
                config.keys_for(Action::Discard),
                tr(language, "keep_editing"),
            ),
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
                "{} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Apply),
                tr(language, "action_apply"),
                config.keys_for(Action::Back),
                tr(language, "action_discard"),
                config.keys_for(Action::CopySelection),
                tr(language, "copy_short")
            ),
            Screen::Help { .. } => format!(
                "{} / {}  |  {} {}",
                config.keys_for(Action::Up),
                config.keys_for(Action::Down),
                config.keys_for(Action::Close),
                tr(language, "action_close")
            ),
            Screen::Navigation if self.view == View::Trash => format!(
                "{} {}  |  {} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Lists),
                tr(language, "action_lists"),
                config.keys_for(Action::Back),
                tr(language, "action_back"),
                config.keys_for(Action::Restore),
                tr(language, "action_restore"),
                config.keys_for(Action::Help),
                tr(language, "help"),
            ),
            Screen::Navigation if matches!(self.view, View::History(_)) => format!(
                "{} {}  |  {} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Lists),
                tr(language, "action_lists"),
                config.keys_for(Action::Back),
                tr(language, "action_back"),
                config.keys_for(Action::Reopen),
                tr(language, "action_reopen"),
                config.keys_for(Action::Help),
                tr(language, "help"),
            ),
            Screen::Navigation if matches!(self.view, View::List(_)) => format!(
                "{} {}  |  {} {}  |  {} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Lists),
                tr(language, "action_lists"),
                config.keys_for(Action::Back),
                tr(language, "action_back"),
                config.keys_for(Action::NewTask),
                tr(language, "new_task"),
                config.keys_for(Action::History),
                tr(language, "history"),
                config.keys_for(Action::Help),
                tr(language, "help"),
            ),
            _ => format!(
                "{} {}  |  {} {}  |  {} {}  |  {} {}  |  {} {}",
                config.keys_for(Action::Lists),
                tr(language, "action_lists"),
                config.keys_for(Action::NewTask),
                tr(language, "new_task"),
                config.keys_for(Action::History),
                tr(language, "history"),
                config.keys_for(Action::Help),
                tr(language, "help"),
                config.keys_for(Action::Quit),
                tr(language, "action_quit")
            ),
        };
        let mut footer: Vec<_> = hint
            .lines()
            .map(|line| {
                Line::from(Span::styled(
                    line.to_owned(),
                    Style::default().fg(Color::DarkGray),
                ))
            })
            .collect();
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

    fn draw_task_view(&mut self, frame: &mut Frame<'_>, area: Rect, config: &Config) {
        let language = config.language;
        let body = if !self.filter.is_empty() {
            let parts = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
            frame.render_widget(
                Paragraph::new(self.filter.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(tr(language, "filter")),
                ),
                parts[0],
            );
            parts[1]
        } else {
            area
        };
        self.draw_navigation(frame, body, config);
    }

    fn draw_list_picker(&mut self, frame: &mut Frame<'_>, area: Rect, language: Language) {
        let mut rows = vec![ListItem::new(tr(language, "all_tasks"))];
        rows.extend(ordinary_list_rows(&self.lists, language));
        rows.push(ListItem::new(format!(
            "{} ({})",
            tr(language, "trash"),
            self.tasks.iter().filter(|task| task.deleted).count()
        )));
        self.mouse_targets = draw_list_popup(frame, area, rows, &mut self.picker, language)
            .into_iter()
            .map(|(rect, index)| (rect, MouseTarget::PickerRow(index)))
            .collect();
    }

    fn draw_editor_list_picker(&mut self, frame: &mut Frame<'_>, area: Rect, language: Language) {
        let Screen::EditorListPicker { editor, picker } = &mut self.screen else {
            unreachable!()
        };
        let _ = draw_editor(frame, area, editor, &self.lists, language);
        let rows = ordinary_list_rows(&self.lists, language);
        self.mouse_targets = draw_list_popup(frame, area, rows, picker, language)
            .into_iter()
            .map(|(rect, index)| (rect, MouseTarget::EditorListRow(index)))
            .collect();
    }

    fn draw_navigation(&mut self, frame: &mut Frame<'_>, area: Rect, config: &Config) {
        let language = config.language;
        let history = matches!(self.view, View::History(_));
        if self.visible.is_empty() {
            self.list_state.select(None);
            let text = if history {
                format!(
                    "{}\n{} {}",
                    tr(language, "no_history"),
                    config.keys_for(Action::Back),
                    tr(language, "action_back")
                )
            } else {
                tr(language, "no_tasks").to_owned()
            };
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), area);
            return;
        }
        let trash = self.view == View::Trash;
        let mut previous_group = None;
        let mut rows = Vec::new();
        let mut row_tasks = Vec::new();
        for (index, task) in self.visible.iter().enumerate() {
            let group = if history {
                history_group_label(task.status_changed_on, self.today, language)
            } else {
                ordinary_group_label(task, self.today, language)
            };
            if !trash && previous_group.as_ref() != Some(&group) {
                rows.push(ListItem::new(Line::from(Span::styled(
                    group.clone(),
                    Style::default().fg(Color::Yellow),
                ))));
                row_tasks.push(None);
            }
            previous_group = Some(group);
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
            let list_label = if matches!(self.view, View::Planned | View::History(None)) {
                let name = self
                    .lists
                    .iter()
                    .find(|list| list.id == task.list_id)
                    .map(|list| list.name.replace(['\r', '\n'], " "))
                    .unwrap_or_else(|| format!("#{}", task.list_id));
                format!("[{name}] ")
            } else {
                String::new()
            };
            rows.push(ListItem::new(Line::from(format!(
                "{marker} {list_label}{title}{date}"
            ))));
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

fn ordinary_list_rows(lists: &[TaskList], language: Language) -> Vec<ListItem<'static>> {
    let default_id = lists.iter().map(|list| list.id).min();
    lists
        .iter()
        .map(|list| {
            let marker = if Some(list.id) == default_id {
                format!(" [{}]", tr(language, "default_list"))
            } else {
                String::new()
            };
            ListItem::new(format!(
                "{} ({}){marker}",
                list.name.replace(['\r', '\n'], " "),
                list.count
            ))
        })
        .collect()
}

fn draw_list_popup(
    frame: &mut Frame<'_>,
    area: Rect,
    rows: Vec<ListItem<'static>>,
    picker: &mut ListPicker,
    language: Language,
) -> Vec<(Rect, usize)> {
    let count = rows.len();
    let width = area.width.saturating_sub(4).min(60);
    let height = area
        .height
        .min((count.max(1) + 2).min(u16::MAX as usize) as u16);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(tr(language, "list_picker"));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    if rows.is_empty() {
        frame.render_widget(Paragraph::new(tr(language, "no_lists")).block(block), popup);
    } else {
        frame.render_stateful_widget(
            List::new(rows)
                .block(block)
                .highlight_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> "),
            popup,
            &mut picker.state,
        );
    }
    picker.area = Some(inner);
    (picker.state.offset()..count)
        .take(usize::from(inner.height))
        .enumerate()
        .map(|(row, index)| {
            (
                Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
                index,
            )
        })
        .collect()
}

fn draw_input(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    buffer: &TextBuffer,
    active: bool,
) -> (Rect, usize, usize) {
    draw_input_placeholder(frame, area, title, buffer, active, None)
}

fn draw_input_placeholder(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    buffer: &TextBuffer,
    active: bool,
    placeholder: Option<&str>,
) -> (Rect, usize, usize) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .title(title)
        .border_style(Style::default().fg(if active { Color::Cyan } else { Color::DarkGray }));
    let inner = block.inner(area);
    let layout = TextLayout::new(&buffer.text, usize::MAX);
    let (row, column) = buffer.cursor_position();
    let scroll_y = row.saturating_sub(inner.height.saturating_sub(1) as usize);
    let scroll_x = column.saturating_sub(inner.width.saturating_sub(1) as usize);
    let lines = if let Some(placeholder) = placeholder.filter(|_| buffer.text.is_empty()) {
        vec![Line::from(Span::styled(
            placeholder,
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        layout
            .lines
            .iter()
            .map(|line| Line::from(line.text.as_str()))
            .collect::<Vec<_>>()
    };
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((
            scroll_y.min(u16::MAX as usize) as u16,
            scroll_x.min(u16::MAX as usize) as u16,
        )),
        area,
    );
    draw_text_selection(frame, inner, buffer, &layout, scroll_y, scroll_x);
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
    lists: &[TaskList],
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
        } else if index == 3 {
            let block = Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title(tr(language, key))
                .border_style(Style::default().fg(if editor.active == 3 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }));
            let inner = block.inner(fields[index]);
            let name = lists
                .iter()
                .find(|list| Some(list.id) == editor.selected_list_id())
                .map(|list| list.name.replace(['\r', '\n'], " "))
                .unwrap_or_else(|| tr(language, "list_unavailable").to_owned());
            frame.render_widget(Paragraph::new(name).block(block), fields[index]);
            (inner, 0, 0)
        } else if index == 2 {
            draw_input_placeholder(
                frame,
                fields[index],
                &format!("{}  {}", tr(language, key), tr(language, "due_format_hint")),
                &editor.fields[index],
                index == editor.active,
                Some("YYYY-MM-DD"),
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
    draw_text_selection(frame, inner, buffer, &layout, scroll_row, 0);
    if active && inner.width > 0 && inner.height > 0 {
        frame.set_cursor_position((
            inner.x + column.min(usize::from(inner.width - 1)) as u16,
            inner.y + (row - scroll_row) as u16,
        ));
    }
    (inner, scroll_row, 0)
}

fn draw_text_selection(
    frame: &mut Frame<'_>,
    inner: Rect,
    buffer: &TextBuffer,
    layout: &TextLayout,
    scroll_row: usize,
    scroll_column: usize,
) {
    let Some(selection) = buffer.selection() else {
        return;
    };
    for visible_row in 0..usize::from(inner.height) {
        let Some(columns) = layout.selection_columns(scroll_row + visible_row, &selection) else {
            continue;
        };
        let start = columns.start.max(scroll_column);
        let end = columns.end.min(scroll_column + usize::from(inner.width));
        if start < end {
            frame.buffer_mut().set_style(
                Rect::new(
                    inner.x + (start - scroll_column) as u16,
                    inner.y + visible_row as u16,
                    (end - start) as u16,
                    1,
                ),
                Style::default().fg(Color::White).bg(Color::Blue),
            );
        }
    }
}

fn draw_language_picker(frame: &mut Frame<'_>, area: Rect, selected: Language, language: Language) {
    let width = area.width.saturating_sub(4).min(32);
    let height = area.height.min(4);
    let dialog = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let mut state =
        ListState::default().with_selected(Some(if selected == Language::En { 0 } else { 1 }));
    frame.render_widget(Clear, dialog);
    frame.render_stateful_widget(
        List::new([ListItem::new("English"), ListItem::new("Dansk")])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(tr(language, "language_picker")),
            )
            .highlight_symbol("> ")
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        dialog,
        &mut state,
    );
}

fn draw_list_deletion(
    frame: &mut Frame<'_>,
    area: Rect,
    preview: &ListDeletionPreview,
    delete: bool,
    changed: bool,
    language: Language,
) {
    let width = area.width.saturating_sub(4).min(72);
    let mut paragraphs = vec![
        (
            format!(
                "{} #{}",
                preview.list_name.replace(['\r', '\n'], " "),
                preview.list_id
            ),
            Color::Cyan,
        ),
        (
            format!(
                "{} {}",
                tr(language, "move_tasks_to"),
                preview.destination_name.replace(['\r', '\n'], " ")
            ),
            Color::White,
        ),
        (
            format!("{}: {}", tr(language, "unfinished"), preview.open_count),
            if preview.open_count > 0 {
                Color::Yellow
            } else {
                Color::White
            },
        ),
        (
            format!("{}: {}", tr(language, "done"), preview.done_count),
            Color::White,
        ),
        (
            format!("{}: {}", tr(language, "cancelled"), preview.cancelled_count),
            Color::White,
        ),
        (
            format!("{}: {}", tr(language, "trashed"), preview.trashed_count),
            Color::White,
        ),
        (tr(language, "list_delete_hint").to_owned(), Color::White),
    ];
    if changed {
        paragraphs.push((
            tr(language, "list_delete_changed").to_owned(),
            Color::Yellow,
        ));
    }
    let mut lines: Vec<Line<'_>> = paragraphs
        .into_iter()
        .flat_map(|(text, color)| {
            TextLayout::new(&text, usize::from(width.saturating_sub(2)))
                .lines
                .into_iter()
                .map(move |line| Line::from(Span::styled(line.text, Style::default().fg(color))))
        })
        .collect();
    let height = lines.len().saturating_add(4);
    if height > usize::from(area.height) {
        frame.render_widget(
            Paragraph::new(tr(language, "terminal_too_small")).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let dialog = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height as u16) / 2,
        width,
        height as u16,
    );
    let choice = |label, selected| {
        Span::styled(
            format!("  {}  ", tr(language, label)),
            if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )
    };
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        choice("no", !delete),
        Span::raw("    "),
        choice("yes", delete),
    ]));
    frame.render_widget(Clear, dialog);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr(language, "delete_list_confirm")),
        ),
        dialog,
    );
    frame.set_cursor_position((dialog.x + 1, dialog.y + dialog.height - 2));
}

fn draw_discard_confirmation(frame: &mut Frame<'_>, area: Rect, discard: bool, language: Language) {
    let width = area.width.saturating_sub(4).min(48);
    let height = area.height.min(5);
    let dialog = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let choice = |label, selected| {
        Span::styled(
            format!("  {}  ", tr(language, label)),
            if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )
    };
    frame.render_widget(Clear, dialog);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                choice("no", !discard),
                Span::raw("    "),
                choice("yes", discard),
            ]),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr(language, "discard_confirm")),
        ),
        dialog,
    );
    frame.set_cursor_position((dialog.x + 1, dialog.y + 1));
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
    use crate::planning::Bucket;
    use crate::query::parse_date;
    use ratatui::backend::TestBackend;
    use tempfile::TempDir;

    struct RecordingClipboard {
        writes: Arc<std::sync::Mutex<Vec<String>>>,
        fail: bool,
    }

    impl TextClipboard for RecordingClipboard {
        fn set_text(&mut self, text: &str) -> Result<()> {
            if self.fail {
                bail!("Clipboard unavailable in test");
            }
            self.writes.lock().unwrap().push(text.to_owned());
            Ok(())
        }
    }

    fn record_clipboard(app: &mut App, fail: bool) -> Arc<std::sync::Mutex<Vec<String>>> {
        let writes = Arc::new(std::sync::Mutex::new(Vec::new()));
        app.clipboard = Box::new(RecordingClipboard {
            writes: writes.clone(),
            fail,
        });
        writes
    }

    #[test]
    fn copy_selection_uses_source_text_without_changing_editor_state() {
        let (_directory, mut store, config) = fixture();
        let mut value = input(1, "Copy test");
        value.description =
            "Skip Køb æbler og pærer fra den lange indkøbsliste\n👩‍💻 e\u{301} End".into();
        let task = store.create_task(&value).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let writes = record_clipboard(&mut app, false);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[1].set_cursor(5, false);
        editor.fields[1].set_cursor(value.description.len() - 4, true);
        let before = editor.fields.clone();
        rendered(&mut app, &config, 38, 24);
        assert!(!app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut store,
            &config
        ));
        assert_eq!(
            *writes.lock().unwrap(),
            ["Køb æbler og pærer fra den lange indkøbsliste\n👩‍💻 e\u{301}"]
        );
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields, before);
        assert!(!editor.is_dirty());
        assert_eq!(store.task(task.id).unwrap(), task);
        assert_eq!(
            app.message.as_ref().unwrap().text,
            tr(config.language, "copied")
        );
        assert!(!app.message.as_ref().unwrap().is_error);
    }

    #[test]
    fn copy_failure_keeps_selection_history_and_unsaved_text() {
        let (_directory, mut store, mut config) = fixture();
        config.language = Language::Da;
        let mut app = App::new(&store, today()).unwrap();
        let writes = record_clipboard(&mut app, true);
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Unsaved draft");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        let before = editor.fields.clone();
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields, before);
        assert!(editor.is_dirty());
        assert!(writes.lock().unwrap().is_empty());
        let message = app.message.as_ref().unwrap();
        assert!(message.is_error);
        assert!(message.text.contains(tr(Language::Da, "copy_failed")));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(&app.screen, Screen::Editor(editor) if !editor.is_dirty()));
    }

    #[test]
    fn copying_filter_and_list_selections_preserves_terminal_paste() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        let writes = record_clipboard(&mut app, false);
        for list_input in [false, true] {
            let mut buffer = TextBuffer::new("first second");
            buffer.set_cursor(6, false);
            buffer.set_cursor(12, true);
            app.screen = if list_input {
                Screen::ListName { id: None, buffer }
            } else {
                Screen::Filter(buffer)
            };
            app.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
            app.paste("replacement");
            let buffer = match &app.screen {
                Screen::ListName { buffer, .. } | Screen::Filter(buffer) => buffer,
                _ => panic!(),
            };
            assert_eq!(buffer.text, "first replacement");
            app.handle_key(
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
            let buffer = match &app.screen {
                Screen::ListName { buffer, .. } | Screen::Filter(buffer) => buffer,
                _ => panic!(),
            };
            assert_eq!(buffer.text, "first second");
            assert_eq!(buffer.selection(), Some(6..12));
        }
        assert_eq!(*writes.lock().unwrap(), ["second", "second"]);
    }

    #[test]
    fn copy_without_selection_and_outside_inputs_never_writes_clipboard() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        let writes = record_clipboard(&mut app, false);
        let copy = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        app.handle_key(copy, &mut store, &config);
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Draft");
        app.handle_key(copy, &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(copy, &mut store, &config);
        assert!(matches!(app.screen, Screen::ConfirmDiscard { .. }));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::F(4)), &mut store, &config);
        app.handle_key(copy, &mut store, &config);
        assert!(matches!(app.screen, Screen::CopyText { .. }));
        assert!(writes.lock().unwrap().is_empty());
    }

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
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.message.as_ref().unwrap().created_at, now);
        let view = app.view;
        let selection = app.selection;
        assert_eq!(view, View::List(list.id));
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
        assert!(matches!(
            app.screen,
            Screen::ConfirmDiscard { discard: false, .. }
        ));
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
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
        let mut editor = TaskEditor::new(Some(task), list.id);
        editor.fields[2] = TextBuffer::default();
        let fields = editor.input(&[list], config.language, today()).unwrap();
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
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(
                store
                    .lists()
                    .unwrap()
                    .iter()
                    .find(|list| list.name == "Ærinder")
                    .unwrap()
                    .id
            ))
        );
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
        assert!(matches!(app.screen, Screen::ListPicker));
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
        app.set_view(View::Trash, &store).unwrap();
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
        app.open_picker();
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
    fn escape_requires_explicit_confirmation_for_existing_and_new_task_edits() {
        let (_directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Keep this title")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Unsaved description");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(
            app.screen,
            Screen::ConfirmDiscard { discard: false, .. }
        ));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == "Unsaved description")
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Discard new task");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(
            app.screen,
            Screen::ConfirmDiscard { discard: false, .. }
        ));
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.tasks().unwrap(), vec![task]);
    }

    #[test]
    fn clean_or_fully_undone_drafts_close_without_confirmation() {
        let (_directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Temporary edit");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Temporary new title");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.tasks().unwrap(), vec![task]);
    }

    #[test]
    fn discard_confirmation_accepts_both_shift_tab_event_forms() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Keep this draft");
        for code in [KeyCode::BackTab, KeyCode::Tab] {
            app.handle_key(key(KeyCode::Esc), &mut store, &config);
            assert!(matches!(
                app.screen,
                Screen::ConfirmDiscard { discard: false, .. }
            ));
            let shift_tab = KeyEvent::new(code, KeyModifiers::SHIFT);
            app.handle_key(shift_tab, &mut store, &config);
            assert!(matches!(
                app.screen,
                Screen::ConfirmDiscard { discard: true, .. }
            ));
            app.handle_key(shift_tab, &mut store, &config);
            assert!(matches!(
                app.screen,
                Screen::ConfirmDiscard { discard: false, .. }
            ));
            app.handle_key(key(KeyCode::Enter), &mut store, &config);
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[0].text == "Keep this draft")
            );
        }
        assert!(store.tasks().unwrap().is_empty());
    }

    #[test]
    fn confirmation_preserves_draft_history_and_version_during_external_refresh() {
        let (directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Local description");
        app.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        let Screen::Editor(draft) = &app.screen else {
            panic!()
        };
        let draft = draft.clone();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        external
            .update_task(task.id, task.version, &input(1, "Changed elsewhere"))
            .unwrap();
        app.handle_key(key(KeyCode::F(5)), &mut store, &config);
        app.paste("Ignored paste");
        app.handle_key(key(KeyCode::Char('x')), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(app.external_update);
        let Screen::ConfirmDiscard { editor, discard } = &app.screen else {
            panic!()
        };
        assert!(*discard);
        assert_eq!(editor.fields, draft.fields);
        assert_eq!(editor.original, draft.original);
        assert_eq!(editor.active, draft.active);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields, draft.fields);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(app.message.as_ref().unwrap().is_error);
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert_eq!(store.task(task.id).unwrap().title, "Changed elsewhere");
    }

    #[test]
    fn editor_heading_has_id_but_copy_text_and_new_task_do_not() {
        let (_directory, mut store, mut config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        assert!(rendered(&mut app, &config, 80, 24).contains("Sometime"));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(rendered(&mut app, &config, 80, 24).contains(&format!("Edit task #{}", task.id)));
        app.handle_key(key(KeyCode::F(4)), &mut store, &config);
        assert!(matches!(&app.screen, Screen::CopyText { text, .. } if text == "Original\n\n"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        config.language = Language::Da;
        assert!(rendered(&mut app, &config, 80, 24).contains("Engang"));
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(screen.contains("Ny opgave"));
        assert!(!screen.contains(&format!("#{}", task.id)));
    }

    #[test]
    fn selected_description_uses_visual_rows_after_resizing() {
        let (_directory, mut store, config) = fixture();
        let mut value = input(1, "Selection");
        value.description = "one two three four five six seven eight ".repeat(4);
        store.create_task(&value).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[1].set_cursor(4, false);
        editor.fields[1].set_cursor(80, true);
        for width in [38, 80] {
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal.draw(|frame| app.draw(frame, &config)).unwrap();
            let (rect, MouseTarget::EditorField { scroll_row, .. }) = app
                .mouse_targets
                .iter()
                .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 1, .. }))
                .copied()
                .unwrap()
            else {
                panic!()
            };
            let layout = TextLayout::new(&value.description, usize::from(rect.width - 1));
            let mut highlighted = 0;
            for y in 0..rect.height {
                let selected = layout.selection_columns(scroll_row + usize::from(y), &(4..80));
                for x in 0..rect.width {
                    let expected = selected
                        .as_ref()
                        .is_some_and(|columns| columns.contains(&usize::from(x)));
                    let actual =
                        terminal.backend().buffer()[(rect.x + x, rect.y + y)].bg == Color::Blue;
                    assert_eq!(actual, expected, "width {width}, column {x}, row {y}");
                    highlighted += usize::from(actual);
                }
            }
            assert!(highlighted > 0);
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == value.description && editor.fields[1].selection() == Some(4..80))
            );
        }
    }

    #[test]
    fn unwrapped_selection_tracks_horizontal_scroll_and_tab_cells() {
        let mut buffer = TextBuffer::new("a\t界b");
        buffer.set_cursor(0, false);
        buffer.set_cursor(buffer.text.len(), true);
        let mut terminal = Terminal::new(TestBackend::new(6, 3)).unwrap();
        let completed = terminal
            .draw(|frame| {
                draw_input(frame, frame.area(), "Title", &buffer, true);
            })
            .unwrap();
        // Terminal diffs omit the trailing cell covered by a wide glyph.
        let rendered = completed.buffer;
        assert_eq!(rendered[(0, 1)].symbol(), " ");
        assert_eq!(rendered[(2, 1)].symbol(), "界");
        for x in 0..5 {
            assert_eq!(rendered[(x, 1)].bg, Color::Blue);
        }
        assert_ne!(rendered[(5, 1)].bg, Color::Blue);
        assert_eq!(terminal.backend().buffer()[(2, 1)].bg, Color::Blue);
    }

    #[test]
    fn field_history_and_selection_survive_field_changes_and_copy_view() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Initial title");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.paste("Replacement title");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.paste("Description");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[0].text == "Replacement title" && editor.fields[1].text.is_empty())
        );
        app.handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::F(4)), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[0].text == "Initial title" && editor.fields[0].selection() == Some(0..13))
        );
        app.handle_key(
            KeyEvent::new(
                KeyCode::Char('Z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[0].text == "Replacement title")
        );
    }

    #[test]
    fn picker_selection_tracks_scope_and_external_list_changes() {
        let (_directory, mut store, config) = fixture();
        store.create_list("Zulu").unwrap();
        let selected = store.create_list("Private").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(selected.id));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        store.rename_list(selected.id, "Home").unwrap();
        store.create_list("AAA").unwrap();
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(selected.id))
        );
        app.select_picker_identity(RowIdentity::Trash);
        store.create_list("Another").unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(app.picker.selection, app.lists.len() + 1);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        app.set_view(View::Planned, &store).unwrap();
        app.new_task();
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(1))
        );
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
        app.open_picker();
        rendered(&mut app, &config, 80, 24);
        let (rect, _) = app.mouse_targets[1];
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
        assert_eq!(app.view, View::List(1));
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
        app.set_view(View::Planned, &store).unwrap();
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
        buffer.move_wrapped_vertical(false, 5, false);
        assert_eq!(buffer.cursor, 7);
        buffer.cursor = buffer.text.len();
        buffer.move_wrapped_vertical(true, 5, false);
        assert_eq!(buffer.cursor, buffer.text.len());
    }

    #[test]
    fn list_shortcuts_keep_default_and_trash_fixed_and_preserve_selected_id() {
        let (_directory, mut store, config) = fixture();
        let first = store.create_list("Work").unwrap();
        let second = store.create_list("Home").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        assert_eq!(app.lists[0].id, 1);
        app.select_picker_identity(RowIdentity::List(1));
        app.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.lists[0].id, 1);
        app.select_picker_identity(RowIdentity::List(second.id));
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(
            app.lists.iter().map(|list| list.id).collect::<Vec<_>>(),
            [1, second.id, first.id]
        );
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(second.id))
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(second.id))
        );
        assert_eq!(app.picker.selection, 2);
        app.select_picker_identity(RowIdentity::Trash);
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.picker.selection, app.lists.len() + 1);
        app.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.picker.selection, app.lists.len() + 1);
    }

    #[test]
    fn picker_open_cancel_and_same_scope_choice_preserve_task_scroll_and_filter() {
        let (_directory, mut store, config) = fixture();
        for index in 0..30 {
            store
                .create_task(&input(1, &format!("Keep task {index}")))
                .unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "keep".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.selection = 22;
        rendered(&mut app, &config, 80, 12);
        let task_id = app.selected_task().unwrap().id;
        let offset = app.list_state.offset();
        let selected_row = app.list_state.selected();
        assert!(offset > 0);
        for close in [KeyCode::Esc, KeyCode::Char('l'), KeyCode::Enter] {
            app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
            rendered(&mut app, &config, 80, 12);
            assert_eq!(app.list_state.offset(), offset);
            assert_eq!(app.list_state.selected(), selected_row);
            assert!(
                app.mouse_targets
                    .iter()
                    .all(|(_, target)| matches!(target, MouseTarget::PickerRow(_)))
            );
            app.handle_key(key(close), &mut store, &config);
            assert!(matches!(app.screen, Screen::Navigation));
            assert_eq!(app.view, View::Planned);
            assert_eq!(app.filter, "keep");
            assert_eq!(app.selected_task().unwrap().id, task_id);
            assert_eq!(app.list_state.offset(), offset);
        }
        app.handle_key(key(KeyCode::Char('a')), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, task_id);
        assert_eq!(app.list_state.offset(), offset);
    }

    #[test]
    fn every_parent_route_restores_planned_bookmark_across_child_switches() {
        let (_directory, mut store, _) = fixture();
        let config = Config::from_toml("[keys]\nall_tasks = ['ctrl+a']").unwrap();
        let work = store.create_list("Work").unwrap();
        for index in 0..30 {
            store
                .create_task(&input(1, &format!("Keep task {index}")))
                .unwrap();
        }
        store.create_task(&input(work.id, "Child task")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "keep".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.selection = 20;
        rendered(&mut app, &config, 80, 12);
        let task_id = app.selected_task().unwrap().id;
        let offset = app.list_state.offset();
        for route in 0..3 {
            app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
            app.select_picker_identity(RowIdentity::List(work.id));
            app.handle_key(key(KeyCode::Enter), &mut store, &config);
            assert_eq!(app.view, View::List(work.id));
            assert!(app.filter.is_empty());
            assert_eq!(app.selection, 0);
            assert!(rendered(&mut app, &config, 80, 12).contains("All tasks > Work"));
            app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
            app.select_picker_identity(RowIdentity::Trash);
            app.handle_key(key(KeyCode::Enter), &mut store, &config);
            assert_eq!(app.view, View::Trash);
            if route == 0 {
                app.handle_key(key(KeyCode::Esc), &mut store, &config);
            } else {
                app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
                if route == 1 {
                    app.handle_key(
                        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
                        &mut store,
                        &config,
                    );
                } else {
                    app.select_picker_identity(RowIdentity::AllTasks);
                    app.handle_key(key(KeyCode::Enter), &mut store, &config);
                }
            }
            assert_eq!(app.view, View::Planned);
            assert!(matches!(app.screen, Screen::Navigation));
            assert_eq!(app.filter, "keep");
            assert_eq!(app.visible.len(), 30);
            assert_eq!(app.selected_task().unwrap().id, task_id);
            rendered(&mut app, &config, 80, 12);
            assert_eq!(app.list_state.offset(), offset);
        }
    }

    #[test]
    fn planned_bookmark_restores_by_id_and_clamps_when_the_saved_task_leaves_results() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let first = store.create_task(&input(1, "Keep first")).unwrap();
        let second = store.create_task(&input(1, "Keep second")).unwrap();
        let third = store.create_task(&input(1, "Keep third")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "keep".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        external.swap_positions(&first, &second).unwrap();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, first.id);
        assert_eq!(app.selection, 1);
        app.selection = 2;
        app.set_view(View::List(work.id), &store).unwrap();
        external
            .update_task(third.id, third.version, &input(1, "No longer matching"))
            .unwrap();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.filter, "keep");
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.selection, 1);
        assert_eq!(app.selected_task().unwrap().id, first.id);
        assert!(app.external_update);
    }

    #[test]
    fn picker_forms_help_and_external_refresh_keep_selection_by_list_id() {
        let (directory, mut store, config) = fixture();
        for index in 0..25 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        let selected = store.lists().unwrap()[20].clone();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(selected.id));
        rendered(&mut app, &config, 80, 12);
        let offset = app.picker.state.offset();
        assert!(offset > 0);
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        rendered(&mut app, &config, 80, 12);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(app.picker.state.offset(), offset);
        for action in ['N', 'R'] {
            app.handle_key(key(KeyCode::Char(action)), &mut store, &config);
            app.paste("Cancel this edit");
            app.handle_key(key(KeyCode::Esc), &mut store, &config);
            assert!(matches!(app.screen, Screen::ListPicker));
            assert_eq!(
                app.picker_identity(app.picker.selection),
                Some(RowIdentity::List(selected.id))
            );
            assert_eq!(app.picker.state.offset(), offset);
        }
        app.handle_key(key(KeyCode::Char('N')), &mut store, &config);
        app.paste("New list");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let created = store
            .lists()
            .unwrap()
            .into_iter()
            .find(|list| list.name == "New list")
            .unwrap();
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(created.id))
        );
        external
            .rename_list(created.id, "Renamed elsewhere")
            .unwrap();
        external.move_list(created.id, false).unwrap();
        external.create_list("Added elsewhere").unwrap();
        app.refresh_changes(&store, &config, today(), true).unwrap();
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(created.id))
        );
        assert!(app.external_update);
        assert!(matches!(app.screen, Screen::ListPicker));
    }

    #[test]
    fn virtual_picker_rows_are_distinct_from_same_named_ordinary_lists() {
        let (_directory, mut store, config) = fixture();
        let ordinary = store.create_list("All tasks").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame, &config)).unwrap();
        let (rect, _) = app.mouse_targets[0];
        let first_row = (rect.x..rect.right())
            .map(|x| terminal.backend().buffer()[(x, rect.y)].symbol())
            .collect::<String>();
        assert!(first_row.contains("All tasks"));
        assert!(!first_row.contains('('));
        for identity in [RowIdentity::AllTasks, RowIdentity::Trash] {
            app.select_picker_identity(identity);
            app.handle_key(key(KeyCode::Char('R')), &mut store, &config);
            assert!(matches!(app.screen, Screen::ListPicker));
            for code in [KeyCode::Up, KeyCode::Down] {
                app.handle_key(KeyEvent::new(code, KeyModifiers::ALT), &mut store, &config);
                assert_eq!(app.picker_identity(app.picker.selection), Some(identity));
            }
        }
        assert_eq!(store.lists().unwrap().len(), 2);
        app.select_picker_identity(RowIdentity::List(ordinary.id));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.view, View::List(ordinary.id));
        assert!(rendered(&mut app, &config, 80, 24).contains("All tasks > All tasks"));
    }

    #[test]
    fn picker_mouse_stays_inside_scrolled_and_resized_popup() {
        let (_directory, mut store, config) = fixture();
        for index in 0..30 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        store.create_task(&input(1, "Background task")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        rendered(&mut app, &config, 80, 12);
        let area = app.picker.area.unwrap();
        let now = Instant::now();
        for event in [
            mouse(MouseEventKind::ScrollDown, 0, 0),
            mouse(MouseEventKind::Down(MouseButton::Left), area.x, area.y - 1),
        ] {
            app.handle_mouse(event, now, &mut store, &config).unwrap();
        }
        assert_eq!(app.picker.selection, 0);
        assert_eq!(app.selection, 0);
        for _ in 0..20 {
            app.handle_mouse(
                mouse(MouseEventKind::ScrollDown, area.x, area.y),
                now,
                &mut store,
                &config,
            )
            .unwrap();
        }
        rendered(&mut app, &config, 60, 18);
        assert!(app.picker.state.offset() > 0);
        let (rect, MouseTarget::PickerRow(index)) = app.mouse_targets[0] else {
            panic!()
        };
        let Some(RowIdentity::List(expected)) = app.picker_identity(index) else {
            panic!()
        };
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert!(matches!(app.screen, Screen::ListPicker));
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            now + Duration::from_millis(100),
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.view, View::List(expected));
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn picker_and_parent_navigation_use_configured_actions() {
        let (_directory, mut store, _) = fixture();
        let work = store.create_list("Work").unwrap();
        let config = Config::from_toml("[keys]\nlists = ['ctrl+l']\nback = ['ctrl+b']\nplanned = ['ctrl+p']\nopen = ['ctrl+o']").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        assert!(rendered(&mut app, &config, 100, 24).contains("ctrl+l"));
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        let control = |letter| KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL);
        app.handle_key(control('l'), &mut store, &config);
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(control('o'), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        assert!(rendered(&mut app, &config, 100, 24).contains("ctrl+b"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        app.handle_key(control('l'), &mut store, &config);
        app.handle_key(control('b'), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(app.view, View::List(work.id));
        app.handle_key(control('l'), &mut store, &config);
        app.handle_key(control('p'), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn history_restores_each_origin_and_root_bookmark_through_navigation() {
        let (_directory, mut store, _) = fixture();
        let config = Config::from_toml("[keys]\nall_tasks = ['ctrl+a']").unwrap();
        let work = store.create_list("Work").unwrap();
        let other = store.create_list("Other").unwrap();
        for list_id in [1, work.id] {
            for index in 0..25 {
                store
                    .create_task(&input(list_id, &format!("Keep {index}")))
                    .unwrap();
            }
            let task = store
                .create_task(&input(list_id, "Old finished task"))
                .unwrap();
            store
                .set_status_on(
                    task.id,
                    task.version,
                    Status::Done,
                    today().pred_opt().unwrap(),
                )
                .unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "keep l:Inbox".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.selection = 20;
        rendered(&mut app, &config, 90, 14);
        let root_id = app.selected_task().unwrap().id;
        let root_state = app.list_state;
        for list_view in [View::Planned, View::List(work.id)] {
            app.set_view(list_view, &store).unwrap();
            if matches!(list_view, View::List(_)) {
                app.filter = "keep".into();
                app.query = Query::parse(&app.filter).unwrap();
                app.refresh(&store).unwrap();
                app.selection = 18;
            }
            rendered(&mut app, &config, 90, 14);
            let origin_id = app.selected_task().unwrap().id;
            let origin_filter = app.filter.clone();
            let origin_state = app.list_state;
            for route in 0..3 {
                app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
                assert_eq!(app.view, View::History(list_view.scope().0));
                assert!(app.filter.is_empty());
                assert!(app.visible.iter().all(|task| task.status == Status::Done));
                app.filter = "s:done".into();
                app.query = Query::parse(&app.filter).unwrap();
                app.refresh(&store).unwrap();
                if route == 2 {
                    app.open_picker();
                    app.choose_picker(&store).unwrap();
                } else {
                    app.handle_key(
                        key(if route == 0 {
                            KeyCode::Esc
                        } else {
                            KeyCode::Char('h')
                        }),
                        &mut store,
                        &config,
                    );
                }
                assert_eq!(app.view, list_view);
                assert_eq!(app.filter, origin_filter);
                assert_eq!(app.selected_task().unwrap().id, origin_id);
                assert_eq!(app.list_state, origin_state);
            }
        }
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(other.id));
        app.choose_picker(&store).unwrap();
        assert_eq!(app.view, View::List(other.id));
        assert!(app.filter.is_empty());
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        assert_eq!(app.filter, "keep l:Inbox");
        assert_eq!(app.selected_task().unwrap().id, root_id);
        assert_eq!(app.list_state, root_state);
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(app.selected_task().unwrap().id, root_id);
        assert_eq!(app.list_state, root_state);
    }

    #[test]
    fn history_keeps_its_scope_through_editor_filter_copy_help_and_status_actions() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(work.id, "Finished task")).unwrap();
        store
            .set_status_on(
                task.id,
                task.version,
                Status::Done,
                today().pred_opt().unwrap(),
            )
            .unwrap();
        let second = store
            .create_task(&input(work.id, "Cancelled task"))
            .unwrap();
        store
            .set_status_on(second.id, second.version, Status::Cancelled, today())
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        assert_eq!(app.mode(), Mode::History);
        let before = store.tasks().unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.tasks().unwrap(), before);
        for open in [KeyCode::F(4), KeyCode::Char('?')] {
            app.handle_key(key(open), &mut store, &config);
            app.handle_key(key(KeyCode::Esc), &mut store, &config);
            assert!(matches!(app.screen, Screen::Navigation));
            assert_eq!(app.view, View::History(Some(work.id)));
        }
        app.handle_key(key(KeyCode::Char('/')), &mut store, &config);
        app.paste("s:done");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.visible.len(), 1);
        assert_eq!(app.selected_task().unwrap().id, task.id);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Updated description");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(app.view, View::History(Some(work.id)));
        assert_eq!(
            store.task(task.id).unwrap().description,
            "Updated description"
        );
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Discard this");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.view, View::History(Some(work.id)));
        app.handle_key(key(KeyCode::Char('r')), &mut store, &config);
        assert!(app.visible.is_empty());
        assert_eq!(store.task(task.id).unwrap().status, Status::Open);
        assert_eq!(store.task(task.id).unwrap().status_changed_on, None);
        assert!(rendered(&mut app, &config, 90, 24).contains("No completed or cancelled tasks"));
        app.filter.clear();
        app.query = Query::default();
        app.refresh(&store).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(store.task(second.id).unwrap().deleted);
        assert!(app.visible.is_empty());
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        assert_eq!(app.selected_task().unwrap().id, task.id);
        app.set_view(View::Trash, &store).unwrap();
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        assert_eq!(app.view, View::Trash);
    }

    #[test]
    fn finished_and_history_headings_match_status_dates_and_stay_inert() {
        let (directory, mut store, config) = fixture();
        store.create_task(&input(1, "Still open")).unwrap();
        let mut ids = Vec::new();
        for (title, status, date) in [
            ("Done now", Status::Done, today()),
            ("Cancelled now", Status::Cancelled, today()),
            ("Done yesterday", Status::Done, today().pred_opt().unwrap()),
            ("Unknown completion", Status::Done, today()),
        ] {
            let task = store.create_task(&input(1, title)).unwrap();
            store
                .set_status_on(task.id, task.version, status, date)
                .unwrap();
            ids.push(task.id);
        }
        rusqlite::Connection::open(directory.path().join("tasks.db"))
            .unwrap()
            .execute(
                "UPDATE tasks SET status_changed_on = NULL, status_changed_at = NULL WHERE id = ?1",
                [ids[3]],
            )
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let screen = rendered(&mut app, &config, 90, 24);
        assert!(screen.find("Still open").unwrap() < screen.find("Done today").unwrap());
        assert!(screen.find("Done today").unwrap() < screen.find("Done now").unwrap());
        assert!(screen.find("Cancelled today").unwrap() < screen.find("Cancelled now").unwrap());
        assert!(!screen.contains("Done yesterday"));
        app.open_history(&store).unwrap();
        assert_eq!(
            app.visible.iter().map(|task| task.id).collect::<Vec<_>>(),
            [ids[1], ids[0], ids[2], ids[3]]
        );
        let mut terminal = Terminal::new(TestBackend::new(90, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame, &config)).unwrap();
        let screen = rendered(&mut app, &config, 90, 24);
        for heading in ["All tasks > History", "Today", "Yesterday", "Unknown date"] {
            assert!(screen.contains(heading), "Missing {heading}");
        }
        let first_row = app
            .mouse_targets
            .iter()
            .find_map(|(rect, target)| matches!(target, MouseTarget::Row(0)).then_some(*rect))
            .unwrap();
        let heading_y = first_row.y - 1;
        assert_eq!(
            terminal.backend().buffer()[(first_row.x + 2, heading_y)].fg,
            Color::Yellow
        );
        assert_eq!(
            terminal.backend().buffer()[(first_row.x + 2, first_row.y)].fg,
            Color::Cyan
        );
        assert!(
            !app.mouse_targets
                .iter()
                .any(|(rect, _)| rect.y == heading_y)
        );
        app.selection = 2;
        app.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                first_row.x,
                heading_y,
            ),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.selection, 2);
    }

    #[test]
    fn finishing_tasks_selects_next_open_then_previous_and_preserves_due_dates() {
        let (_directory, mut store, config) = fixture();
        let first = store.create_task(&input(1, "First")).unwrap();
        let second = store.create_task(&input(1, "Second")).unwrap();
        let third = store.create_task(&input(1, "Third")).unwrap();
        let mut app = App::new(&store, Local::now().date_naive()).unwrap();
        app.selection = 1;
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, third.id);
        assert_eq!(store.task(second.id).unwrap().due, None);
        app.handle_key(key(KeyCode::Char('c')), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, first.id);
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, first.id);
        let closed = store.tasks().unwrap();
        app.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(store.tasks().unwrap(), closed);
        app.filter = "s:open".into();
        app.query = Query::parse(&app.filter).unwrap();
        let task = store.task(first.id).unwrap();
        store
            .set_status(task.id, task.version, Status::Open)
            .unwrap();
        app.refresh(&store).unwrap();
        app.handle_key(key(KeyCode::Char('c')), &mut store, &config);
        assert!(app.visible.is_empty());
        assert!(app.selected_task().is_none());
    }

    #[test]
    fn list_deletion_defaults_to_no_protects_virtual_rows_and_keeps_all_task_kinds() {
        let (_directory, mut store, config) = fixture();
        store.rename_list(1, "Personal").unwrap();
        let work = store.create_list("Work").unwrap();
        for (status, deleted) in [
            (Status::Open, false),
            (Status::Done, false),
            (Status::Cancelled, false),
            (Status::Done, true),
        ] {
            let task = store.create_task(&input(work.id, status.as_str())).unwrap();
            let task = store
                .set_status_on(task.id, task.version, status, today())
                .unwrap();
            if deleted {
                store.set_deleted(task.id, task.version, true).unwrap();
            }
        }
        let original = store.tasks().unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        assert!(rendered(&mut app, &config, 100, 24).contains("Personal (0) [default]"));
        for identity in [
            RowIdentity::AllTasks,
            RowIdentity::Trash,
            RowIdentity::List(1),
        ] {
            app.select_picker_identity(identity);
            app.handle_key(key(KeyCode::Delete), &mut store, &config);
            assert!(matches!(app.screen, Screen::ListPicker));
        }
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        let display = rendered(&mut app, &config, 90, 24);
        for text in [
            "Delete this list?",
            "Move all tasks to Personal",
            "Unfinished: 1",
            "Done: 1",
            "Cancelled: 1",
            "In Trash: 1",
        ] {
            assert!(display.contains(text), "Missing {text}");
        }
        assert!(app.mouse_targets.is_empty());
        assert!(matches!(
            app.screen,
            Screen::DeleteListConfirm { delete: false, .. }
        ));
        app.paste("Yes");
        for event in [
            key(KeyCode::Char('y')),
            key(KeyCode::Char(' ')),
            key(KeyCode::Char('d')),
            key(KeyCode::F(4)),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        ] {
            app.handle_key(event, &mut store, &config);
        }
        assert!(matches!(
            app.screen,
            Screen::DeleteListConfirm { delete: false, .. }
        ));
        assert_eq!(store.tasks().unwrap(), original);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(store.lists().unwrap().len(), 2);
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(1))
        );
        assert_eq!(store.lists().unwrap().len(), 1);
        for mut task in original {
            task.list_id = 1;
            task.version += 1;
            assert_eq!(store.task(task.id).unwrap(), task);
        }
    }

    #[test]
    fn deletion_refresh_and_write_races_require_new_yes_and_handle_missing_lists() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Delete), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        external
            .create_task(&input(work.id, "Added after prompt"))
            .unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::DeleteListConfirm { preview, delete: false, changed: true } if preview.open_count == 1)
        );
        assert_eq!(store.lists().unwrap().len(), 2);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        external.rename_list(work.id, "Renamed elsewhere").unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(
            matches!(&app.screen, Screen::DeleteListConfirm { preview, delete: false, changed: true } if preview.list_name == "Renamed elsewhere")
        );
        assert!(rendered(&mut app, &config, 90, 24).contains("The list changed."));
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        external.rename_list(1, "New destination name").unwrap();
        app.refresh_changes(&store, &config, today(), true).unwrap();
        assert!(
            matches!(&app.screen, Screen::DeleteListConfirm { preview, delete: false, .. } if preview.destination_name == "New destination name")
        );
        let preview = external.preview_list_deletion(work.id).unwrap();
        external.delete_list(&preview).unwrap();
        app.refresh_changes(&store, &config, today(), true).unwrap();
        assert!(matches!(app.screen, Screen::ListPicker));
        assert!(
            app.message
                .as_ref()
                .unwrap()
                .text
                .contains("deleted elsewhere")
        );
        assert!(app.external_update);
        assert!(rendered(&mut app, &config, 90, 24).contains("deleted elsewhere"));
        assert_eq!(store.tasks().unwrap()[0].list_id, 1);
    }

    #[test]
    fn deleting_current_scope_restores_root_and_unrelated_scope_stays_open() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let other = store.create_list("Other").unwrap();
        let root_task = store.create_task(&input(1, "Keep parent")).unwrap();
        store.create_task(&input(work.id, "Child task")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "keep".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(other.id));
        app.handle_key(key(KeyCode::Delete), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Delete), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        assert_eq!(app.filter, "keep");
        assert_eq!(app.selected_task().unwrap().id, root_task.id);
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(1))
        );
    }

    #[test]
    fn external_list_deletion_preserves_drafts_and_does_not_restore_a_stale_history_origin() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let root_task = store.create_task(&input(1, "Parent")).unwrap();
        let child = store.create_task(&input(work.id, "Child")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        app.open_history(&store).unwrap();
        let preview = external.preview_list_deletion(work.id).unwrap();
        external.delete_list(&preview).unwrap();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        assert_eq!(app.selected_task().unwrap().id, root_task.id);
        assert_ne!(app.selected_task().unwrap().id, child.id);

        let another = store.create_list("Another").unwrap();
        let task = store.create_task(&input(another.id, "Edit me")).unwrap();
        app.refresh(&store).unwrap();
        app.set_view(View::List(another.id), &store).unwrap();
        app.open_selection(&store).unwrap();
        app.paste("Unsaved description");
        let preview = external.preview_list_deletion(another.id).unwrap();
        external.delete_list(&preview).unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert_eq!(app.view, View::Planned);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == "Unsaved description" && editor.original.as_ref().unwrap().version == task.version)
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(
            app.message
                .as_ref()
                .unwrap()
                .text
                .contains("changed in another process")
        );
        assert!(store.task(task.id).unwrap().description.is_empty());
    }

    #[test]
    fn new_actions_use_configured_shortcuts_and_localized_hints() {
        let (_directory, mut store, _) = fixture();
        let config = Config::from_toml("language = 'da'\n[keys]\nhistory = ['ctrl+h']\nreopen = ['ctrl+r']\ndelete_list = ['ctrl+d']\nback = ['ctrl+b']").unwrap();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(work.id, "Done")).unwrap();
        store
            .set_status_on(task.id, task.version, Status::Done, today())
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        assert!(rendered(&mut app, &config, 100, 24).contains("ctrl+h Historik"));
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let display = rendered(&mut app, &config, 100, 24);
        assert!(display.contains("Alle opgaver > Historik"));
        assert!(display.contains("ctrl+r"));
        app.open_picker();
        let display = rendered(&mut app, &config, 100, 24);
        assert!(display.contains("[standard]"));
        assert!(display.contains("ctrl+d"));
        assert!(display.contains(tr(Language::Da, "rename_list")));
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Delete), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(rendered(&mut app, &config, 100, 24).contains("Flyt alle opgaver til Inbox"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::DeleteListConfirm { .. }));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(app.view, View::History(None));
        assert_eq!(store.task(task.id).unwrap().list_id, work.id);
    }

    #[test]
    fn language_save_preserves_the_entire_editor_and_writes_the_selected_path() {
        let (directory, mut store, _) = fixture();
        let path = directory.path().join("shared.toml");
        std::fs::write(
            &path,
            "# Keep this comment\nlanguage = 'en'\n[keys]\nquit = ['ctrl+q']\n",
        )
        .unwrap();
        let mut config = Config::load(&path, true).unwrap();
        let task = store.create_task(&input(1, "Keep task text")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.paste("Unsaved æble\nsecond line");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        let draft = editor.clone();
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        assert!(matches!(
            app.screen,
            Screen::Language {
                selected: Language::En,
                ..
            }
        ));
        assert!(rendered(&mut app, &config, 90, 24).contains("Choose a language"));
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(config.language, Language::En);
        assert!(matches!(app.screen, Screen::Language { .. }));
        app.apply_language_change(&mut config, &path).unwrap();
        assert_eq!(config.language, Language::Da);
        assert_eq!(Config::load(&path, true).unwrap().language, Language::Da);
        assert_eq!(config.keys_for(Action::Quit), "ctrl+q");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# Keep this comment"));
        assert!(saved.contains("quit = ['ctrl+q']"));
        assert!(!directory.path().join("config.toml").exists());
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields, draft.fields);
        assert_eq!(editor.active, draft.active);
        assert_eq!(editor.initial_text, draft.initial_text);
        assert_eq!(editor.original, draft.original);
        assert_eq!(store.task(task.id).unwrap(), task);
        let screen = rendered(&mut app, &config, 90, 24);
        assert!(screen.contains("Beskrivelse"));
        assert!(screen.contains("Sproget er gemt."));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(&app.screen, Screen::Editor(editor) if !editor.is_dirty()));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn language_cancel_preserves_picker_scroll_and_blocks_background_actions() {
        let (directory, mut store, mut config) = fixture();
        for index in 0..25 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        app.picker.selection = 20;
        app.clamp_picker();
        rendered(&mut app, &config, 90, 14);
        let selection = app.picker.selection;
        let scroll = app.picker.state;
        let original_lists = store.lists().unwrap();
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.paste("Rename this");
        for code in [
            KeyCode::Char('N'),
            KeyCode::Char('R'),
            KeyCode::Delete,
            KeyCode::Char('l'),
        ] {
            app.handle_key(key(code), &mut store, &config);
        }
        rendered(&mut app, &config, 90, 14);
        assert!(app.mouse_targets.is_empty());
        app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, 20, 8),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), 20, 8),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert!(matches!(
            app.screen,
            Screen::Language {
                selected: Language::Da,
                ..
            }
        ));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.apply_language_change(&mut config, &directory.path().join("config.toml"))
            .unwrap();
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(app.picker.selection, selection);
        assert_eq!(app.picker.state, scroll);
        assert_eq!(config.language, Language::En);
        assert_eq!(store.lists().unwrap(), original_lists);
        assert!(!directory.path().join("config.toml").exists());
    }

    #[test]
    fn failed_language_save_keeps_the_previous_language_and_draft_until_cancelled() {
        let (directory, mut store, mut config) = fixture();
        let blocked_parent = directory.path().join("not-a-directory");
        std::fs::write(&blocked_parent, "Preserve this file").unwrap();
        let path = blocked_parent.join("config.toml");
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.paste("Keep this draft");
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        let draft = editor.clone();
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let error = app.apply_language_change(&mut config, &path).unwrap_err();
        app.set_error(error, &config);
        assert_eq!(config.language, Language::En);
        assert!(matches!(
            app.screen,
            Screen::Language {
                selected: Language::Da,
                ..
            }
        ));
        assert!(rendered(&mut app, &config, 120, 24).contains("Could not save language"));
        assert_eq!(
            std::fs::read_to_string(&blocked_parent).unwrap(),
            "Preserve this file"
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields, draft.fields);
        assert_eq!(editor.original, draft.original);
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn language_picker_uses_configured_controls_and_preserves_nested_screens() {
        let (directory, mut store, _) = fixture();
        let mut config = Config::from_toml("[keys]\nlanguage = ['f6']\nup = ['ctrl+k']\ndown = ['ctrl+j']\napply = ['ctrl+enter']\nback = ['ctrl+b']").unwrap();
        let path = directory.path().join("config.toml");
        let mut app = App::new(&store, today()).unwrap();
        let control = |code| KeyEvent::new(code, KeyModifiers::CONTROL);
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        for screen in [
            Screen::Filter(TextBuffer::new("l:Inbox")),
            Screen::ListName {
                id: None,
                buffer: TextBuffer::new("New list draft"),
            },
            Screen::Help {
                previous: Box::new(Screen::ListPicker),
            },
        ] {
            let expected = format!("{screen:?}");
            app.screen = screen;
            app.handle_key(key(KeyCode::F(6)), &mut store, &config);
            app.handle_key(key(KeyCode::Down), &mut store, &config);
            assert!(matches!(
                app.screen,
                Screen::Language {
                    selected: Language::En,
                    ..
                }
            ));
            app.handle_key(control(KeyCode::Char('j')), &mut store, &config);
            app.handle_key(control(KeyCode::Char('k')), &mut store, &config);
            app.handle_key(control(KeyCode::Char('j')), &mut store, &config);
            app.handle_key(key(KeyCode::Enter), &mut store, &config);
            assert!(app.pending_language.is_none());
            app.handle_key(control(KeyCode::Enter), &mut store, &config);
            app.apply_language_change(&mut config, &path).unwrap();
            assert_eq!(format!("{:?}", app.screen), expected);
            assert_eq!(config.language, Language::Da);
            app.handle_key(key(KeyCode::F(6)), &mut store, &config);
            assert!(matches!(
                app.screen,
                Screen::Language {
                    selected: Language::Da,
                    ..
                }
            ));
            app.handle_key(control(KeyCode::Char('b')), &mut store, &config);
            assert_eq!(format!("{:?}", app.screen), expected);
            config.language = Language::En;
        }
        assert_eq!(Config::load(&path, true).unwrap().language, Language::Da);
        assert!(store.tasks().unwrap().is_empty());
        assert_eq!(store.lists().unwrap().len(), 1);
    }

    #[test]
    fn language_changes_leave_clean_editors_clean_and_do_not_open_above_confirmations() {
        let (directory, mut store, mut config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let work = store.create_list("Work").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.apply_language_change(&mut config, &directory.path().join("config.toml"))
            .unwrap();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        app.open_selection(&store).unwrap();
        app.paste("Unsaved");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        assert!(matches!(app.screen, Screen::ConfirmDiscard { .. }));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::F(4)), &mut store, &config);
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        assert!(matches!(app.screen, Screen::CopyText { .. }));
        app.screen = Screen::Navigation;
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Delete), &mut store, &config);
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        assert!(matches!(app.screen, Screen::DeleteListConfirm { .. }));
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn due_presets_change_only_due_and_keep_selected_task_for_open_and_history_rows() {
        let (_directory, mut store, config) = fixture();
        let monday = parse_date("2026-11-09").unwrap();
        let work = store.create_list("Work").unwrap();
        let mut original_input = input(work.id, "Preserve this title #tag");
        original_input.description = "First line\nSecond line æble".into();
        let task = store.create_task(&original_input).unwrap();
        store.create_task(&input(1, "Other task")).unwrap();
        let mut app = App::new(&store, monday).unwrap();
        for status in [Status::Open, Status::Done, Status::Cancelled] {
            let current = store.task(task.id).unwrap();
            store
                .set_status_on(task.id, current.version, status, monday.pred_opt().unwrap())
                .unwrap();
            app.set_view(View::List(work.id), &store).unwrap();
            if status != Status::Open {
                app.open_history(&store).unwrap();
            }
            app.refresh(&store).unwrap();
            assert_eq!(app.selected_task().unwrap().id, task.id);
            for (key_code, due) in [
                (KeyCode::Char('t'), Some(monday)),
                (KeyCode::Char('T'), Some(parse_date("2026-11-10").unwrap())),
                (KeyCode::Char('w'), Some(parse_date("2026-11-16").unwrap())),
                (KeyCode::Char('x'), None),
            ] {
                let mut expected = store.task(task.id).unwrap();
                expected.due = due;
                expected.version += 1;
                app.handle_key(key(key_code), &mut store, &config);
                assert_eq!(store.task(task.id).unwrap(), expected);
                assert_eq!(app.selected_task().unwrap().id, task.id);
                app.handle_key(key(key_code), &mut store, &config);
                assert_eq!(store.task(task.id).unwrap(), expected);
            }
        }
    }

    #[test]
    fn due_preset_filter_departure_clamps_and_stale_noop_does_not_overwrite() {
        let (directory, mut store, config) = fixture();
        let first = store.create_task(&input(1, "First")).unwrap();
        let second = store.create_task(&input(1, "Second")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "due:none".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        assert_eq!(store.task(first.id).unwrap().due, Some(today()));
        assert_eq!(app.visible.len(), 1);
        assert_eq!(app.selected_task().unwrap().id, second.id);
        let mut external_input = input(1, "Changed elsewhere");
        external_input.due = Some(today());
        let updated = external
            .update_task(second.id, second.version, &external_input)
            .unwrap();
        app.handle_key(key(KeyCode::Char('x')), &mut store, &config);
        assert_eq!(store.task(second.id).unwrap(), updated);
        assert!(
            app.message
                .as_ref()
                .unwrap()
                .text
                .contains("changed in another process")
        );
        assert_eq!(app.selected_task().unwrap().version, second.version);
        app.refresh(&store).unwrap();
        assert!(app.selected_task().is_none());
        let before = store.tasks().unwrap();
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        assert_eq!(store.tasks().unwrap(), before);
        let removed = store
            .set_deleted(first.id, store.task(first.id).unwrap().version, true)
            .unwrap();
        app.set_view(View::Trash, &store).unwrap();
        for letter in ['t', 'T', 'w', 'x'] {
            app.handle_key(key(KeyCode::Char(letter)), &mut store, &config);
        }
        assert_eq!(store.task(first.id).unwrap(), removed);
    }

    #[test]
    fn due_field_presets_are_single_undo_edits_and_save_only_when_requested() {
        let (_directory, mut store, config) = fixture();
        let mut value = input(1, "Keep title");
        value.due = Some(parse_date("2026-11-12").unwrap());
        let task = store.create_task(&value).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        assert_eq!(app.mode(), Mode::DueField);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[2].insert("3");
        editor.fields[2].set_cursor(2, false);
        editor.fields[2].set_cursor(5, true);
        let old_text = editor.fields[2].text.clone();
        let old_cursor = editor.fields[2].cursor;
        let old_selection = editor.fields[2].selection();
        let title = editor.fields[0].clone();
        let description = editor.fields[1].clone();
        let list = editor.fields[3].clone();
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields[2].text, "2026-11-08");
        assert_eq!(editor.fields[2].cursor, 10);
        assert!(editor.fields[2].selection().is_none());
        assert_eq!(editor.fields[0], title);
        assert_eq!(editor.fields[1], description);
        assert_eq!(editor.fields[3], list);
        assert_eq!(editor.active, 2);
        assert_eq!(store.task(task.id).unwrap(), task);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[2].set_cursor(2, false);
        editor.fields[2].set_cursor(5, true);
        let unchanged = editor.fields[2].clone();
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[2] == unchanged));
        let copied = record_clipboard(&mut app, false);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(*copied.lock().unwrap(), vec!["26-".to_owned()]);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields[2].text, old_text);
        assert_eq!(editor.fields[2].cursor, old_cursor);
        assert_eq!(editor.fields[2].selection(), old_selection);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "2026-11-12")
        );
        for _ in 0..2 {
            app.handle_key(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
        }
        app.handle_key(key(KeyCode::Char('x')), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text.is_empty()));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "2026-11-08")
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.task(task.id).unwrap().due, Some(today()));
    }

    #[test]
    fn due_preset_letters_remain_text_elsewhere_and_hints_use_configured_localized_keys() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.new_task();
        for index in [0, 1] {
            let Screen::Editor(editor) = &mut app.screen else {
                panic!()
            };
            editor.active = index;
            let original = editor.fields[index].text.clone();
            for letter in ['t', 'T', 'w', 'x'] {
                app.handle_key(key(KeyCode::Char(letter)), &mut store, &config);
            }
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[index].text == format!("{original}tTwx"))
            );
        }
        for screen in [
            Screen::Filter(TextBuffer::default()),
            Screen::ListName {
                id: None,
                buffer: TextBuffer::default(),
            },
        ] {
            app.screen = screen;
            for letter in ['t', 'T', 'w', 'x'] {
                app.handle_key(key(KeyCode::Char(letter)), &mut store, &config);
            }
            assert!(
                matches!(&app.screen, Screen::Filter(buffer) | Screen::ListName { buffer, .. } if buffer.text == "tTwx")
            );
        }
        let config = Config::from_toml("language = 'da'\n[keys]\ndue_today = ['ctrl+t']\ndue_tomorrow = ['ctrl+u']\ndue_next_week = ['ctrl+w']\nclear_due = ['ctrl+x']").unwrap();
        app.new_task();
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 2;
        let display = rendered(&mut app, &config, 130, 24);
        for text in [
            "ctrl+t",
            "ctrl+u",
            "ctrl+w",
            "ctrl+x",
            "I dag",
            "I morgen",
            tr(Language::Da, "next_monday"),
            tr(Language::Da, "clear_due"),
        ] {
            assert!(display.contains(text), "Missing {text}");
        }
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "t"));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "2026-11-08")
        );
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        assert_eq!(app.mode(), Mode::ListField);
        assert!(!rendered(&mut app, &config, 130, 24).contains(tr(Language::Da, "next_monday")));
        assert!(store.tasks().unwrap().is_empty());
    }

    #[test]
    fn midnight_presets_use_the_new_day_without_mutating_a_replacement_selection() {
        let (_directory, mut store, config) = fixture();
        let open = store.create_task(&input(1, "Open task")).unwrap();
        let finished = store
            .create_task(&input(1, "Finished yesterday at midnight"))
            .unwrap();
        let finished = store
            .set_status_on(finished.id, finished.version, Status::Done, today())
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.select_task_id(Some(finished.id));
        let tomorrow = today().succ_opt().unwrap();
        app.handle_key_on_date(key(KeyCode::Char('t')), &mut store, &config, tomorrow);
        let mut expected = finished;
        expected.due = Some(tomorrow);
        expected.version += 1;
        assert_eq!(store.task(expected.id).unwrap(), expected);
        assert_eq!(store.task(open.id).unwrap(), open);
        assert_eq!(app.today, tomorrow);
        assert_eq!(app.selected_task().unwrap().id, open.id);
        app.open_selection(&store).unwrap();
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 2;
        editor.fields[1].insert("Keep unsaved text");
        let next_day = tomorrow.succ_opt().unwrap();
        app.handle_key_on_date(key(KeyCode::Char('t')), &mut store, &config, next_day);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == next_day.to_string() && editor.fields[1].text == "Keep unsaved text" && editor.original.as_ref().unwrap().version == open.version)
        );
        assert_eq!(store.task(open.id).unwrap(), open);
    }

    #[test]
    fn all_list_row_labels_follow_scope_renames_and_moves_without_changing_task_text() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let home = store.create_list("Home").unwrap();
        let mut value = input(work.id, "Review");
        value.description = "Original description".into();
        let work_task = store.create_task(&value).unwrap();
        let home_task = store.create_task(&input(home.id, "Review")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let screen = rendered(&mut app, &config, 90, 24);
        assert!(screen.contains("[ ] [Work] Review"));
        assert!(screen.contains("[ ] [Home] Review"));
        let task_rows: Vec<_> = app
            .mouse_targets
            .iter()
            .filter_map(|(rect, target)| matches!(target, MouseTarget::Row(_)).then_some(rect.y))
            .collect();
        assert_eq!(task_rows.len(), 2);
        assert_eq!(task_rows[1], task_rows[0] + 1);
        app.filter = "l:Work".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        assert!(rendered(&mut app, &config, 90, 24).contains("[Work] Review"));
        app.open_copy_text();
        assert!(
            matches!(&app.screen, Screen::CopyText { text, .. } if text == "Review\n\nOriginal description")
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.open_selection(&store).unwrap();
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[0].text == "Review"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(store.task(work_task.id).unwrap(), work_task);
        assert_eq!(store.task(home_task.id).unwrap(), home_task);
        app.filter.clear();
        app.query = Query::default();
        store.rename_list(work.id, "Office\r\nTeam").unwrap();
        app.refresh(&store).unwrap();
        let screen = rendered(&mut app, &config, 90, 24);
        assert!(screen.contains("[Office  Team] Review"));
        assert_eq!(
            app.mouse_targets
                .iter()
                .filter(|(_, target)| matches!(target, MouseTarget::Row(_)))
                .count(),
            2
        );
        value.list_id = home.id;
        let moved = store
            .update_task(work_task.id, work_task.version, &value)
            .unwrap();
        app.refresh(&store).unwrap();
        let screen = rendered(&mut app, &config, 90, 24);
        assert!(!screen.contains("[Office  Team]"));
        assert_eq!(screen.matches("[Home] Review").count(), 2);
        store
            .set_status_on(moved.id, moved.version, Status::Done, today())
            .unwrap();
        store
            .set_status_on(home_task.id, home_task.version, Status::Cancelled, today())
            .unwrap();
        app.refresh(&store).unwrap();
        app.open_history(&store).unwrap();
        app.filter = "l:Home".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(
            rendered(&mut app, &config, 90, 24)
                .matches("[Home] Review")
                .count(),
            2
        );
        app.set_view(View::List(home.id), &store).unwrap();
        assert!(!rendered(&mut app, &config, 90, 24).contains("[Home]"));
        app.open_history(&store).unwrap();
        assert!(!rendered(&mut app, &config, 90, 24).contains("[Home]"));
        let moved = store.task(moved.id).unwrap();
        store.set_deleted(moved.id, moved.version, true).unwrap();
        app.set_view(View::Trash, &store).unwrap();
        let screen = rendered(&mut app, &config, 90, 24);
        assert!(screen.contains("Review"));
        assert!(!screen.contains("[Home]"));
    }

    #[test]
    fn compact_dates_normalize_only_on_field_exit_and_keep_one_undo_step() {
        let (_directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        let display = rendered(&mut app, &config, 80, 24);
        for format in ["YYYY-MM-DD", "YYYYMMDD", "YYMMDD", "MMDD"] {
            assert!(display.contains(format));
        }
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text.is_empty() && !editor.is_dirty())
        );
        let copied = record_clipboard(&mut app, false);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(copied.lock().unwrap().is_empty());
        for (raw, exit) in [("20261109", 0), (" 261109 ", 1), ("1109", 2)] {
            let Screen::Editor(editor) = &mut app.screen else {
                panic!()
            };
            editor.active = 2;
            editor.fields[2] = TextBuffer::default();
            for character in raw.chars() {
                app.handle_key(key(KeyCode::Char(character)), &mut store, &config);
            }
            assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == raw));
            if exit == 2 {
                rendered(&mut app, &config, 80, 24);
                let (rect, _) = app
                    .mouse_targets
                    .iter()
                    .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 0, .. }))
                    .copied()
                    .unwrap();
                app.handle_mouse(
                    mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
                    Instant::now(),
                    &mut store,
                    &config,
                )
                .unwrap();
            } else {
                app.handle_key(
                    if exit == 0 {
                        key(KeyCode::Tab)
                    } else {
                        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
                    },
                    &mut store,
                    &config,
                );
            }
            let Screen::Editor(editor) = &mut app.screen else {
                panic!()
            };
            assert_eq!(editor.fields[2].text, "2026-11-09");
            editor.active = 2;
            app.handle_key(
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
            assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == raw));
            app.handle_key(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
            let Screen::Editor(editor) = &app.screen else {
                panic!()
            };
            let normalized = editor.fields[2].clone();
            app.handle_key(key(KeyCode::Tab), &mut store, &config);
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[2] == normalized)
            );
            assert_eq!(store.task(task.id).unwrap(), task);
        }
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 2;
        editor.fields[2].replace_all("2026-");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "2026-" && editor.active == 3)
        );
        assert!(app.message.as_ref().unwrap().is_error);
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn compact_dates_use_the_new_year_for_keyboard_mouse_and_save() {
        let (_directory, mut store, config) = fixture();
        let december = parse_date("2026-12-31").unwrap();
        let january = parse_date("2027-01-01").unwrap();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, december).unwrap();
        app.open_selection(&store).unwrap();
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 2;
        editor.fields[2].insert("0102");
        app.handle_key_on_date(key(KeyCode::Tab), &mut store, &config, january);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "2027-01-02")
        );
        app.today = december;
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 2;
        editor.fields[2].replace_all("0103");
        rendered(&mut app, &config, 80, 24);
        let (rect, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 0, .. }))
            .copied()
            .unwrap();
        app.handle_mouse_on_date(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            Instant::now(),
            &mut store,
            &config,
            january,
        )
        .unwrap();
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[2].text == "2027-01-03")
        );
        app.today = december;
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 2;
        editor.fields[2].replace_all("0104");
        app.handle_key_on_date(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
            january,
        );
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(
            store.task(task.id).unwrap().due,
            Some(parse_date("2027-01-04").unwrap())
        );
    }

    #[test]
    fn editor_list_picker_changes_only_the_draft_and_supports_cancel_undo_and_save() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        store.create_list("All tasks").unwrap();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.paste("Unsaved description");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        assert_eq!(app.mode(), Mode::ListField);
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        let before = editor.clone();
        let copied = record_clipboard(&mut app, false);
        app.paste("Must not type a list");
        for code in [
            KeyCode::Char('x'),
            KeyCode::Delete,
            KeyCode::Backspace,
            KeyCode::Home,
        ] {
            app.handle_key(key(code), &mut store, &config);
        }
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(copied.lock().unwrap().is_empty());
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields == before.fields));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        rendered(&mut app, &config, 80, 24);
        assert_eq!(app.mouse_targets.len(), 3);
        assert!(
            app.mouse_targets
                .iter()
                .all(|(_, target)| matches!(target, MouseTarget::EditorListRow(_)))
        );
        app.paste("Ignore paste");
        for event in [
            key(KeyCode::Char('N')),
            key(KeyCode::Char('R')),
            key(KeyCode::Delete),
            key(KeyCode::F(4)),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        ] {
            app.handle_key(event, &mut store, &config);
        }
        assert!(matches!(app.screen, Screen::EditorListPicker { .. }));
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields == before.fields && editor.active == 3)
        );
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.selected_list_id(), Some(work.id));
        assert_eq!(editor.fields[..3], before.fields[..3]);
        assert_eq!(editor.original, before.original);
        assert_eq!(editor.active, 3);
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(1))
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let saved = store.task(task.id).unwrap();
        assert_eq!(saved.list_id, work.id);
        assert_eq!(saved.description, "Unsaved description");
        assert_eq!(saved.title, task.title);
        assert_eq!(saved.version, task.version + 1);
    }

    #[test]
    fn editor_list_identity_survives_rename_but_never_retargets_a_deleted_choice() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = 3;
        let clean = editor.fields.clone();
        external.rename_list(1, "Personal").unwrap();
        app.refresh(&store).unwrap();
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields == clean && !editor.is_dirty())
        );
        assert!(rendered(&mut app, &config, 80, 24).contains("Personal"));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields == clean && !editor.is_dirty())
        );
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let preview = external.preview_list_deletion(work.id).unwrap();
        external.delete_list(&preview).unwrap();
        let replacement = external.create_list("Work").unwrap();
        app.refresh(&store).unwrap();
        assert!(
            rendered(&mut app, &config, 80, 24).contains(tr(config.language, "list_unavailable"))
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(work.id))
        );
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(replacement.id))
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(work.id))
        );
        assert!(
            rendered(&mut app, &config, 80, 24).contains(tr(config.language, "list_unavailable"))
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let preview = external.preview_list_deletion(replacement.id).unwrap();
        external.delete_list(&preview).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::EditorListPicker { .. }));
        assert!(app.message.as_ref().unwrap().is_error);
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(replacement.id))
        );
    }

    #[test]
    fn editor_list_picker_preserves_scrolled_state_through_help_language_and_refresh() {
        let (directory, mut store, mut config) = fixture();
        for index in 0..25 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.picker.selection = 7;
        app.clamp_picker();
        let navigation_picker = app.picker.state;
        app.open_selection(&store).unwrap();
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = 3;
        }
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        for _ in 0..22 {
            app.handle_key(key(KeyCode::Down), &mut store, &config);
        }
        rendered(&mut app, &config, 80, 18);
        let picker = app.screen.editor_picker().unwrap();
        let selected = app.lists[picker.selection].id;
        let offset = picker.state.offset();
        assert!(offset > 0);
        assert_eq!(app.picker.state, navigation_picker);
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        external.rename_list(selected, "Renamed selection").unwrap();
        external.move_list(selected, false).unwrap();
        app.refresh_changes(&store, &config, today(), true).unwrap();
        assert_eq!(
            app.lists[app.screen.editor_picker().unwrap().selection].id,
            selected
        );
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.apply_language_change(&mut config, &directory.path().join("config.toml"))
            .unwrap();
        assert!(matches!(app.screen, Screen::Help { .. }));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::EditorListPicker { .. }));
        assert_eq!(
            app.lists[app.screen.editor_picker().unwrap().selection].id,
            selected
        );
        assert_eq!(app.screen.editor_picker().unwrap().state.offset(), offset);
        let display = rendered(&mut app, &config, 80, 18);
        assert!(display.contains("Renamed selection"));
        assert!(display.contains("Vælg en liste"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(1) && !editor.is_dirty())
        );
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn editor_list_mouse_opens_and_chooses_only_visible_popup_rows() {
        let (_directory, mut store, config) = fixture();
        for index in 0..25 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = 2;
            editor.fields[2].insert("1109");
        }
        rendered(&mut app, &config, 80, 24);
        let (field, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: 3, .. }))
            .copied()
            .unwrap();
        let now = Instant::now();
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), field.x, field.y),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert!(
            matches!(&app.screen, Screen::EditorListPicker { editor, .. } if editor.active == 3 && editor.fields[2].text == "2026-11-09")
        );
        rendered(&mut app, &config, 80, 18);
        let inner = app.screen.editor_picker().unwrap().area.unwrap();
        rendered(&mut app, &config, 20, 8);
        assert!(app.screen.editor_picker().unwrap().area.is_none());
        app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, inner.x, inner.y),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.screen.editor_picker().unwrap().selection, 0);
        rendered(&mut app, &config, 80, 18);
        app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, 0, 0),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.screen.editor_picker().unwrap().selection, 0);
        for _ in 0..20 {
            app.handle_mouse(
                mouse(MouseEventKind::ScrollDown, inner.x, inner.y),
                now,
                &mut store,
                &config,
            )
            .unwrap();
        }
        rendered(&mut app, &config, 60, 24);
        let (rect, MouseTarget::EditorListRow(index)) = app.mouse_targets[0] else {
            panic!()
        };
        let expected = app.lists[index].id;
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert!(matches!(app.screen, Screen::EditorListPicker { .. }));
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            now + Duration::from_millis(100),
            &mut store,
            &config,
        )
        .unwrap();
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(expected) && editor.active == 3)
        );
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn editor_list_picker_uses_configured_actions_and_stale_task_saves_still_fail() {
        let (directory, mut store, _) = fixture();
        let config =
            Config::from_toml("[keys]\nopen = ['ctrl+o']\nlists = ['ctrl+l']\nback = ['ctrl+b']")
                .unwrap();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(work.id, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = 3;
        }
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::EditorListPicker { .. }));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        let display = rendered(&mut app, &config, 100, 24);
        assert!(display.contains("ctrl+o / ctrl+l"));
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let preview = external.preview_list_deletion(work.id).unwrap();
        external.delete_list(&preview).unwrap();
        app.refresh(&store).unwrap();
        app.handle_key(
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(1) && editor.original.as_ref().unwrap().version == task.version)
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(
            app.message
                .as_ref()
                .unwrap()
                .text
                .contains("changed in another process")
        );
        assert_eq!(store.task(task.id).unwrap().version, task.version + 1);
    }

    #[test]
    fn dated_navigation_headings_split_future_days_and_keep_scroll_hits_aligned() {
        let (_directory, mut store, config) = fixture();
        for (index, days) in [0, 1, 2, 2, 3, 30].into_iter().enumerate() {
            let mut value = input(1, &format!("Scheduled {index}"));
            value.due = Some(today() + chrono::Duration::days(days));
            store.create_task(&value).unwrap();
        }
        let done = store.create_task(&input(1, "Finished")).unwrap();
        store
            .set_status_on(done.id, done.version, Status::Done, today())
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| app.draw(frame, &config)).unwrap();
        let buffer = terminal.backend().buffer();
        let headings: Vec<_> = (0..30)
            .filter(|row| buffer[(2, *row)].fg == Color::Yellow)
            .map(|row| {
                (0..100)
                    .map(|column| buffer[(column, row)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect();
        let mut expected = vec![
            format!("Today - {}", date_label(today(), Language::En)),
            format!(
                "Tomorrow - {}",
                date_label(today().succ_opt().unwrap(), Language::En)
            ),
        ];
        for days in [2, 3, 30] {
            expected.push(date_label(
                today() + chrono::Duration::days(days),
                Language::En,
            ));
        }
        expected.push(format!(
            "Done today - {}",
            date_label(today(), Language::En)
        ));
        assert_eq!(headings, expected);
        app.selection = app.visible.len() - 1;
        rendered(&mut app, &config, 100, 10);
        assert!(app.list_state.offset() > 0);
        let (rect, MouseTarget::Row(index)) = app.mouse_targets[0] else {
            panic!()
        };
        let expected_id = app.visible[index].id;
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.selected_task().unwrap().id, expected_id);
    }

    #[test]
    fn editor_discard_confirmation_remains_above_the_selected_list() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(work.id, "Child task")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Unsaved description");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[1].text == "Unsaved description")
        );
        assert_eq!(app.view, View::List(work.id));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(app.view, View::List(work.id));
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
    }
}
