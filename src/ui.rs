use std::io::{self, IsTerminal, Stdout};
use std::panic::PanicHookInfo;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate, Utc};
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
use crate::i18n::{Language, error_message, sync_failure_message, tr};
use crate::model::{Status, Task, TaskInput, TaskList};
use crate::planning::{
    Bucket, DuePreset, PlannedGroup, date_label, history_group_label, open_group_label,
    ordinary_group_label, planned_group, suggested_list_id,
};
use crate::query::{Query, visible_history, visible_tasks};
use crate::store::{
    EmptyTrashOutcome, ListDeletionOutcome, ListDeletionPreview, Store, SyncCounters,
    TaskPurgeOutcome, TrashPreview,
};
use crate::sync::worker::SyncWorker;
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

/// How long the exit sync may take before Husker leaves the changes for the next sync.
const EXIT_SYNC_TIMEOUT: Duration = Duration::from_secs(10);

/// Runs the terminal interface. `sync` is the background sync worker of a
/// profile with sync settings.
pub fn run(
    store: &mut Store,
    config: &Config,
    config_path: &Path,
    sync: Option<SyncWorker>,
) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(tr(config.language, "terminal_required"));
    }
    let mut app = App::new(store, Local::now().date_naive())?;
    let mut config = config.clone();
    let guard = TerminalGuard::enter()?;
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut refreshed = Instant::now();
    let mut mouse_captured = true;
    loop {
        app.expire_message(Instant::now());
        terminal.draw(|frame| app.draw(frame, &config))?;
        if event::poll(Duration::from_millis(200))? {
            let event = event::read()?;
            if let Some(sync) = &sync
                && matches!(event, Event::Key(_) | Event::Paste(_) | Event::Mouse(_))
            {
                sync.input();
            }
            match event {
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
        app.report_sync(sync.as_ref(), &config, Instant::now());
    }
    let mut unsent = None;
    if let Some(sync) = sync
        && store.sync_pending().unwrap_or(false)
    {
        app.show_message(tr(config.language, "sync_exit"), false, Instant::now());
        terminal.draw(|frame| app.draw(frame, &config))?;
        unsent = sync.finish(EXIT_SYNC_TIMEOUT);
    }
    drop(terminal);
    drop(guard);
    if let Some(failure) = unsent {
        eprintln!("{}", sync_failure_message(config.language, failure.kind));
    }
    Ok(())
}

/// What the last sync commit did, for the message the interface shows once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyncNotice {
    Received,
    Conflict,
}

// Task editor buffer indexes. `FIELD_ORDER` decides where each field is drawn and Tab order.
const TITLE: usize = 0;
const DESCRIPTION: usize = 1;
const DUE: usize = 2;
const LIST: usize = 3;
const FIELD_KEYS: [&str; 4] = ["title", "description", "due", "list"];
const FIELD_ORDER: [usize; 4] = [TITLE, LIST, DUE, DESCRIPTION];

fn due_preset(action: Action) -> Option<DuePreset> {
    match action {
        Action::DueToday => Some(DuePreset::Today),
        Action::DueTomorrow => Some(DuePreset::Tomorrow),
        Action::DueNextWeek => Some(DuePreset::NextMonday),
        Action::ClearDue => Some(DuePreset::Clear),
        _ => None,
    }
}

fn navigation_selection(current: usize, count: usize, action: Action) -> usize {
    if count == 0 {
        return 0;
    }
    let last = count - 1;
    let current = current.min(last);
    match action {
        Action::First => 0,
        Action::Last => last,
        Action::Up => {
            if current == 0 {
                last
            } else {
                current - 1
            }
        }
        Action::Down => {
            if current == last {
                0
            } else {
                current + 1
            }
        }
        _ => current,
    }
}

/// Moves a selection by one displayed page without wrapping.
fn page_selection(current: usize, count: usize, page: usize, down: bool) -> usize {
    if count == 0 {
        return 0;
    }
    let step = page.max(1);
    if down {
        current.saturating_add(step).min(count - 1)
    } else {
        current.min(count - 1).saturating_sub(step)
    }
}

/// `y` or `n` in a confirmation, with Shift allowed for the uppercase letters.
fn confirmation_letter(key: KeyEvent) -> Option<bool> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Char('y' | 'Y') => Some(true),
        KeyCode::Char('n' | 'N') => Some(false),
        _ => None,
    }
}

/// What a key does in a No/Yes confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmKey {
    Cancel,
    Answer(bool),
    Toggle,
    Accept,
    Ignore,
}

/// Maps a key in a No/Yes confirmation. `cancel` is the configured action that leaves it.
fn confirm_key(key: KeyEvent, action: Option<Action>, cancel: Action) -> ConfirmKey {
    if action == Some(cancel) {
        return ConfirmKey::Cancel;
    }
    if let Some(choice) = confirmation_letter(key) {
        return ConfirmKey::Answer(choice);
    }
    let tab = matches!(key.code, KeyCode::Tab | KeyCode::BackTab);
    if key.modifiers.is_empty() {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => ConfirmKey::Toggle,
            _ if tab => ConfirmKey::Toggle,
            KeyCode::Enter => ConfirmKey::Accept,
            _ => ConfirmKey::Ignore,
        }
    } else if tab && key.modifiers == KeyModifiers::SHIFT {
        ConfirmKey::Toggle
    } else {
        ConfirmKey::Ignore
    }
}

/// The screen an Empty Trash confirmation returns to.
fn trash_confirmation_origin(from_picker: bool) -> Screen {
    if from_picker {
        Screen::ListPicker
    } else {
        Screen::Navigation
    }
}

/// Packs `key label` hints into at most `max_lines` lines of `width` cells.
/// An entry with an empty key is free text. Entries that do not fit are skipped.
fn pack_hints(entries: &[(String, String)], width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (key, label) in entries {
        let text = if key.is_empty() {
            label.clone()
        } else {
            format!("{key} {label}")
        };
        let fits = lines
            .last()
            .is_some_and(|line| line.width() + 3 + text.width() <= width);
        if fits {
            let line = lines.last_mut().expect("a current line exists");
            line.push_str(" | ");
            line.push_str(&text);
        } else if lines.len() < max_lines {
            lines.push(text);
        }
    }
    lines
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeldTask {
    id: i64,
    version: i64,
    after: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
enum UndoKind {
    Status {
        status: Status,
        status_changed_on: Option<NaiveDate>,
        status_changed_at: Option<DateTime<Utc>>,
    },
    Due(Option<NaiveDate>),
    Deleted(bool),
    List(i64),
    Fields(TaskInput),
    Swap {
        other_id: i64,
        other_version: i64,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct UndoEntry {
    task_id: i64,
    version: i64,
    kind: UndoKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelpOrigin {
    Tasks,
    Lists,
    Editor,
}

enum HelpEntry {
    Action(Action),
    Builtin(String, &'static str),
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
        let active = if task.is_some() { DESCRIPTION } else { TITLE };
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
        self.fields[LIST].text.parse().ok()
    }

    fn choose_list(&mut self, id: i64) {
        self.fields[LIST].replace_all(&id.to_string());
    }

    fn focus_field(&mut self, index: usize, today: NaiveDate) {
        if self.active == DUE
            && index != DUE
            && let Ok(date) = parse_editor_date(&self.fields[DUE].text, today)
        {
            self.fields[DUE].replace_all(&date.map(|date| date.to_string()).unwrap_or_default());
        }
        self.active = index;
    }

    /// The field after (or before) the active one in `FIELD_ORDER`, wrapping at both ends.
    fn adjacent_field(&self, forward: bool) -> usize {
        let count = FIELD_ORDER.len();
        let position = FIELD_ORDER
            .iter()
            .position(|&index| index == self.active)
            .unwrap_or(0);
        let next = if forward {
            (position + 1) % count
        } else {
            (position + count - 1) % count
        };
        FIELD_ORDER[next]
    }

    fn input(&self, lists: &[TaskList], language: Language, today: NaiveDate) -> Result<TaskInput> {
        let due = parse_editor_date(&self.fields[DUE].text, today)
            .with_context(|| tr(language, "invalid_date"))?;
        let list_id = lists
            .iter()
            .find(|list| Some(list.id) == self.selected_list_id())
            .map(|list| list.id)
            .with_context(|| tr(language, "unknown_list"))?;
        Ok(TaskInput {
            list_id,
            title: self.fields[TITLE].text.clone(),
            description: self.fields[DESCRIPTION].text.clone(),
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
    /// Delete forever confirmation for one task in Trash, opened from the Trash view.
    PurgeTaskConfirm {
        task: Box<Task>,
        delete: bool,
        changed: bool,
    },
    /// Empty Trash confirmation, opened from the Trash view or the list picker.
    EmptyTrashConfirm {
        preview: TrashPreview,
        delete: bool,
        changed: bool,
        from_picker: bool,
    },
    Editor(Box<TaskEditor>),
    EditorListPicker {
        editor: Box<TaskEditor>,
        picker: ListPicker,
    },
    MoveTaskPicker {
        task_id: i64,
        version: i64,
        list_id: i64,
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
    /// The popup picker of the editor or move-task screen, through Help or the language picker.
    fn editor_picker(&self) -> Option<&ListPicker> {
        match self {
            Self::EditorListPicker { picker, .. } | Self::MoveTaskPicker { picker, .. } => {
                Some(picker)
            }
            Self::Help { previous } | Self::Language { previous, .. } => previous.editor_picker(),
            _ => None,
        }
    }

    fn editor_picker_mut(&mut self) -> Option<&mut ListPicker> {
        match self {
            Self::EditorListPicker { picker, .. } | Self::MoveTaskPicker { picker, .. } => {
                Some(picker)
            }
            Self::Help { previous } | Self::Language { previous, .. } => {
                previous.editor_picker_mut()
            }
            _ => None,
        }
    }

    fn help_origin(&self) -> HelpOrigin {
        match self {
            Self::ListPicker => HelpOrigin::Lists,
            Self::EditorListPicker { .. } => HelpOrigin::Editor,
            Self::Help { previous } | Self::Language { previous, .. } => previous.help_origin(),
            _ => HelpOrigin::Tasks,
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
    help_scroll: usize,
    help_rows: usize,
    help_page: usize,
    page_rows: usize,
    copy_page: usize,
    held: Vec<HeldTask>,
    undo: Option<UndoEntry>,
    last_list: Option<i64>,
    today: NaiveDate,
    message: Option<TransientMessage>,
    data_version: i64,
    external_update: bool,
    sync_counters: SyncCounters,
    sync_notice: Option<SyncNotice>,
    sync_requested: bool,
    /// True while `refresh_changes` runs its refresh.
    periodic_refresh: bool,
    /// Set when a refresh found a change by another program.
    external_seen: bool,
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
            help_scroll: 0,
            help_rows: 0,
            help_page: 10,
            page_rows: 10,
            copy_page: 10,
            held: Vec::new(),
            undo: None,
            last_list: None,
            today,
            message: None,
            data_version: store.data_version()?,
            external_update: false,
            sync_counters: store.sync_counters()?,
            sync_notice: None,
            sync_requested: false,
            periodic_refresh: false,
            external_seen: false,
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
            Screen::DeleteListConfirm { .. }
            | Screen::PurgeTaskConfirm { .. }
            | Screen::EmptyTrashConfirm { .. } => Mode::DeleteConfirm,
            Screen::Editor(ref editor) if editor.active == DUE => Mode::DueField,
            Screen::Editor(ref editor) if editor.active == LIST => Mode::ListField,
            Screen::Editor(_) => Mode::Editor,
            Screen::EditorListPicker { .. } | Screen::MoveTaskPicker { .. } => {
                Mode::EditorListPicker
            }
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
        let mut unexplained = false;
        if data_version != self.data_version {
            // A sync commit is not a change by another program. It gets its own
            // message instead of the notice that stays until F5.
            let counters = store.sync_counters()?;
            if counters.commits == self.sync_counters.commits {
                self.external_update = true;
                self.external_seen = true;
            } else if counters.conflicts != self.sync_counters.conflicts {
                self.sync_notice = Some(SyncNotice::Conflict);
            } else if counters.applied != self.sync_counters.applied {
                if self.sync_notice.is_none() {
                    self.sync_notice = Some(SyncNotice::Received);
                }
            } else {
                // The sync commit changed no list or task. If they differ anyway,
                // another program committed in the same interval.
                unexplained = self.periodic_refresh;
            }
            self.sync_counters = counters;
        }
        let mut selected_task = self.selected_task().map(|task| task.id);
        let selected_list = self.picker_identity(self.picker.selection);
        let editor_list = self
            .screen
            .editor_picker()
            .and_then(|picker| self.lists.get(picker.selection))
            .map(|list| list.id);
        let lists = store.lists()?;
        let tasks = store.tasks()?;
        if unexplained && (lists != self.lists || tasks != self.tasks) {
            self.external_update = true;
            self.external_seen = true;
        }
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
        self.apply_holds();
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
        let external = store.data_version()? != self.data_version;
        let date_changed = self.today != today;
        self.today = today;
        if manual || date_changed {
            self.held.clear();
        }
        self.external_seen = false;
        let outcome_message = if external || date_changed || manual {
            // Only this refresh may compare stored tasks with the loaded ones. A
            // refresh after the user's own action would see that action as a difference.
            self.periodic_refresh = true;
            let refreshed = self.refresh(store);
            self.periodic_refresh = false;
            refreshed?;
            self.refresh_confirmation(store, config)?
        } else {
            false
        };
        if self.external_seen {
            self.external_update = true;
        } else if manual {
            self.external_update = false;
        }
        if manual && !outcome_message {
            self.show_message(tr(config.language, "refreshed"), false, Instant::now());
        }
        Ok(())
    }

    /// Rechecks an open deletion confirmation against fresh data, resetting the choice to No
    /// when its target changed. Returns true when it closed the confirmation with a message.
    fn refresh_confirmation(&mut self, store: &Store, config: &Config) -> Result<bool> {
        let language = config.language;
        match &mut self.screen {
            Screen::DeleteListConfirm {
                preview,
                delete,
                changed,
            } => {
                if !self.lists.iter().any(|list| list.id == preview.list_id) {
                    self.screen = Screen::ListPicker;
                    self.show_message(tr(language, "list_delete_missing"), true, Instant::now());
                    return Ok(true);
                }
                let current = store.preview_list_deletion(preview.list_id)?;
                if current != **preview {
                    **preview = current;
                    *delete = false;
                    *changed = true;
                }
            }
            Screen::PurgeTaskConfirm {
                task,
                delete,
                changed,
            } => {
                let Some(current) = self
                    .tasks
                    .iter()
                    .find(|current| current.id == task.id && current.deleted)
                else {
                    self.screen = Screen::Navigation;
                    self.show_message(tr(language, "task_left_trash"), true, Instant::now());
                    return Ok(true);
                };
                if current.version != task.version {
                    **task = current.clone();
                    *delete = false;
                    *changed = true;
                }
            }
            Screen::EmptyTrashConfirm {
                preview,
                delete,
                changed,
                from_picker,
            } => {
                let current = store.preview_empty_trash()?;
                if current.count == 0 {
                    self.screen = trash_confirmation_origin(*from_picker);
                    // An error is not hidden behind the external update notice.
                    self.show_message(tr(language, "trash_empty"), true, Instant::now());
                    return Ok(true);
                }
                if current != *preview {
                    *preview = current;
                    *delete = false;
                    *changed = true;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn wants_mouse_capture(&self) -> bool {
        self.mouse_enabled && !matches!(self.screen, Screen::CopyText { .. })
    }

    fn open_copy_text(&mut self) {
        let text = match &self.screen {
            Screen::Editor(editor) => {
                format!(
                    "{}\n\n{}",
                    editor.fields[TITLE].text, editor.fields[DESCRIPTION].text
                )
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
            View::Planned | View::History(None) => self
                .last_list
                .filter(|id| self.lists.iter().any(|list| list.id == *id))
                .map_or(RowIdentity::AllTasks, RowIdentity::List),
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
        if date_changed {
            self.held.clear();
        }
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
                Screen::EditorListPicker { picker, .. } | Screen::MoveTaskPicker { picker, .. } => {
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
                    self.help_scroll = if down {
                        self.help_scroll.saturating_add(1)
                    } else {
                        self.help_scroll.saturating_sub(1)
                    };
                    self.clamp_help_scroll();
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
                if matches!(
                    self.screen,
                    Screen::EditorListPicker { .. } | Screen::MoveTaskPicker { .. }
                ) =>
            {
                let Some(list) = self.lists.get(index) else {
                    return Ok(());
                };
                let identity = RowIdentity::List(list.id);
                let double_click = self.last_click.is_some_and(|(previous, at)| {
                    previous == identity
                        && now.saturating_duration_since(at) <= Duration::from_millis(400)
                });
                let moving = matches!(self.screen, Screen::MoveTaskPicker { .. });
                if let Screen::EditorListPicker { picker, .. }
                | Screen::MoveTaskPicker { picker, .. } = &mut self.screen
                {
                    picker.selection = index;
                    picker.state.select(Some(index));
                }
                if double_click {
                    self.last_click = None;
                    if moving {
                        self.choose_move_destination(store, config)?;
                    } else {
                        self.choose_editor_list(store, config)?;
                    }
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
                    if index == DESCRIPTION {
                        let layout = TextLayout::new(
                            &editor.fields[index].text,
                            usize::from(rect.width.saturating_sub(1)),
                        );
                        editor.fields[index].set_cursor(layout.cursor_at(row, column), false);
                    } else if index != LIST {
                        editor.fields[index].set_cursor_position(row, column);
                    }
                }
                if index == LIST && matches!(self.screen, Screen::Editor(_)) {
                    self.open_editor_list_picker(store)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn clamp_help_scroll(&mut self) {
        self.help_scroll = self
            .help_scroll
            .min(self.help_rows.saturating_sub(self.help_page.max(1)));
    }

    /// Keeps just-closed tasks in their previous rows until the view changes.
    fn apply_holds(&mut self) {
        let visible = &self.visible;
        self.held.retain(|hold| {
            visible.iter().any(|task| {
                task.id == hold.id
                    && task.version == hold.version
                    && task.status != Status::Open
                    && !task.deleted
            })
        });
        // Place a hold after its anchor row is placed, so chains of held rows keep their order.
        let depth = |hold: &HeldTask| {
            let mut depth = 0;
            let mut anchor = hold.after;
            while let Some(id) = anchor.filter(|_| depth < self.held.len()) {
                match self.held.iter().find(|held| held.id == id) {
                    Some(held) => {
                        depth += 1;
                        anchor = held.after;
                    }
                    None => break,
                }
            }
            depth
        };
        let mut ordered = self.held.clone();
        ordered.sort_by_key(depth);
        for hold in ordered {
            let Some(from) = self.visible.iter().position(|task| task.id == hold.id) else {
                continue;
            };
            let task = self.visible.remove(from);
            let to = match hold.after {
                None => 0,
                Some(after) => self
                    .visible
                    .iter()
                    .position(|task| task.id == after)
                    .map_or(from.min(self.visible.len()), |index| index + 1),
            };
            self.visible.insert(to, task);
        }
    }

    fn is_held(&self, id: i64) -> bool {
        self.held.iter().any(|hold| hold.id == id)
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

    /// Passes an F5 request to the sync worker and shows what sync did or why it failed.
    fn report_sync(&mut self, sync: Option<&SyncWorker>, config: &Config, now: Instant) {
        let requested = std::mem::take(&mut self.sync_requested);
        if let Some(notice) = self.sync_notice.take() {
            let key = match notice {
                SyncNotice::Received => "sync_received",
                SyncNotice::Conflict => "sync_conflict",
            };
            self.show_message(tr(config.language, key), false, now);
        }
        let Some(sync) = sync else {
            return;
        };
        if requested {
            sync.sync_now();
        }
        if let Some(failure) = sync.take_failure() {
            self.show_message(
                sync_failure_message(config.language, failure.kind),
                true,
                now,
            );
        }
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
            Screen::Editor(editor) if editor.active != LIST => &editor.fields[editor.active],
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
        if date_changed {
            self.held.clear();
        }
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
            && editor.active == DUE
        {
            let value = preset
                .resolve(self.today)?
                .map(|date| date.to_string())
                .unwrap_or_default();
            editor.fields[DUE].replace_all(&value);
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
                self.sync_requested = true;
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
        let moving = matches!(self.screen, Screen::MoveTaskPicker { .. });
        match &mut self.screen {
            Screen::EditorListPicker { picker, .. } | Screen::MoveTaskPicker { picker, .. } => {
                match action {
                    Some(action @ (Action::Up | Action::Down | Action::First | Action::Last)) => {
                        picker.selection =
                            navigation_selection(picker.selection, self.lists.len(), action);
                        picker
                            .state
                            .select((!self.lists.is_empty()).then_some(picker.selection));
                    }
                    Some(action @ (Action::PageUp | Action::PageDown)) => {
                        let page = picker.area.map_or(10, |area| usize::from(area.height));
                        picker.selection = page_selection(
                            picker.selection,
                            self.lists.len(),
                            page,
                            action == Action::PageDown,
                        );
                        picker
                            .state
                            .select((!self.lists.is_empty()).then_some(picker.selection));
                    }
                    Some(Action::Open) => {
                        if moving {
                            self.choose_move_destination(store, config)?;
                        } else {
                            self.choose_editor_list(store, config)?;
                        }
                    }
                    Some(Action::Back | Action::Lists) => {
                        if moving {
                            self.screen = Screen::Navigation;
                            self.last_click = None;
                        } else {
                            self.close_editor_list_picker();
                        }
                        self.refresh(store)?;
                    }
                    Some(Action::Help) => self.open_help(),
                    _ => {}
                }
                return Ok(false);
            }
            Screen::Language { selected, .. } => {
                match action {
                    Some(action @ (Action::Up | Action::Down | Action::First | Action::Last)) => {
                        let index = usize::from(*selected == Language::Da);
                        *selected = if navigation_selection(index, 2, action) == 0 {
                            Language::En
                        } else {
                            Language::Da
                        };
                    }
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
                    Some(action @ (Action::Up | Action::Down | Action::First | Action::Last)) => {
                        self.picker.selection = navigation_selection(
                            self.picker.selection,
                            self.lists.len() + 2,
                            action,
                        );
                        self.clamp_picker();
                    }
                    Some(action @ (Action::PageUp | Action::PageDown)) => {
                        let page = self.picker.area.map_or(10, |area| usize::from(area.height));
                        self.picker.selection = page_selection(
                            self.picker.selection,
                            self.lists.len() + 2,
                            page,
                            action == Action::PageDown,
                        );
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
                    Some(Action::DeleteList) => match self.picker_identity(self.picker.selection) {
                        Some(RowIdentity::List(id)) => {
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
                        Some(RowIdentity::Trash) => {
                            self.confirm_empty_trash(store, config, true)?
                        }
                        _ => {}
                    },
                    Some(Action::MoveUp | Action::MoveDown) => {
                        if let Some(RowIdentity::List(id)) =
                            self.picker_identity(self.picker.selection)
                        {
                            store.move_list(id, action == Some(Action::MoveDown))?;
                            self.refresh(store)?;
                        }
                    }
                    Some(Action::Help) => self.open_help(),
                    _ => {}
                }
                return Ok(false);
            }
            Screen::DeleteListConfirm { delete, .. }
            | Screen::PurgeTaskConfirm { delete, .. }
            | Screen::EmptyTrashConfirm { delete, .. } => {
                match confirm_key(key, action, Action::Back) {
                    ConfirmKey::Cancel => self.cancel_deletion(store)?,
                    ConfirmKey::Answer(choice) => {
                        *delete = choice;
                        self.finish_deletion(store, config)?;
                    }
                    ConfirmKey::Toggle => *delete = !*delete,
                    ConfirmKey::Accept => self.finish_deletion(store, config)?,
                    ConfirmKey::Ignore => {}
                }
                return Ok(false);
            }
            Screen::ConfirmDiscard { discard, .. } => {
                match confirm_key(key, action, Action::Discard) {
                    ConfirmKey::Cancel => self.finish_discard_confirmation(false, store)?,
                    ConfirmKey::Answer(choice) => {
                        self.finish_discard_confirmation(choice, store)?
                    }
                    ConfirmKey::Toggle => *discard = !*discard,
                    ConfirmKey::Accept => {
                        let discard = *discard;
                        self.finish_discard_confirmation(discard, store)?;
                    }
                    ConfirmKey::Ignore => {}
                }
                return Ok(false);
            }
            Screen::CopyText {
                text, row, column, ..
            } => {
                let last_row = text
                    .lines()
                    .count()
                    .saturating_sub(1)
                    .min(u16::MAX as usize) as u16;
                let page = self.copy_page.clamp(1, u16::MAX as usize) as u16;
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
                    Some(Action::Down) => *row = row.saturating_add(1).min(last_row),
                    Some(Action::PageUp) => *row = row.saturating_sub(page),
                    Some(Action::PageDown) => *row = row.saturating_add(page).min(last_row),
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
                    Some(Action::Open | Action::Lists) if editor.active == LIST => {
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
                        editor.focus_field(editor.adjacent_field(true), self.today);
                    }
                    Some(Action::PreviousField) => {
                        editor.focus_field(editor.adjacent_field(false), self.today);
                    }
                    _ => {
                        if key.code == KeyCode::Enter
                            && key.modifiers.is_empty()
                            && matches!(editor.active, TITLE | DUE)
                        {
                            self.save_editor(store, config)?;
                        } else if editor.active == LIST {
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && !key.modifiers.contains(KeyModifiers::ALT)
                                && matches!(key.code, KeyCode::Char('z' | 'Z' | 'y' | 'Y'))
                            {
                                editor.fields[LIST].key(key, false);
                            }
                        } else if editor.active == DESCRIPTION
                            && matches!(key.code, KeyCode::Up | KeyCode::Down)
                        {
                            editor.fields[DESCRIPTION].move_wrapped_vertical(
                                key.code == KeyCode::Down,
                                self.description_width,
                                key.modifiers.contains(KeyModifiers::SHIFT),
                            );
                        } else {
                            editor.fields[editor.active].key(key, editor.active == DESCRIPTION);
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
                        self.held.clear();
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
                    Some(Action::Up) => self.help_scroll = self.help_scroll.saturating_sub(1),
                    Some(Action::Down) => self.help_scroll = self.help_scroll.saturating_add(1),
                    Some(Action::PageUp) => {
                        self.help_scroll = self.help_scroll.saturating_sub(self.help_page.max(1));
                    }
                    Some(Action::PageDown) => {
                        self.help_scroll = self.help_scroll.saturating_add(self.help_page.max(1));
                    }
                    Some(Action::First) => self.help_scroll = 0,
                    Some(Action::Last) => self.help_scroll = usize::MAX,
                    _ => {}
                }
                self.clamp_help_scroll();
                return Ok(false);
            }
            Screen::Navigation => {}
        }
        let Some(action) = action else {
            return Ok(false);
        };
        match action {
            Action::Quit => return Ok(true),
            action @ (Action::Up | Action::Down | Action::First | Action::Last) => {
                self.selection = navigation_selection(self.selection, self.row_count(), action);
                self.clamp_selection();
            }
            action @ (Action::PageUp | Action::PageDown) => {
                self.selection = page_selection(
                    self.selection,
                    self.row_count(),
                    self.page_rows,
                    action == Action::PageDown,
                );
                self.clamp_selection();
            }
            Action::Lists => {
                self.refresh(store)?;
                self.open_picker();
            }
            Action::AllTasks => self.set_view(View::Planned, store)?,
            Action::Back if !self.filter.is_empty() => self.clear_filter(store)?,
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
            Action::Help => self.open_help(),
            Action::MoveTask => self.open_move_picker(store)?,
            Action::Undo if self.view != View::Trash => self.undo_last(store, config)?,
            Action::Trash if self.view == View::Trash => self.confirm_task_purge(),
            Action::EmptyTrash => {
                if self.view == View::Trash {
                    self.confirm_empty_trash(store, config, false)?;
                }
            }
            _ => self.task_action(action, store, config)?,
        }
        Ok(false)
    }

    fn open_help(&mut self) {
        let previous = Box::new(std::mem::replace(&mut self.screen, Screen::Navigation));
        self.screen = Screen::Help { previous };
        self.help_scroll = 0;
        self.last_click = None;
    }

    /// Removes the applied filter and keeps the current view and selected task.
    fn clear_filter(&mut self, store: &Store) -> Result<()> {
        let selected = self.selected_task().map(|task| task.id);
        self.filter.clear();
        self.query = Query::default();
        self.held.clear();
        self.refresh(store)?;
        self.select_task_id(selected);
        Ok(())
    }

    fn open_move_picker(&mut self, store: &Store) -> Result<()> {
        self.refresh(store)?;
        let Some(task) = self.selected_task().filter(|task| !task.deleted) else {
            return Ok(());
        };
        let (task_id, version, list_id) = (task.id, task.version, task.list_id);
        let selection = self
            .lists
            .iter()
            .position(|list| list.id == list_id)
            .unwrap_or(0);
        self.screen = Screen::MoveTaskPicker {
            task_id,
            version,
            list_id,
            picker: ListPicker {
                selection,
                state: ListState::default()
                    .with_selected((!self.lists.is_empty()).then_some(selection)),
                area: None,
            },
        };
        self.last_click = None;
        Ok(())
    }

    fn choose_move_destination(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Screen::MoveTaskPicker {
            task_id,
            version,
            list_id,
            picker,
        } = &self.screen
        else {
            return Ok(());
        };
        let (task_id, version, previous_list) = (*task_id, *version, *list_id);
        let Some(destination) = self.lists.get(picker.selection).cloned() else {
            return Ok(());
        };
        self.screen = Screen::Navigation;
        self.last_click = None;
        let updated = store.set_list(task_id, version, destination.id)?;
        if updated.version == version {
            return self.refresh(store);
        }
        self.undo = Some(UndoEntry {
            task_id,
            version: updated.version,
            kind: UndoKind::List(previous_list),
        });
        self.held.retain(|hold| hold.id != task_id);
        self.refresh(store)?;
        self.select_task_id(Some(task_id));
        self.show_message(
            format!(
                "{} {}",
                tr(config.language, "moved_to"),
                destination.name.replace(['\r', '\n'], " ")
            ),
            false,
            Instant::now(),
        );
        Ok(())
    }

    fn undo_last(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Some(entry) = self.undo.take() else {
            self.show_message(
                tr(config.language, "nothing_to_undo"),
                false,
                Instant::now(),
            );
            return Ok(());
        };
        let current = store.task(entry.task_id)?;
        let changed = |id: i64| {
            anyhow::anyhow!("Task {id} changed since the last action. Nothing was undone.")
        };
        if current.version != entry.version {
            return Err(changed(entry.task_id));
        }
        match entry.kind {
            UndoKind::Status {
                status,
                status_changed_on,
                status_changed_at,
            } => {
                store.restore_status(
                    current.id,
                    current.version,
                    status,
                    status_changed_on,
                    status_changed_at,
                )?;
            }
            UndoKind::Due(previous) => {
                store.set_due(current.id, current.version, previous)?;
            }
            UndoKind::Deleted(previous) => {
                store.set_deleted(current.id, current.version, previous)?;
            }
            UndoKind::List(previous) => {
                store.set_list(current.id, current.version, previous)?;
            }
            UndoKind::Fields(previous) => {
                store.update_task(current.id, current.version, &previous)?;
            }
            UndoKind::Swap {
                other_id,
                other_version,
            } => {
                let other = store.task(other_id)?;
                if other.version != other_version {
                    return Err(changed(other_id));
                }
                store.swap_positions(&current, &other)?;
            }
        }
        self.held.retain(|hold| hold.id != entry.task_id);
        self.refresh(store)?;
        self.select_task_id(Some(entry.task_id));
        self.show_message(tr(config.language, "undone"), false, Instant::now());
        Ok(())
    }

    fn paste(&mut self, text: &str) {
        match &mut self.screen {
            Screen::Editor(editor) if editor.active != LIST => {
                let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                let normalized = if editor.active == DESCRIPTION {
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
        for visited in [self.view, view] {
            if let View::List(id) | View::History(Some(id)) = visited {
                self.last_list = Some(id);
            }
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
        self.held.clear();
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

    /// Closes a deletion confirmation without deleting, returning to the screen that opened it.
    fn cancel_deletion(&mut self, store: &Store) -> Result<()> {
        self.screen = match self.screen {
            Screen::DeleteListConfirm { .. }
            | Screen::EmptyTrashConfirm {
                from_picker: true, ..
            } => Screen::ListPicker,
            _ => Screen::Navigation,
        };
        self.refresh(store)
    }

    /// Applies the selected choice of the open deletion confirmation.
    fn finish_deletion(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        match self.screen {
            Screen::DeleteListConfirm { .. } => self.finish_list_deletion(store, config),
            Screen::PurgeTaskConfirm { .. } => self.finish_task_purge(store, config),
            Screen::EmptyTrashConfirm { .. } => self.finish_empty_trash(store, config),
            _ => Ok(()),
        }
    }

    /// Opens the Delete forever confirmation for the selected task in Trash.
    fn confirm_task_purge(&mut self) {
        if let Some(task) = self.selected_task().filter(|task| task.deleted) {
            self.screen = Screen::PurgeTaskConfirm {
                task: Box::new(task.clone()),
                delete: false,
                changed: false,
            };
        }
    }

    fn finish_task_purge(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Screen::PurgeTaskConfirm { task, delete, .. } = &self.screen else {
            return Ok(());
        };
        if !delete {
            return self.cancel_deletion(store);
        }
        let language = config.language;
        match store.purge_task(task.id, task.version)? {
            TaskPurgeOutcome::Deleted => {
                self.screen = Screen::Navigation;
                // A deleted task cannot be restored, so no earlier action can be undone safely.
                self.undo = None;
                self.refresh(store)?;
                self.show_message(tr(language, "deleted_forever"), false, Instant::now());
            }
            TaskPurgeOutcome::Changed(current) => {
                self.screen = Screen::PurgeTaskConfirm {
                    task: Box::new(current),
                    delete: false,
                    changed: true,
                };
                self.refresh(store)?;
            }
            TaskPurgeOutcome::NotInTrash => {
                self.screen = Screen::Navigation;
                self.refresh(store)?;
                self.show_message(tr(language, "task_left_trash"), true, Instant::now());
            }
        }
        Ok(())
    }

    /// Opens the Empty Trash confirmation, or reports an empty Trash without opening it.
    fn confirm_empty_trash(
        &mut self,
        store: &Store,
        config: &Config,
        from_picker: bool,
    ) -> Result<()> {
        let preview = store.preview_empty_trash()?;
        if preview.count == 0 {
            self.show_message(tr(config.language, "trash_empty"), false, Instant::now());
        } else {
            self.screen = Screen::EmptyTrashConfirm {
                preview,
                delete: false,
                changed: false,
                from_picker,
            };
        }
        Ok(())
    }

    fn finish_empty_trash(&mut self, store: &mut Store, config: &Config) -> Result<()> {
        let Screen::EmptyTrashConfirm {
            preview,
            delete,
            from_picker,
            ..
        } = &self.screen
        else {
            return Ok(());
        };
        if !delete {
            return self.cancel_deletion(store);
        }
        let from_picker = *from_picker;
        let language = config.language;
        match store.empty_trash(preview)? {
            EmptyTrashOutcome::Emptied(_) => {
                self.screen = trash_confirmation_origin(from_picker);
                self.undo = None;
                self.refresh(store)?;
                self.show_message(tr(language, "trash_emptied"), false, Instant::now());
            }
            EmptyTrashOutcome::Changed(preview) if preview.count == 0 => {
                // Another connection emptied Trash. An error is not hidden behind the external
                // update notice, which the refresh below sets.
                self.screen = trash_confirmation_origin(from_picker);
                self.refresh(store)?;
                self.show_message(tr(language, "trash_empty"), true, Instant::now());
            }
            EmptyTrashOutcome::Changed(preview) => {
                self.screen = Screen::EmptyTrashConfirm {
                    preview,
                    delete: false,
                    changed: true,
                    from_picker,
                };
                self.refresh(store)?;
            }
        }
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
            return self.cancel_deletion(store);
        }
        let destination = preview.destination_id;
        match store.delete_list(preview)? {
            ListDeletionOutcome::Deleted => {
                self.screen = Screen::ListPicker;
                self.undo = None;
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
            self.held.clear();
        }
        Ok(())
    }

    fn new_task(&mut self) {
        let list_id = match self.view {
            View::List(id) => self
                .lists
                .iter()
                .find(|list| list.id == id)
                .map(|list| list.id),
            _ => suggested_list_id(&self.tasks, &self.lists),
        };
        if let Some(list_id) = list_id {
            self.screen = Screen::Editor(Box::new(TaskEditor::new(None, list_id)));
            self.message = None;
            self.held.clear();
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
        let (task, kind, message) = if let Some(original) = &editor.original {
            (
                store.update_task(original.id, original.version, &input)?,
                UndoKind::Fields(TaskInput::from(original)),
                "saved",
            )
        } else {
            (
                store.create_task(&input)?,
                UndoKind::Deleted(true),
                "created",
            )
        };
        self.undo = Some(UndoEntry {
            task_id: task.id,
            version: task.version,
            kind,
        });
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
        self.show_message(tr(config.language, message), false, Instant::now());
        Ok(())
    }

    fn task_action(&mut self, action: Action, store: &mut Store, config: &Config) -> Result<()> {
        let Some(task) = self.selected_task().cloned() else {
            return Ok(());
        };
        if task.deleted && !matches!(action, Action::Restore) {
            return Ok(());
        }
        let language = config.language;
        if let Some(preset) = due_preset(action) {
            let updated = store.set_due(task.id, task.version, preset.resolve(self.today)?)?;
            if updated.version != task.version {
                self.undo = Some(UndoEntry {
                    task_id: task.id,
                    version: updated.version,
                    kind: UndoKind::Due(task.due),
                });
                let message = match (updated.due, task.due) {
                    (Some(date), _) => {
                        format!(
                            "{} {}",
                            tr(language, "planned_for"),
                            date_label(date, language)
                        )
                    }
                    (None, Some(previous)) => format!(
                        "{} ({} {})",
                        tr(language, "due_cleared"),
                        tr(language, "previous_value"),
                        date_label(previous, language)
                    ),
                    (None, None) => tr(language, "updated").to_owned(),
                };
                self.refresh(store)?;
                self.show_message(message, false, Instant::now());
            } else {
                self.refresh(store)?;
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
        let previous_row = self
            .selection
            .checked_sub(1)
            .and_then(|index| self.visible.get(index))
            .map(|task| task.id);
        let status_undo = UndoKind::Status {
            status: task.status,
            status_changed_on: task.status_changed_on,
            status_changed_at: task.status_changed_at,
        };
        let (updated, kind, message) = match action {
            Action::ToggleDone => {
                let status = if task.status == Status::Done {
                    Status::Open
                } else {
                    Status::Done
                };
                let updated = store.set_status(task.id, task.version, status)?;
                let message = if status == Status::Done {
                    "marked_done"
                } else {
                    "reopened"
                };
                (updated, status_undo, message)
            }
            Action::ToggleCancelled => {
                let status = if task.status == Status::Cancelled {
                    Status::Open
                } else {
                    Status::Cancelled
                };
                let updated = store.set_status(task.id, task.version, status)?;
                let message = if status == Status::Cancelled {
                    "marked_cancelled"
                } else {
                    "reopened"
                };
                (updated, status_undo, message)
            }
            Action::Trash => (
                store.set_deleted(task.id, task.version, true)?,
                UndoKind::Deleted(false),
                "deleted",
            ),
            Action::Restore if task.deleted => (
                store.set_deleted(task.id, task.version, false)?,
                UndoKind::Deleted(true),
                "restored",
            ),
            Action::Reopen if matches!(self.view, View::History(_)) => (
                store.set_status(task.id, task.version, Status::Open)?,
                status_undo,
                "reopened",
            ),
            Action::MoveUp | Action::MoveDown => {
                if task.status != Status::Open || matches!(self.view, View::History(_)) {
                    return Ok(());
                }
                let step = |index: usize| {
                    if matches!(action, Action::MoveUp) {
                        index.checked_sub(1)
                    } else {
                        index.checked_add(1)
                    }
                };
                // Held rows are closed tasks that only look like neighbors.
                let mut index = step(self.selection);
                let other = loop {
                    let Some(candidate) = index.and_then(|index| self.visible.get(index)) else {
                        return Ok(());
                    };
                    if self.is_held(candidate.id) {
                        index = index.and_then(step);
                        continue;
                    }
                    break candidate;
                };
                if task.due != other.due
                    || (task.status == Status::Open) != (other.status == Status::Open)
                {
                    return Ok(());
                }
                store.swap_positions(&task, other)?;
                let (other_id, other_version) = (other.id, other.version);
                // A successful swap increments both versions exactly once.
                let mut updated = task.clone();
                updated.version += 1;
                (
                    updated,
                    UndoKind::Swap {
                        other_id,
                        other_version: other_version + 1,
                    },
                    "updated",
                )
            }
            _ => return Ok(()),
        };
        self.undo = Some(UndoEntry {
            task_id: task.id,
            version: updated.version,
            kind,
        });
        self.held.retain(|hold| hold.id != task.id);
        if closing && !matches!(self.view, View::History(_)) {
            self.held.push(HeldTask {
                id: task.id,
                version: updated.version,
                after: previous_row,
            });
        }
        self.refresh(store)?;
        self.select_task_id(next_open);
        self.show_message(tr(language, message), false, Instant::now());
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
                    | Screen::PurgeTaskConfirm { .. }
                    | Screen::EmptyTrashConfirm { .. }
            ) && area.height < 18)
        {
            frame.render_widget(
                Paragraph::new(tr(language, "terminal_too_small")).wrap(Wrap { trim: false }),
                area,
            );
            return;
        }
        let hint_lines = pack_hints(&self.footer_hints(config), usize::from(area.width), 2);
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(if matches!(self.screen, Screen::Editor(_)) {
                    3
                } else {
                    2
                }),
                Constraint::Min(1),
                Constraint::Length(hint_lines.len() as u16 + 2),
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
        if matches!(&self.screen, Screen::Editor(editor) if editor.active == DUE) {
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
        } else if matches!(&self.screen, Screen::Editor(editor) if editor.active == LIST) {
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
            Screen::MoveTaskPicker { .. } => {
                let list_state = self.list_state;
                self.draw_task_view(frame, sections[1], config);
                self.list_state = list_state;
                self.mouse_targets.clear();
                self.draw_move_task_picker(frame, sections[1], language);
            }
            Screen::DeleteListConfirm {
                preview,
                delete,
                changed,
            } => {
                draw_list_deletion(frame, sections[1], preview, *delete, *changed, language);
            }
            Screen::PurgeTaskConfirm {
                task,
                delete,
                changed,
            } => {
                draw_task_purge(frame, sections[1], task, *delete, *changed, language);
            }
            Screen::EmptyTrashConfirm {
                preview,
                delete,
                changed,
                ..
            } => {
                draw_empty_trash(frame, sections[1], preview, *delete, *changed, language);
            }
            Screen::ConfirmDiscard { editor, discard } => {
                let _ = draw_editor(frame, sections[1], editor, &self.lists, language);
                draw_discard_confirmation(frame, sections[1], *discard, language);
            }
            Screen::Editor(editor) => {
                self.mouse_targets = draw_editor(frame, sections[1], editor, &self.lists, language);
                if let Some((rect, _)) = self.mouse_targets.iter().find(|(_, target)| {
                    matches!(
                        target,
                        MouseTarget::EditorField {
                            index: DESCRIPTION,
                            ..
                        }
                    )
                }) {
                    self.description_width = usize::from(rect.width.saturating_sub(1));
                }
            }
            Screen::Help { .. } => {
                let lines = help_lines(config, self.screen.help_origin());
                self.help_rows = lines.len();
                self.help_page = usize::from(sections[1].height.saturating_sub(1));
                self.clamp_help_scroll();
                draw_help(frame, sections[1], config, lines, self.help_scroll);
            }
            Screen::CopyText {
                text, row, column, ..
            } => {
                self.copy_page = usize::from(sections[1].height);
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
        let mut footer: Vec<_> = hint_lines
            .into_iter()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::DarkGray))))
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

    /// The footer entries of the current screen in priority order.
    fn footer_hints(&self, config: &Config) -> Vec<(String, String)> {
        let language = config.language;
        let key = |action: Action| config.primary_key(action);
        let label = |name: &str| tr(language, name).to_owned();
        let pair = |action: Action, name: &str| (key(action), label(name));
        let text = |name: &str| (String::new(), label(name));
        let arrows = |up: Action, down: Action| format!("{}/{}", key(up), key(down));
        match &self.screen {
            Screen::EditorListPicker { .. } => vec![
                pair(Action::Open, "choose_short"),
                pair(Action::Back, "action_back"),
                pair(Action::Help, "help"),
            ],
            Screen::MoveTaskPicker { .. } => vec![
                pair(Action::Open, "move_short"),
                pair(Action::Back, "action_back"),
                pair(Action::Help, "help"),
            ],
            Screen::Language { .. } => vec![
                (arrows(Action::Up, Action::Down), label("choose_short")),
                pair(Action::Apply, "action_apply"),
                pair(Action::Back, "action_back"),
            ],
            Screen::ListPicker => vec![
                pair(Action::Open, "action_open"),
                pair(Action::Back, "action_back"),
                pair(Action::NewList, "new_list"),
                pair(Action::RenameList, "rename_list"),
                pair(
                    Action::DeleteList,
                    if self.picker_identity(self.picker.selection) == Some(RowIdentity::Trash) {
                        "action_empty_trash"
                    } else {
                        "action_delete_list"
                    },
                ),
                (
                    arrows(Action::MoveUp, Action::MoveDown),
                    label("reorder_short"),
                ),
                pair(Action::Help, "help"),
                pair(Action::Quit, "action_quit"),
            ],
            Screen::DeleteListConfirm { .. }
            | Screen::PurgeTaskConfirm { .. }
            | Screen::EmptyTrashConfirm { .. } => vec![
                ("y".into(), label("yes")),
                ("n".into(), label("no")),
                (label("help_arrows"), label("choose_short")),
                (config.display_key("enter"), label("confirm_short")),
                pair(Action::Back, "action_back"),
            ],
            Screen::ConfirmDiscard { .. } => vec![
                ("y".into(), label("yes")),
                ("n".into(), label("no")),
                (label("help_arrows"), label("choose_short")),
                (config.display_key("enter"), label("confirm_short")),
                pair(Action::Discard, "keep_editing"),
            ],
            Screen::Editor(_) => vec![
                pair(Action::Save, "action_save"),
                pair(Action::Discard, "action_discard"),
                pair(Action::NextField, "action_next_field"),
                pair(Action::CopyText, "action_copy_text"),
            ],
            Screen::CopyText { .. } => vec![
                text("copy_hint"),
                (
                    arrows(Action::PageUp, Action::PageDown),
                    label("page_short"),
                ),
                pair(Action::Close, "action_close"),
            ],
            Screen::Filter(_) => vec![
                pair(Action::Apply, "action_apply"),
                pair(Action::Back, "action_discard"),
                pair(Action::CopySelection, "copy_short"),
                text("filter_hint"),
            ],
            Screen::ListName { .. } => vec![
                pair(Action::Apply, "action_apply"),
                pair(Action::Back, "action_discard"),
                pair(Action::CopySelection, "copy_short"),
                text("list_hint"),
            ],
            Screen::Help { .. } => vec![
                (arrows(Action::Up, Action::Down), label("scroll_short")),
                (
                    arrows(Action::PageUp, Action::PageDown),
                    label("page_short"),
                ),
                pair(Action::Close, "action_close"),
            ],
            Screen::Navigation => {
                let mut hints = Vec::new();
                if self.view == View::Trash {
                    hints.push(pair(Action::Restore, "action_restore"));
                    hints.push(pair(Action::Trash, "delete_forever_short"));
                    hints.push(pair(Action::EmptyTrash, "action_empty_trash"));
                    hints.push(pair(Action::CopyText, "copy_short"));
                } else {
                    hints.extend([
                        pair(Action::Open, "edit_short"),
                        pair(Action::ToggleDone, "done_short"),
                        pair(Action::ToggleCancelled, "cancel_short"),
                        pair(Action::Trash, "trash_short"),
                        (
                            format!(
                                "{}/{}/{}/{}",
                                key(Action::DueToday),
                                key(Action::DueTomorrow),
                                key(Action::DueNextWeek),
                                key(Action::ClearDue)
                            ),
                            label("date_short"),
                        ),
                    ]);
                    if matches!(self.view, View::History(_)) {
                        hints.push(pair(Action::Reopen, "action_reopen"));
                    }
                    hints.push(pair(Action::Undo, "undo_short"));
                    hints.push(pair(Action::MoveTask, "move_short"));
                }
                hints.push(pair(Action::Filter, "filter"));
                hints.push(pair(Action::Lists, "lists"));
                if matches!(self.view, View::Planned | View::List(_)) {
                    hints.push(pair(Action::NewTask, "new_task"));
                }
                if matches!(self.view, View::Planned | View::List(_)) {
                    hints.push(pair(Action::History, "history"));
                }
                if !self.filter.is_empty() {
                    hints.push(pair(Action::Back, "clear_filter_short"));
                } else if self.view != View::Planned {
                    hints.push(pair(Action::Back, "action_back"));
                }
                hints.push(pair(Action::Help, "help"));
                hints.push(pair(Action::Quit, "action_quit"));
                hints
            }
        }
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
        self.mouse_targets = draw_list_popup(
            frame,
            area,
            rows,
            &mut self.picker,
            tr(language, "list_picker"),
            language,
        )
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
        self.mouse_targets = draw_list_popup(
            frame,
            area,
            rows,
            picker,
            tr(language, "list_picker"),
            language,
        )
        .into_iter()
        .map(|(rect, index)| (rect, MouseTarget::EditorListRow(index)))
        .collect();
    }

    fn draw_move_task_picker(&mut self, frame: &mut Frame<'_>, area: Rect, language: Language) {
        let Screen::MoveTaskPicker { picker, .. } = &mut self.screen else {
            unreachable!()
        };
        let rows = ordinary_list_rows(&self.lists, language);
        self.mouse_targets = draw_list_popup(
            frame,
            area,
            rows,
            picker,
            tr(language, "move_task_picker"),
            language,
        )
        .into_iter()
        .map(|(rect, index)| (rect, MouseTarget::EditorListRow(index)))
        .collect();
    }

    fn draw_navigation(&mut self, frame: &mut Frame<'_>, area: Rect, config: &Config) {
        let language = config.language;
        let history = matches!(self.view, View::History(_));
        self.page_rows = usize::from(area.height);
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
        let labels: Vec<(String, bool)> = self
            .visible
            .iter()
            .map(|task| {
                let held = self.is_held(task.id);
                if history {
                    (
                        history_group_label(task.status_changed_on, self.today, language),
                        false,
                    )
                } else if held {
                    (
                        open_group_label(task, self.today, language),
                        Bucket::for_task(task, self.today) == Bucket::Overdue,
                    )
                } else {
                    let dated_heading = matches!(
                        planned_group(task, self.today),
                        PlannedGroup::Due(
                            Bucket::Today
                                | Bucket::Tomorrow
                                | Bucket::Soon
                                | Bucket::Next
                                | Bucket::Undated
                        )
                    );
                    (
                        ordinary_group_label(task, self.today, language),
                        trash || !dated_heading,
                    )
                }
            })
            .collect();
        let mut rows = Vec::new();
        let mut row_tasks = Vec::new();
        for (index, task) in self.visible.iter().enumerate() {
            let held = self.is_held(task.id);
            let (group, show_date) = &labels[index];
            let starts_group = index == 0 || labels[index - 1].0 != *group;
            if !trash && starts_group {
                // The heading counts the rows displayed under it, held rows included.
                let count = labels[index..]
                    .iter()
                    .take_while(|(label, _)| label == group)
                    .count();
                rows.push(ListItem::new(Line::from(Span::styled(
                    format!("{group} ({count})"),
                    Style::default().fg(Color::Yellow),
                ))));
                row_tasks.push(None);
            }
            let show_date = *show_date;
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
                .filter(|_| show_date)
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
            let text = format!("{marker} {list_label}{title}{date}");
            let line = if held {
                Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
            } else {
                Line::from(text)
            };
            rows.push(ListItem::new(line));
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
        let mut displayed_tasks = 0;
        for (row, task_index) in row_tasks
            .iter()
            .skip(self.list_state.offset())
            .take(usize::from(area.height))
            .enumerate()
        {
            if let Some(index) = task_index {
                displayed_tasks += 1;
                self.mouse_targets.push((
                    Rect::new(area.x, area.y + row as u16, area.width, 1),
                    MouseTarget::Row(*index),
                ));
            }
        }
        // A page is the number of task rows on screen, not counting headings.
        self.page_rows = displayed_tasks.max(1);
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
    title: &str,
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
    let block = Block::default().borders(Borders::ALL).title(title);
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
    // Description takes the remaining height. The other fields are one line each.
    let rows = Layout::vertical(FIELD_ORDER.map(|index| {
        if index == DESCRIPTION {
            Constraint::Min(3)
        } else {
            Constraint::Length(3)
        }
    }))
    .split(area);
    let mut targets = Vec::with_capacity(FIELD_ORDER.len());
    for (&index, &area) in FIELD_ORDER.iter().zip(rows.iter()) {
        let key = FIELD_KEYS[index];
        let (inner, scroll_row, scroll_column) = if index == DESCRIPTION {
            draw_description(
                frame,
                area,
                tr(language, key),
                &editor.fields[index],
                index == editor.active,
            )
        } else if index == LIST {
            let block = Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title(tr(language, key))
                .border_style(Style::default().fg(if editor.active == LIST {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }));
            let inner = block.inner(area);
            let name = lists
                .iter()
                .find(|list| Some(list.id) == editor.selected_list_id())
                .map(|list| list.name.replace(['\r', '\n'], " "))
                .unwrap_or_else(|| tr(language, "list_unavailable").to_owned());
            frame.render_widget(Paragraph::new(name).block(block), area);
            (inner, 0, 0)
        } else if index == DUE {
            draw_input_placeholder(
                frame,
                area,
                &format!("{}  {}", tr(language, key), tr(language, "due_format_hint")),
                &editor.fields[index],
                index == editor.active,
                Some(tr(language, "undated")),
            )
        } else {
            draw_input(
                frame,
                area,
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
    draw_deletion_dialog(
        frame,
        area,
        "delete_list_confirm",
        paragraphs,
        delete,
        language,
    );
}

fn draw_task_purge(
    frame: &mut Frame<'_>,
    area: Rect,
    task: &Task,
    delete: bool,
    changed: bool,
    language: Language,
) {
    let title = task.title.replace(['\r', '\n'], " ");
    let inner_width = usize::from(deletion_dialog_width(area).saturating_sub(2));
    let mut paragraphs = vec![
        (
            shortened_title(&title, &format!(" #{}", task.id), inner_width),
            Color::Cyan,
        ),
        (tr(language, "cannot_undo").to_owned(), Color::Yellow),
    ];
    if changed {
        paragraphs.push((tr(language, "task_purge_changed").to_owned(), Color::Yellow));
    }
    draw_deletion_dialog(
        frame,
        area,
        "delete_forever_confirm",
        paragraphs,
        delete,
        language,
    );
}

fn draw_empty_trash(
    frame: &mut Frame<'_>,
    area: Rect,
    preview: &TrashPreview,
    delete: bool,
    changed: bool,
    language: Language,
) {
    let mut paragraphs = vec![
        (
            format!("{}: {}", tr(language, "trashed"), preview.count),
            Color::Cyan,
        ),
        (tr(language, "empty_trash_hint").to_owned(), Color::White),
        (tr(language, "cannot_undo").to_owned(), Color::Yellow),
    ];
    if changed {
        paragraphs.push((tr(language, "trash_changed").to_owned(), Color::Yellow));
    }
    draw_deletion_dialog(
        frame,
        area,
        "empty_trash_confirm",
        paragraphs,
        delete,
        language,
    );
}

/// Wrapped lines a task title may use in the Delete forever dialog.
const PURGE_TITLE_LINES: usize = 3;

/// Joins `title` and `suffix`, shortening the title with an ellipsis so the result wraps
/// into at most `PURGE_TITLE_LINES` lines of `width` cells. The suffix is always kept.
fn shortened_title(title: &str, suffix: &str, width: usize) -> String {
    let fits = |text: &str| TextLayout::new(text, width).lines.len() <= PURGE_TITLE_LINES;
    let full = format!("{title}{suffix}");
    if fits(&full) {
        return full;
    }
    let characters: Vec<char> = title.chars().collect();
    let shortened = |count: usize| {
        let kept: String = characters[..count].iter().collect();
        format!("{}…{suffix}", kept.trim_end())
    };
    // Find the longest title prefix that still fits.
    let (mut low, mut high) = (0, characters.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(&shortened(middle)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    shortened(low)
}

fn deletion_dialog_width(area: Rect) -> u16 {
    area.width.saturating_sub(4).min(72)
}

/// Draws a centered No/Yes deletion dialog with wrapped, colored paragraphs.
fn draw_deletion_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    paragraphs: Vec<(String, Color)>,
    delete: bool,
    language: Language,
) {
    let width = deletion_dialog_width(area);
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
                .title(tr(language, title)),
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

/// Help sections in display order, with the opening screen's section first.
fn help_sections(config: &Config, origin: HelpOrigin) -> Vec<(&'static str, Vec<HelpEntry>)> {
    let language = config.language;
    let action = HelpEntry::Action;
    let builtin = |keys: String, label: &'static str| HelpEntry::Builtin(keys, label);
    let keys = |names: &[&str]| {
        names
            .iter()
            .map(|name| config.display_key(name))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let arrows = tr(language, "help_arrows");
    // Enter saves only while no configured editor action claims it.
    let enter_saves = config
        .action(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Mode::Editor,
        )
        .is_none();
    let mut editor_entries = vec![action(Action::Save)];
    if enter_saves {
        editor_entries.push(builtin(keys(&["enter"]), "help_enter_saves"));
    }
    editor_entries.extend([
        action(Action::Discard),
        action(Action::NextField),
        action(Action::PreviousField),
        builtin(
            format!(
                "{} / {}",
                config.primary_key(Action::Open),
                config.primary_key(Action::Lists)
            ),
            "help_list_field",
        ),
        action(Action::DueToday),
        action(Action::DueTomorrow),
        action(Action::DueNextWeek),
        action(Action::ClearDue),
        action(Action::CopySelection),
        action(Action::CopyText),
    ]);
    let mut sections = vec![
        (
            "help_tasks",
            [
                Action::Up,
                Action::Down,
                Action::First,
                Action::Last,
                Action::PageUp,
                Action::PageDown,
                Action::Open,
                Action::NewTask,
                Action::Filter,
                Action::Lists,
                Action::History,
                Action::AllTasks,
                Action::Back,
                Action::ToggleDone,
                Action::ToggleCancelled,
                Action::Trash,
                Action::Restore,
                Action::EmptyTrash,
                Action::Reopen,
                Action::MoveTask,
                Action::DueToday,
                Action::DueTomorrow,
                Action::DueNextWeek,
                Action::ClearDue,
                Action::MoveUp,
                Action::MoveDown,
                Action::Undo,
                Action::CopyText,
                Action::Quit,
            ]
            .into_iter()
            .flat_map(|item| {
                // In Trash, the trash keys delete the selected task forever.
                let extra = (item == Action::Trash)
                    .then(|| builtin(config.keys_for(Action::Trash), "help_delete_forever"));
                std::iter::once(action(item)).chain(extra)
            })
            .collect(),
        ),
        (
            "help_lists",
            [
                Action::Open,
                Action::Back,
                Action::NewList,
                Action::RenameList,
                Action::DeleteList,
                Action::MoveUp,
                Action::MoveDown,
                Action::Quit,
            ]
            .into_iter()
            .flat_map(|item| {
                // On the Trash entry, the delete_list keys empty Trash.
                let extra = (item == Action::DeleteList).then(|| {
                    builtin(
                        config.keys_for(Action::DeleteList),
                        "help_empty_trash_entry",
                    )
                });
                std::iter::once(action(item)).chain(extra)
            })
            .collect(),
        ),
        ("help_editor", editor_entries),
        (
            "help_text",
            vec![
                builtin(keys(&["ctrl+left", "ctrl+right"]), "help_word_move"),
                builtin(keys(&["ctrl+backspace", "ctrl+delete"]), "help_word_delete"),
                builtin(keys(&["home", "end"]), "help_line_edges"),
                builtin(keys(&["ctrl+home", "ctrl+end"]), "help_field_edges"),
                builtin(format!("Shift+{arrows}"), "help_select"),
                builtin(keys(&["ctrl+a"]), "help_select_all"),
                builtin(keys(&["ctrl+z"]), "help_undo_text"),
                builtin(keys(&["ctrl+y", "ctrl+shift+z"]), "help_redo_text"),
                builtin(keys(&["enter"]), "help_newline"),
            ],
        ),
        (
            "help_forms",
            vec![
                action(Action::Apply),
                action(Action::Back),
                action(Action::CopySelection),
            ],
        ),
        (
            "help_confirm",
            vec![
                builtin(
                    format!("{arrows} / {}", keys(&["tab"])),
                    "help_choose_option",
                ),
                builtin(keys(&["enter"]), "help_confirm_choice"),
                builtin("y / n".into(), "help_yes_no"),
                builtin(keys(&["esc"]), "help_cancel"),
            ],
        ),
        (
            "help_everywhere",
            [
                Action::Help,
                Action::Close,
                Action::Language,
                Action::Refresh,
                Action::ToggleMouse,
                Action::ScrollLeft,
                Action::ScrollRight,
            ]
            .into_iter()
            .map(action)
            .collect(),
        ),
    ];
    let first = match origin {
        HelpOrigin::Tasks => 0,
        HelpOrigin::Lists => 1,
        HelpOrigin::Editor => 2,
    };
    let section = sections.remove(first);
    sections.insert(0, section);
    sections
}

fn help_lines(config: &Config, origin: HelpOrigin) -> Vec<Line<'static>> {
    let language = config.language;
    let mut lines = Vec::new();
    for (index, (title, entries)) in help_sections(config, origin).into_iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            tr(language, title).to_owned(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        for entry in entries {
            let (keys, label) = match entry {
                HelpEntry::Action(action) => (
                    config.keys_for(action),
                    tr(language, &format!("action_{}", action.name())).to_owned(),
                ),
                HelpEntry::Builtin(keys, label) => (keys, tr(language, label).to_owned()),
            };
            lines.push(Line::from(format!("{keys:<30} {label}")));
        }
    }
    lines
}

fn draw_help(
    frame: &mut Frame<'_>,
    area: Rect,
    config: &Config,
    lines: Vec<Line<'static>>,
    scroll: usize,
) {
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(tr(config.language, "help")))
            .scroll((scroll.min(u16::MAX as usize) as u16, 0)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::Bucket;
    use crate::query::parse_date;
    use crate::sync::remote::testing::MemoryRemote;
    use crate::sync::worker::Cadence;
    use crate::sync::{FailureKind, sync_once};
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
        editor.fields[DESCRIPTION].set_cursor(5, false);
        editor.fields[DESCRIPTION].set_cursor(value.description.len() - 4, true);
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
        editor.fields[TITLE] = TextBuffer::new("Changed title");
        editor.fields[DESCRIPTION] = TextBuffer::new("Unsaved description\nSecond line");
        editor.fields[DUE] = TextBuffer::new("not-a-date");
        editor.active = DUE;
        editor.fields[DUE].cursor = 3;
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
        // Title, List, Planned date, Description.
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.paste("2026-02-30");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.paste("To kartoner\nHavremælk");
        app.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(store.tasks().unwrap().is_empty());
        if let Screen::Editor(editor) = &mut app.screen {
            editor.fields[DUE] = TextBuffer::new("2026-11-09");
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
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "qlpd")
        );
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
        editor.fields[DUE] = TextBuffer::default();
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
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.active == DESCRIPTION));
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
        assert!(first.contains(tr(config.language, "help_tasks")));
        assert!(first.contains(tr(config.language, "action_up")));
        assert!(first.contains("Up / k"));
        for _ in 0..200 {
            app.handle_key(key(KeyCode::Down), &mut store, &config);
        }
        let last = rendered(&mut app, &config, 80, 12);
        assert!(last.contains(tr(config.language, "action_scroll_right")));
        assert_eq!(app.help_scroll, app.help_rows - app.help_page);
        app.handle_key(key(KeyCode::Home), &mut store, &config);
        assert_eq!(app.help_scroll, 0);
        app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        assert_eq!(app.help_scroll, app.help_page);
        app.handle_key(key(KeyCode::End), &mut store, &config);
        assert_eq!(app.help_scroll, app.help_rows - app.help_page);
        app.handle_key(key(KeyCode::PageUp), &mut store, &config);
        assert_eq!(app.help_scroll, app.help_rows - 2 * app.help_page);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        app.open_picker();
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        let picker_help = rendered(&mut app, &config, 80, 40);
        let lists_at = picker_help.find(tr(config.language, "help_lists")).unwrap();
        let tasks_at = picker_help.find(tr(config.language, "help_tasks")).unwrap();
        assert!(lists_at < tasks_at);
        assert!(picker_help.contains("n / N"));
        let text_help = rendered(&mut app, &config, 80, 120);
        assert!(text_help.contains(tr(config.language, "help_select_all")));
        assert!(text_help.contains("Ctrl+A"));
        assert!(text_help.contains("Ctrl+Shift+Z"));
        assert!(text_help.contains("Alt+Up / K"));
        assert!(text_help.contains("q / Ctrl+C"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
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

    fn shift(character: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(character), KeyModifiers::SHIFT)
    }

    fn message(app: &App) -> &str {
        app.message
            .as_ref()
            .map_or("", |message| message.text.as_str())
    }

    #[test]
    fn delete_forever_defaults_to_no_and_clears_undo() {
        let (_directory, mut store, config) = fixture();
        let kept = store.create_task(&input(1, "Kept")).unwrap();
        for title in ["First", "Second"] {
            let task = store.create_task(&input(1, title)).unwrap();
            store.set_deleted(task.id, task.version, true).unwrap();
        }
        let first = store.tasks().unwrap()[1].clone();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(store.task(kept.id).unwrap().deleted);
        assert!(app.undo.is_some());
        app.set_view(View::Trash, &store).unwrap();
        app.select_task_id(Some(first.id));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(matches!(
            app.screen,
            Screen::PurgeTaskConfirm { delete: false, .. }
        ));
        let display = rendered(&mut app, &config, 90, 24);
        for text in [
            "Delete this task forever?",
            &format!("First #{}", first.id),
            "This cannot be undone.",
            "y Yes",
        ] {
            assert!(display.contains(text), "Missing {text}");
        }
        // Enter on the default No, Esc, and n all keep the task.
        for keys in [
            vec![key(KeyCode::Enter)],
            vec![key(KeyCode::Esc)],
            vec![key(KeyCode::Tab), key(KeyCode::Char('n'))],
        ] {
            if !matches!(app.screen, Screen::PurgeTaskConfirm { .. }) {
                app.handle_key(key(KeyCode::Delete), &mut store, &config);
            }
            app.paste("Yes");
            for event in keys {
                app.handle_key(event, &mut store, &config);
            }
            assert!(matches!(app.screen, Screen::Navigation));
            assert_eq!(store.task(first.id).unwrap(), first);
            assert_eq!(app.selected_task().unwrap().id, first.id);
        }
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        app.handle_key(key(KeyCode::Right), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert!(store.task(first.id).is_err());
        assert!(app.visible.iter().all(|task| task.id != first.id));
        assert_eq!(message(&app), "Deleted forever");
        assert!(app.undo.is_none());
        app.set_view(View::Planned, &store).unwrap();
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(message(&app), "Nothing to undo");
        assert!(store.task(kept.id).unwrap().deleted);
    }

    #[test]
    fn delete_forever_reconfirms_external_edits_and_closes_after_restore() {
        let (directory, mut store, config) = fixture();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let task = store.set_deleted(task.id, task.version, true).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::Trash, &store).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        let mut renamed = TaskInput::from(&task);
        renamed.title = "Renamed elsewhere".into();
        let renamed = external
            .update_task(task.id, task.version, &renamed)
            .unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(matches!(
            &app.screen,
            Screen::PurgeTaskConfirm { task, delete: false, changed: true }
                if task.title == "Renamed elsewhere"
        ));
        let display = rendered(&mut app, &config, 90, 24);
        assert!(display.contains("The task changed. Review it and confirm again."));
        // A change between refreshes is caught by the store when confirming.
        let mut edited = TaskInput::from(&renamed);
        edited.description = "Edited again".into();
        let edited = external
            .update_task(task.id, renamed.version, &edited)
            .unwrap();
        app.handle_key(key(KeyCode::Char('y')), &mut store, &config);
        assert!(matches!(
            &app.screen,
            Screen::PurgeTaskConfirm { task, delete: false, changed: true }
                if task.version == edited.version
        ));
        assert_eq!(store.task(task.id).unwrap(), edited);
        let restored = external
            .set_deleted(task.id, edited.version, false)
            .unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(message(&app), "This task is no longer in Trash.");
        assert_eq!(store.task(task.id).unwrap(), restored);
        // A restore that the terminal has not seen yet also deletes nothing.
        let trashed = store.set_deleted(task.id, restored.version, true).unwrap();
        app.refresh(&store).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        let restored = external
            .set_deleted(task.id, trashed.version, false)
            .unwrap();
        app.handle_key(key(KeyCode::Char('y')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(message(&app), "This task is no longer in Trash.");
        assert_eq!(store.task(task.id).unwrap(), restored);
    }

    #[test]
    fn empty_trash_counts_hidden_tasks_and_reconfirms_a_changed_trash() {
        let (directory, mut store, config) = fixture();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let open = store.create_task(&input(1, "Open")).unwrap();
        for title in ["Milk one", "Milk two", "Bread"] {
            let task = store.create_task(&input(1, title)).unwrap();
            store.set_deleted(task.id, task.version, true).unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(shift('D'), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.tasks().unwrap().len(), 4);
        app.set_view(View::Trash, &store).unwrap();
        app.filter = "milk".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(app.visible.len(), 2);
        app.handle_key(shift('D'), &mut store, &config);
        assert!(matches!(
            app.screen,
            Screen::EmptyTrashConfirm {
                delete: false,
                from_picker: false,
                ..
            }
        ));
        let display = rendered(&mut app, &config, 90, 24);
        for text in [
            "Empty Trash?",
            "In Trash: 3",
            "including tasks the filter hides",
            "This cannot be undone.",
        ] {
            assert!(display.contains(text), "Missing {text}");
        }
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(app.filter, "milk");
        assert_eq!(store.tasks().unwrap().len(), 4);
        app.handle_key(shift('D'), &mut store, &config);
        app.handle_key(key(KeyCode::Left), &mut store, &config);
        external.set_deleted(open.id, open.version, true).unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(matches!(
            &app.screen,
            Screen::EmptyTrashConfirm { preview, delete: false, changed: true, .. }
                if preview.count == 4
        ));
        let display = rendered(&mut app, &config, 90, 24);
        assert!(display.contains("Trash changed. Review the count and confirm again."));
        app.handle_key(key(KeyCode::Char('y')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert!(store.tasks().unwrap().is_empty());
        assert!(app.visible.is_empty());
        assert_eq!(message(&app), "Trash emptied");
        app.handle_key(shift('D'), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(message(&app), "Trash is empty");
    }

    #[test]
    fn empty_trash_from_the_picker_returns_to_the_trash_entry() {
        let (directory, mut store, config) = fixture();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        for title in ["One", "Two"] {
            let task = store.create_task(&input(1, title)).unwrap();
            store.set_deleted(task.id, task.version, true).unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        app.select_picker_identity(RowIdentity::Trash);
        assert!(rendered(&mut app, &config, 100, 24).contains("d Empty Trash"));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::Trash)
        );
        assert!(store.tasks().unwrap().is_empty());
        assert_eq!(message(&app), "Trash emptied");
        assert!(rendered(&mut app, &config, 100, 24).contains("Trash (0)"));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(message(&app), "Trash is empty");
        // Trash emptied elsewhere closes the open confirmation.
        let task = store.create_task(&input(1, "Three")).unwrap();
        store.set_deleted(task.id, task.version, true).unwrap();
        app.refresh(&store).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(matches!(app.screen, Screen::EmptyTrashConfirm { .. }));
        let preview = external.preview_empty_trash().unwrap();
        external.empty_trash(&preview).unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(message(&app), "Trash is empty");
        assert!(app.external_update);
        assert!(rendered(&mut app, &config, 100, 24).contains("Trash is empty"));
    }

    #[test]
    fn delete_forever_shortens_a_long_title_and_keeps_the_id() {
        let (_directory, mut store, config) = fixture();
        let title = "Very long title ".repeat(70);
        let task = store.create_task(&input(1, &title)).unwrap();
        store.set_deleted(task.id, task.version, true).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::Trash, &store).unwrap();
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        for (width, height) in [(80, 24), (38, 18)] {
            let display = rendered(&mut app, &config, width, height);
            let id = format!("#{}", task.id);
            for text in ["Delete", "…", id.as_str(), "This cannot be undone.", "Yes"] {
                assert!(display.contains(text), "Missing {text} at {width}x{height}");
            }
            assert!(!display.contains(tr(config.language, "terminal_too_small")));
        }
        assert_eq!(shortened_title("Buy milk", " #7", 40), "Buy milk #7");
        let long = shortened_title(&"word ".repeat(50), " #7", 20);
        assert!(long.ends_with("… #7"));
        assert!(TextLayout::new(&long, 20).lines.len() <= PURGE_TITLE_LINES);
    }

    #[test]
    fn help_lists_delete_forever_and_empty_trash() {
        let (_directory, store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::Trash, &store).unwrap();
        app.open_help();
        let help = rendered(&mut app, &config, 100, 80);
        assert!(help.contains("Delete forever in Trash"));
        assert!(help.contains("Empty Trash from the Trash entry"));
        let empty_trash = help_lines(&config, HelpOrigin::Tasks)
            .iter()
            .map(|line| line.to_string())
            .find(|line| line.ends_with(" Empty Trash"))
            .unwrap();
        assert!(empty_trash.starts_with("D "));
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == "Unsaved description")
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
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "Keep this draft")
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
        editor.fields[DESCRIPTION].set_cursor(4, false);
        editor.fields[DESCRIPTION].set_cursor(80, true);
        for width in [38, 80] {
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal.draw(|frame| app.draw(frame, &config)).unwrap();
            let (rect, MouseTarget::EditorField { scroll_row, .. }) = app
                .mouse_targets
                .iter()
                .find(|(_, target)| {
                    matches!(
                        target,
                        MouseTarget::EditorField {
                            index: DESCRIPTION,
                            ..
                        }
                    )
                })
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
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == value.description && editor.fields[DESCRIPTION].selection() == Some(4..80))
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "Replacement title" && editor.fields[DESCRIPTION].text.is_empty())
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "Initial title" && editor.fields[TITLE].selection() == Some(0..13))
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "Replacement title")
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
            .find(|(_, target)| {
                matches!(
                    target,
                    MouseTarget::EditorField {
                        index: DESCRIPTION,
                        ..
                    }
                )
            })
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
            matches!(&app.screen, Screen::Editor(editor) if editor.active == DESCRIPTION && editor.fields[DESCRIPTION].text == "Æble\n界!x")
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == "First line\nSecond line")
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == "Unsaved local text" && editor.original.as_ref().unwrap().version == task.version)
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
    fn help_wheel_scrolls_and_clicks_change_nothing() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        let first = rendered(&mut app, &config, 80, 12);
        assert!(app.mouse_targets.is_empty());
        let now = Instant::now();
        app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, 5, 5),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.help_scroll, 1);
        let scrolled = rendered(&mut app, &config, 80, 12);
        assert_ne!(first, scrolled);
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), 5, 5),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.help_scroll, 1);
        assert!(matches!(app.screen, Screen::Help { .. }));
        app.handle_mouse(
            mouse(MouseEventKind::ScrollUp, 5, 5),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        app.handle_mouse(
            mouse(MouseEventKind::ScrollUp, 5, 5),
            now,
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.help_scroll, 0);
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
            editor.fields[DESCRIPTION].cursor = 41;
        }
        let screen = rendered(&mut app, &config, 38, 24);
        assert!(screen.contains("five six seven"));
        app.handle_key(key(KeyCode::Up), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == 5)
        );
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == 41)
        );
        let (rect, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| {
                matches!(
                    target,
                    MouseTarget::EditorField {
                        index: DESCRIPTION,
                        ..
                    }
                )
            })
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
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == 43)
        );
        app.paste("!");
        let expected = description.replacen("six", "si!x", 1);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == expected)
        );
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
            .find(|(_, target)| {
                matches!(
                    target,
                    MouseTarget::EditorField {
                        index: DESCRIPTION,
                        ..
                    }
                )
            })
            .copied()
            .unwrap()
        else {
            panic!()
        };
        assert!(scroll_row > 0);
        app.handle_key(key(KeyCode::Left), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == description.len() - "👨‍👩‍👧‍👦".len())
        );
        app.handle_key(key(KeyCode::Left), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == description.len() - "e\u{301}👨‍👩‍👧‍👦".len())
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
        assert!(rendered(&mut app, &config, 100, 24).contains("Ctrl+L"));
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        let control = |letter| KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL);
        app.handle_key(control('l'), &mut store, &config);
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(control('o'), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        assert!(rendered(&mut app, &config, 100, 24).contains("Ctrl+B"));
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
                } else if route == 0 {
                    app.handle_key(key(KeyCode::Esc), &mut store, &config);
                    assert_eq!(app.view, View::History(list_view.scope().0));
                    assert!(app.filter.is_empty());
                    app.handle_key(key(KeyCode::Esc), &mut store, &config);
                } else {
                    app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
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
        for identity in [RowIdentity::AllTasks, RowIdentity::List(1)] {
            app.select_picker_identity(identity);
            app.handle_key(key(KeyCode::Delete), &mut store, &config);
            assert!(matches!(app.screen, Screen::ListPicker));
        }
        // Delete on Trash offers to empty it instead of deleting a list.
        app.select_picker_identity(RowIdentity::Trash);
        app.handle_key(key(KeyCode::Delete), &mut store, &config);
        assert!(matches!(
            app.screen,
            Screen::EmptyTrashConfirm {
                delete: false,
                from_picker: true,
                ..
            }
        ));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(store.tasks().unwrap(), original);
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == "Unsaved description" && editor.original.as_ref().unwrap().version == task.version)
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
        assert!(rendered(&mut app, &config, 100, 24).contains("Ctrl+H Historik"));
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        let display = rendered(&mut app, &config, 100, 24);
        assert!(display.contains("Alle opgaver > Historik"));
        assert!(display.contains("Ctrl+R"));
        app.open_picker();
        let display = rendered(&mut app, &config, 100, 24);
        assert!(display.contains("[standard]"));
        assert!(display.contains("Ctrl+D"));
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
        assert_eq!(config.keys_for(Action::Quit), "Ctrl+Q");
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
        // An existing task opens at Description. Planned date is just above it.
        app.handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert_eq!(app.mode(), Mode::DueField);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[DUE].insert("3");
        editor.fields[DUE].set_cursor(2, false);
        editor.fields[DUE].set_cursor(5, true);
        let old_text = editor.fields[DUE].text.clone();
        let old_cursor = editor.fields[DUE].cursor;
        let old_selection = editor.fields[DUE].selection();
        let title = editor.fields[TITLE].clone();
        let description = editor.fields[DESCRIPTION].clone();
        let list = editor.fields[LIST].clone();
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.fields[DUE].text, "2026-11-08");
        assert_eq!(editor.fields[DUE].cursor, 10);
        assert!(editor.fields[DUE].selection().is_none());
        assert_eq!(editor.fields[TITLE], title);
        assert_eq!(editor.fields[DESCRIPTION], description);
        assert_eq!(editor.fields[LIST], list);
        assert_eq!(editor.active, DUE);
        assert_eq!(store.task(task.id).unwrap(), task);
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[DUE].set_cursor(2, false);
        editor.fields[DUE].set_cursor(5, true);
        let unchanged = editor.fields[DUE].clone();
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE] == unchanged));
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
        assert_eq!(editor.fields[DUE].text, old_text);
        assert_eq!(editor.fields[DUE].cursor, old_cursor);
        assert_eq!(editor.fields[DUE].selection(), old_selection);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "2026-11-12")
        );
        for _ in 0..2 {
            app.handle_key(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
        }
        app.handle_key(key(KeyCode::Char('x')), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text.is_empty())
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "2026-11-08")
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
        editor.active = DUE;
        let display = rendered(&mut app, &config, 130, 24);
        for text in [
            "Ctrl+T",
            "Ctrl+U",
            "Ctrl+W",
            "Ctrl+X",
            "I dag",
            "I morgen",
            tr(Language::Da, "next_monday"),
            tr(Language::Da, "clear_due"),
        ] {
            assert!(display.contains(text), "Missing {text}");
        }
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "t"));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "2026-11-08")
        );
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        assert_eq!(app.mode(), Mode::Editor);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.active == DESCRIPTION));
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
        editor.active = DUE;
        editor.fields[DESCRIPTION].insert("Keep unsaved text");
        let next_day = tomorrow.succ_opt().unwrap();
        app.handle_key_on_date(key(KeyCode::Char('t')), &mut store, &config, next_day);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == next_day.to_string() && editor.fields[DESCRIPTION].text == "Keep unsaved text" && editor.original.as_ref().unwrap().version == open.version)
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
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "Review")
        );
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
        app.handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        let display = rendered(&mut app, &config, 80, 24);
        for format in ["YYYY-MM-DD", "YYYYMMDD", "YYMMDD", "MMDD"] {
            assert!(display.contains(format));
        }
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text.is_empty() && !editor.is_dirty())
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
            editor.active = DUE;
            editor.fields[DUE] = TextBuffer::default();
            for character in raw.chars() {
                app.handle_key(key(KeyCode::Char(character)), &mut store, &config);
            }
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == raw)
            );
            if exit == 2 {
                rendered(&mut app, &config, 80, 24);
                let (rect, _) = app
                    .mouse_targets
                    .iter()
                    .find(|(_, target)| {
                        matches!(target, MouseTarget::EditorField { index: TITLE, .. })
                    })
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
            assert_eq!(editor.fields[DUE].text, "2026-11-09");
            editor.active = DUE;
            app.handle_key(
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == raw)
            );
            app.handle_key(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
                &mut store,
                &config,
            );
            let Screen::Editor(editor) = &app.screen else {
                panic!()
            };
            let normalized = editor.fields[DUE].clone();
            app.handle_key(key(KeyCode::Tab), &mut store, &config);
            assert!(
                matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE] == normalized)
            );
            assert_eq!(store.task(task.id).unwrap(), task);
        }
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = DUE;
        editor.fields[DUE].replace_all("2026-");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "2026-" && editor.active == DESCRIPTION)
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
        editor.active = DUE;
        editor.fields[DUE].insert("0102");
        app.handle_key_on_date(key(KeyCode::Tab), &mut store, &config, january);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "2027-01-02")
        );
        app.today = december;
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = DUE;
        editor.fields[DUE].replace_all("0103");
        rendered(&mut app, &config, 80, 24);
        let (rect, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: TITLE, .. }))
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "2027-01-03")
        );
        app.today = december;
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.active = DUE;
        editor.fields[DUE].replace_all("0104");
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields == before.fields && editor.active == LIST)
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
        assert_eq!(editor.active, LIST);
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
        editor.active = LIST;
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
            editor.active = LIST;
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
            editor.active = DUE;
            editor.fields[DUE].insert("1109");
        }
        rendered(&mut app, &config, 80, 24);
        let (field, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index: LIST, .. }))
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
            matches!(&app.screen, Screen::EditorListPicker { editor, .. } if editor.active == LIST && editor.fields[DUE].text == "2026-11-09")
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
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(expected) && editor.active == LIST)
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
            editor.active = LIST;
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
        assert!(display.contains("Ctrl+O / Ctrl+L"));
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
            format!("Today - {} (1)", date_label(today(), Language::En)),
            format!(
                "Tomorrow - {} (1)",
                date_label(today().succ_opt().unwrap(), Language::En)
            ),
        ];
        for (days, count) in [(2, 2), (3, 1), (30, 1)] {
            expected.push(format!(
                "{} ({count})",
                date_label(today() + chrono::Duration::days(days), Language::En)
            ));
        }
        expected.push(format!(
            "Done today - {} (1)",
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
    fn keyboard_wrap_and_jumps_follow_filtered_task_rows_without_writing() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        for category in ["open", "history", "trash"] {
            for index in 0..8 {
                let prefix = if index % 4 == 3 { "Skip" } else { "Keep" };
                let mut value = input(work.id, &format!("{prefix} {category} {index}"));
                value.due = Some(today() + chrono::Duration::days(index % 3 - 1));
                let task = store.create_task(&value).unwrap();
                if category == "history" {
                    store
                        .set_status_on(
                            task.id,
                            task.version,
                            Status::Done,
                            today() - chrono::Duration::days(index % 3 + 1),
                        )
                        .unwrap();
                } else if category == "trash" {
                    store.set_deleted(task.id, task.version, true).unwrap();
                }
            }
        }
        store.create_task(&input(1, "Keep another list")).unwrap();
        let baseline = store.tasks().unwrap();
        let mut readonly =
            Store::open_for_mcp(&directory.path().join("tasks.db"), None, true).unwrap();
        let mut app = App::new(&readonly, today()).unwrap();
        for view in [
            View::Planned,
            View::List(work.id),
            View::Trash,
            View::History(None),
            View::History(Some(work.id)),
        ] {
            match view {
                View::History(scope) => {
                    app.set_view(scope.map_or(View::Planned, View::List), &readonly)
                        .unwrap();
                    app.open_history(&readonly).unwrap();
                }
                _ => app.set_view(view, &readonly).unwrap(),
            }
            app.filter = "keep".into();
            app.query = Query::parse(&app.filter).unwrap();
            app.refresh(&readonly).unwrap();
            app.selection = 0;
            let ids = app.visible.iter().map(|task| task.id).collect::<Vec<_>>();
            assert!(ids.len() >= 6);
            for (code, expected) in [
                (KeyCode::Up, ids.len() - 1),
                (KeyCode::Down, 0),
                (KeyCode::Char('k'), ids.len() - 1),
                (KeyCode::Char('j'), 0),
                (KeyCode::End, ids.len() - 1),
                (KeyCode::Char('g'), 0),
                (KeyCode::Char('G'), ids.len() - 1),
                (KeyCode::Home, 0),
                (KeyCode::Right, ids.len() - 1),
                (KeyCode::Left, 0),
            ] {
                app.handle_key(key(code), &mut readonly, &config);
                assert_eq!(
                    app.selected_task().unwrap().id,
                    ids[expected],
                    "{view:?} {code:?}"
                );
                rendered(&mut app, &config, 90, 10);
                assert!(app.mouse_targets.iter().any(
                    |(_, target)| matches!(target, MouseTarget::Row(index) if *index == expected)
                ));
                if expected == 0 {
                    assert!(app.list_state.offset() <= 1);
                } else {
                    assert!(app.list_state.offset() > 0);
                }
            }
            app.handle_mouse(
                mouse(MouseEventKind::ScrollUp, 0, 4),
                Instant::now(),
                &mut readonly,
                &config,
            )
            .unwrap();
            assert_eq!(app.selection, 0);
            app.handle_key(key(KeyCode::End), &mut readonly, &config);
            app.handle_mouse(
                mouse(MouseEventKind::ScrollDown, 0, 4),
                Instant::now(),
                &mut readonly,
                &config,
            )
            .unwrap();
            assert_eq!(app.selection, ids.len() - 1);
        }
        app.set_view(View::Planned, &readonly).unwrap();
        for (filter, count) in [("No matching task", 0), ("\"Keep open 0\"", 1)] {
            app.filter = filter.into();
            app.query = Query::parse(filter).unwrap();
            app.refresh(&readonly).unwrap();
            for code in [
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Home,
                KeyCode::End,
                KeyCode::Char('g'),
                KeyCode::Char('G'),
            ] {
                app.handle_key(key(code), &mut readonly, &config);
                assert_eq!(app.selection, 0);
                assert_eq!(app.visible.len(), count);
            }
            rendered(&mut app, &config, 90, 10);
            assert_eq!(app.list_state.selected().is_some(), count == 1);
        }
        assert_eq!(readonly.tasks().unwrap(), baseline);
        app.filter = "Keep open".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&readonly).unwrap();
        app.handle_key(key(KeyCode::End), &mut readonly, &config);
        let removed = app.selected_task().unwrap().clone();
        let count = app.visible.len();
        store
            .set_deleted(removed.id, removed.version, true)
            .unwrap();
        let changed = store.tasks().unwrap();
        app.refresh(&readonly).unwrap();
        assert_eq!(app.visible.len(), count - 1);
        assert_eq!(app.selection, count - 2);
        assert_eq!(readonly.tasks().unwrap(), changed);
    }

    #[test]
    fn navigation_picker_wraps_virtual_endpoints_and_cancel_preserves_task_state() {
        let (_directory, mut store, config) = fixture();
        for index in 0..25 {
            store.create_list(&format!("List {index}")).unwrap();
            store
                .create_task(&input(1, &format!("Keep task {index}")))
                .unwrap();
        }
        let baseline = store.tasks().unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.filter = "keep".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.selection = 22;
        rendered(&mut app, &config, 90, 12);
        let task = app.selected_task().unwrap().id;
        let scroll = app.list_state;
        app.open_picker();
        for (code, identity) in [
            (KeyCode::Up, RowIdentity::Trash),
            (KeyCode::Down, RowIdentity::AllTasks),
            (KeyCode::Char('k'), RowIdentity::Trash),
            (KeyCode::Char('j'), RowIdentity::AllTasks),
            (KeyCode::End, RowIdentity::Trash),
            (KeyCode::Home, RowIdentity::AllTasks),
            (KeyCode::Char('G'), RowIdentity::Trash),
            (KeyCode::Char('g'), RowIdentity::AllTasks),
            (KeyCode::Right, RowIdentity::Trash),
            (KeyCode::Left, RowIdentity::AllTasks),
        ] {
            app.handle_key(key(code), &mut store, &config);
            assert_eq!(app.picker_identity(app.picker.selection), Some(identity));
            rendered(&mut app, &config, 90, 12);
            assert!(app.mouse_targets.iter().any(|(_, target)| matches!(target, MouseTarget::PickerRow(index) if *index == app.picker.selection)));
            assert_eq!(app.list_state, scroll);
        }
        let area = app.picker.area.unwrap();
        app.handle_mouse(
            mouse(MouseEventKind::ScrollUp, area.x, area.y),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(app.picker.selection, 0);
        app.handle_key(key(KeyCode::End), &mut store, &config);
        app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, area.x, area.y),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::Trash)
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, task);
        assert_eq!(app.filter, "keep");
        assert_eq!(app.list_state, scroll);
        assert_eq!(store.tasks().unwrap(), baseline);
    }

    #[test]
    fn editor_picker_wraps_ordinary_rows_and_handles_singleton_and_empty_choices() {
        let (directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = LIST;
        }
        app.open_editor_list_picker(&store).unwrap();
        for code in [KeyCode::Up, KeyCode::Down, KeyCode::Home, KeyCode::End] {
            app.handle_key(key(code), &mut store, &config);
            assert_eq!(app.screen.editor_picker().unwrap().selection, 0);
        }
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        for index in 0..25 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        let Screen::Editor(editor) = &mut app.screen else {
            panic!()
        };
        editor.fields[DESCRIPTION].insert("Unsaved text");
        let draft = editor.clone();
        app.open_editor_list_picker(&store).unwrap();
        let last = app.lists.len() - 1;
        for (code, expected) in [
            (KeyCode::Up, last),
            (KeyCode::Down, 0),
            (KeyCode::Char('G'), last),
            (KeyCode::Char('g'), 0),
            (KeyCode::End, last),
            (KeyCode::Home, 0),
            (KeyCode::Right, last),
            (KeyCode::Left, 0),
        ] {
            app.handle_key(key(code), &mut store, &config);
            assert_eq!(app.screen.editor_picker().unwrap().selection, expected);
            rendered(&mut app, &config, 90, 18);
            assert!(app.mouse_targets.iter().any(|(_, target)| matches!(target, MouseTarget::EditorListRow(index) if *index == expected)));
            assert!(
                matches!(&app.screen, Screen::EditorListPicker { editor, .. } if editor.fields == draft.fields && editor.original == draft.original)
            );
        }
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields == draft.fields));
        assert_eq!(store.task(task.id).unwrap(), task);

        let work = store.create_list("Removed scope").unwrap();
        let scoped_task = store.create_task(&input(work.id, "Scoped task")).unwrap();
        let mut scoped = Store::open_for_mcp(
            &directory.path().join("tasks.db"),
            Some("Removed scope"),
            false,
        )
        .unwrap();
        let mut empty = App::new(&scoped, today()).unwrap();
        empty.open_selection(&scoped).unwrap();
        if let Screen::Editor(editor) = &mut empty.screen {
            editor.active = LIST;
        }
        let preview = store.preview_list_deletion(work.id).unwrap();
        store.delete_list(&preview).unwrap();
        empty.open_editor_list_picker(&scoped).unwrap();
        for code in [KeyCode::Up, KeyCode::Down, KeyCode::Home, KeyCode::End] {
            empty.handle_key(key(code), &mut scoped, &config);
            let picker = empty.screen.editor_picker().unwrap();
            assert_eq!(picker.selection, 0);
            assert!(picker.state.selected().is_none());
        }
        assert!(rendered(&mut empty, &config, 90, 18).contains(tr(config.language, "no_lists")));
        empty.handle_key(key(KeyCode::Esc), &mut scoped, &config);
        assert!(
            matches!(&empty.screen, Screen::Editor(editor) if editor.original.as_ref().unwrap() == &scoped_task && editor.selected_list_id() == Some(work.id))
        );
    }

    #[test]
    fn language_wrap_and_jumps_do_not_apply_until_confirmed() {
        let (directory, mut store, mut config) = fixture();
        let task = store.create_task(&input(1, "Original")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_selection(&store).unwrap();
        app.paste("Unsaved description");
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        let draft = editor.clone();
        app.handle_key(key(KeyCode::F(3)), &mut store, &config);
        for (code, expected) in [
            (KeyCode::Up, Language::Da),
            (KeyCode::Up, Language::En),
            (KeyCode::Down, Language::Da),
            (KeyCode::Down, Language::En),
            (KeyCode::Char('k'), Language::Da),
            (KeyCode::Char('j'), Language::En),
            (KeyCode::End, Language::Da),
            (KeyCode::Home, Language::En),
            (KeyCode::Char('G'), Language::Da),
            (KeyCode::Char('g'), Language::En),
            (KeyCode::Right, Language::Da),
            (KeyCode::Left, Language::En),
        ] {
            app.handle_key(key(code), &mut store, &config);
            assert!(
                matches!(&app.screen, Screen::Language { selected, .. } if *selected == expected)
            );
            assert!(app.pending_language.is_none());
            assert_eq!(config.language, Language::En);
        }
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.apply_language_change(&mut config, &directory.path().join("config.toml"))
            .unwrap();
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields == draft.fields && editor.original == draft.original)
        );
        assert!(!directory.path().join("config.toml").exists());
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn jumps_respect_configured_keys_and_leave_text_and_scrolling_controls_bounded() {
        let (_directory, mut store, _) = fixture();
        let config =
            Config::from_toml("[keys]\nfirst = ['ctrl+home']\nlast = ['ctrl+end']").unwrap();
        let first = store.create_task(&input(1, "First")).unwrap();
        let last = store.create_task(&input(1, "Last")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::End), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, first.id);
        app.handle_key(
            KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(app.selected_task().unwrap().id, last.id);
        app.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(app.selected_task().unwrap().id, first.id);
        let unchanged = store.tasks().unwrap();
        app.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::ALT),
            &mut store,
            &config,
        );
        assert_eq!(app.selection, 0);
        assert_eq!(store.tasks().unwrap(), unchanged);
        app.open_selection(&store).unwrap();
        app.paste("gG\nsecond line");
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = TITLE;
        }
        let defaults = Config::default();
        app.handle_key(key(KeyCode::Char('g')), &mut store, &defaults);
        app.handle_key(key(KeyCode::Char('G')), &mut store, &defaults);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "FirstgG")
        );
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = DESCRIPTION;
        }
        app.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == 0 && editor.fields[DESCRIPTION].text == "gG\nsecond line")
        );
        app.handle_key(key(KeyCode::End), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].cursor == 2)
        );
        app.open_copy_text();
        app.handle_key(key(KeyCode::Up), &mut store, &config);
        assert!(matches!(app.screen, Screen::CopyText { row: 0, .. }));
        for _ in 0..10 {
            app.handle_key(key(KeyCode::Down), &mut store, &config);
        }
        let Screen::CopyText { row, .. } = app.screen else {
            panic!()
        };
        app.handle_key(key(KeyCode::Down), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::CopyText { row: actual, .. } if actual == row));
        app.screen = Screen::Help {
            previous: Box::new(Screen::Navigation),
        };
        app.help_scroll = 0;
        rendered(&mut app, &config, 80, 12);
        app.handle_key(key(KeyCode::Up), &mut store, &config);
        assert_eq!(app.help_scroll, 0);
        for _ in 0..app.help_rows + 2 {
            app.handle_key(key(KeyCode::Down), &mut store, &config);
        }
        assert_eq!(app.help_scroll, app.help_rows - app.help_page);
        app.handle_key(key(KeyCode::Home), &mut store, &config);
        assert_eq!(app.help_scroll, app.help_rows - app.help_page);
        app.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert_eq!(app.help_scroll, 0);
        for screen in [
            Screen::Filter(TextBuffer::default()),
            Screen::ListName {
                id: None,
                buffer: TextBuffer::default(),
            },
        ] {
            app.screen = screen;
            app.handle_key(key(KeyCode::Char('g')), &mut store, &config);
            app.handle_key(key(KeyCode::Char('G')), &mut store, &config);
            app.handle_key(key(KeyCode::Home), &mut store, &config);
            assert!(
                matches!(&app.screen, Screen::Filter(buffer) | Screen::ListName { buffer, .. } if buffer.text == "gG" && buffer.cursor == 0)
            );
        }
        assert_eq!(store.tasks().unwrap(), unchanged);
    }

    #[test]
    fn history_hides_planned_date_suffixes_but_preserves_titles_filters_and_editor_values() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let mut value = input(work.id, "Known closure");
        value.due = Some(parse_date("2035-11-09").unwrap());
        let known = store.create_task(&value).unwrap();
        store
            .set_status_on(known.id, known.version, Status::Done, today())
            .unwrap();
        value.title = "Remember 1999-01-01".into();
        value.due = Some(parse_date("2036-01-02").unwrap());
        let unknown = store.create_task(&value).unwrap();
        store
            .set_status_on(unknown.id, unknown.version, Status::Cancelled, today())
            .unwrap();
        rusqlite::Connection::open(directory.path().join("tasks.db"))
            .unwrap()
            .execute(
                "UPDATE tasks SET status_changed_on = NULL, status_changed_at = NULL WHERE id = ?1",
                [unknown.id],
            )
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        for scope in [None, Some(work.id)] {
            app.set_view(scope.map_or(View::Planned, View::List), &store)
                .unwrap();
            app.open_history(&store).unwrap();
            let display = rendered(&mut app, &config, 100, 24);
            assert!(!display.contains("2035-11-09"));
            assert!(!display.contains("2036-01-02"));
            assert!(display.contains("Remember 1999-01-01"));
            assert!(display.contains("Unknown date"));
            assert!(display.contains(&format!("Today - {}", date_label(today(), Language::En))));
            app.filter = "due:2035-11-09".into();
            app.query = Query::parse(&app.filter).unwrap();
            app.refresh(&store).unwrap();
            assert_eq!(app.visible.len(), 1);
            assert_eq!(app.selected_task().unwrap().id, known.id);
            app.open_selection(&store).unwrap();
            let display = rendered(&mut app, &config, 100, 24);
            assert!(display.contains("Planned date"));
            assert!(display.contains("2035-11-09"));
            app.handle_key(key(KeyCode::Esc), &mut store, &config);
        }
        for view in [View::Planned, View::List(work.id)] {
            app.set_view(view, &store).unwrap();
            app.filter = "s:all".into();
            app.query = Query::parse(&app.filter).unwrap();
            app.refresh(&store).unwrap();
            let display = rendered(&mut app, &config, 100, 24);
            assert!(display.contains("2035-11-09"));
            assert!(display.contains("2036-01-02"));
        }
        let known = store.task(known.id).unwrap();
        store.set_deleted(known.id, known.version, true).unwrap();
        app.set_view(View::Trash, &store).unwrap();
        assert!(rendered(&mut app, &config, 100, 24).contains("2035-11-09"));
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
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == "Unsaved description")
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

    fn visible_ids(app: &App) -> Vec<i64> {
        app.visible.iter().map(|task| task.id).collect()
    }

    /// The first screen row containing `needle`. Rows are `width` cells of single-width symbols.
    fn row_text(screen: &str, width: usize, needle: &str) -> Option<String> {
        screen
            .chars()
            .collect::<Vec<_>>()
            .chunks(width)
            .map(|row| row.iter().collect::<String>())
            .find(|row| row.contains(needle))
    }

    #[test]
    fn closed_tasks_hold_their_row_until_the_view_changes() {
        let (directory, mut store, config) = fixture();
        let today = Local::now().date_naive();
        let work = store.create_list("Work").unwrap();
        let mut ids = Vec::new();
        for title in ["First", "Second", "Third"] {
            let mut fields = input(1, title);
            fields.due = Some(today);
            ids.push(store.create_task(&fields).unwrap().id);
        }
        let mut app = App::new(&store, today).unwrap();
        app.selection = 1;
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(visible_ids(&app), ids);
        assert_eq!(app.selected_task().unwrap().id, ids[2]);
        assert_eq!(store.task(ids[1]).unwrap().status, Status::Done);
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(!screen.contains(tr(Language::En, "done_today")));
        assert!(screen.contains("[x] [Inbox] Second"));
        assert!(screen.contains(tr(Language::En, "marked_done")));
        let cli = visible_tasks(
            &store.tasks().unwrap(),
            &store.lists().unwrap(),
            &Query::default(),
            None,
            false,
            today,
        );
        assert_eq!(cli.last().unwrap().id, ids[1]);
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Char('?')), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(visible_ids(&app), ids);
        app.set_view(View::List(work.id), &store).unwrap();
        app.set_view(View::Planned, &store).unwrap();
        assert_eq!(visible_ids(&app), vec![ids[0], ids[2], ids[1]]);
        assert!(rendered(&mut app, &config, 80, 24).contains(tr(Language::En, "done_today")));
        app.selection = 0;
        app.handle_key(key(KeyCode::Char('c')), &mut store, &config);
        assert_eq!(visible_ids(&app), vec![ids[0], ids[2], ids[1]]);
        assert_eq!(app.selected_task().unwrap().id, ids[2]);
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let held = external.task(ids[0]).unwrap();
        external
            .set_due(
                held.id,
                held.version,
                today.checked_add_days(chrono::Days::new(12)),
            )
            .unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(visible_ids(&app), vec![ids[2], ids[1], ids[0]]);
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(visible_ids(&app)[0], ids[2]);
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(store.task(ids[2]).unwrap().status, Status::Open);
        assert!(app.held.is_empty());
        app.filter = "s:open".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        assert_eq!(visible_ids(&app), vec![ids[2]]);
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert!(app.visible.is_empty());
        assert!(app.held.is_empty());
        app.clear_filter(&store).unwrap();
        let mut fields = input(1, "Old task");
        let old_date = today.checked_sub_days(chrono::Days::new(7)).unwrap();
        fields.due = Some(old_date);
        let earlier = store.create_task(&fields).unwrap();
        app.refresh(&store).unwrap();
        app.select_task_id(Some(earlier.id));
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(visible_ids(&app)[0], earlier.id);
        let screen = rendered(&mut app, &config, 80, 24);
        let row = row_text(&screen, 80, "[x] [Inbox] Old task").unwrap();
        assert!(row.contains(&old_date.to_string()));
        assert!(screen.contains(tr(Language::En, "overdue")));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(app.held.is_empty());
    }

    #[test]
    fn undo_reverses_the_last_task_action_once() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let mut fields = input(1, "First");
        fields.due = Some(parse_date("2026-11-10").unwrap());
        let first = store.create_task(&fields).unwrap();
        let second = store.create_task(&input(1, "Second")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(
            app.message.as_ref().unwrap().text,
            tr(Language::En, "nothing_to_undo")
        );
        app.select_task_id(Some(first.id));
        app.handle_key(key(KeyCode::Char('x')), &mut store, &config);
        assert_eq!(store.task(first.id).unwrap().due, None);
        assert_eq!(
            app.message.as_ref().unwrap().text,
            "Planned date cleared (was 2026-11-10 Tue W46)"
        );
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(store.task(first.id).unwrap().due, first.due);
        assert_eq!(app.message.as_ref().unwrap().text, "Undone");
        assert_eq!(app.selected_task().unwrap().id, first.id);
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(
            app.message.as_ref().unwrap().text,
            tr(Language::En, "nothing_to_undo")
        );
        app.handle_key(key(KeyCode::Char('T')), &mut store, &config);
        assert_eq!(
            app.message.as_ref().unwrap().text,
            "Planned for 2026-11-09 Mon W46"
        );
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(store.task(first.id).unwrap().due, first.due);
        app.select_task_id(Some(second.id));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(store.task(second.id).unwrap().deleted);
        assert_eq!(app.message.as_ref().unwrap().text, "Moved to Trash");
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert!(!store.task(second.id).unwrap().deleted);
        assert_eq!(app.selected_task().unwrap().id, second.id);
        let closed_on = parse_date("2026-11-01").unwrap();
        let closed_at = DateTime::parse_from_rfc3339("2026-11-01T08:15:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let current = store.task(second.id).unwrap();
        store
            .set_status_at(
                current.id,
                current.version,
                Status::Done,
                closed_on,
                closed_at,
            )
            .unwrap();
        app.open_history(&store).unwrap();
        app.select_task_id(Some(second.id));
        app.handle_key(key(KeyCode::Char('r')), &mut store, &config);
        assert_eq!(store.task(second.id).unwrap().status, Status::Open);
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        let restored = store.task(second.id).unwrap();
        assert_eq!(restored.status, Status::Done);
        assert_eq!(restored.status_changed_on, Some(closed_on));
        assert_eq!(restored.status_changed_at, Some(closed_at));
        app.set_view(View::Planned, &store).unwrap();
        app.filter = "s:all".into();
        app.query = Query::parse(&app.filter).unwrap();
        app.refresh(&store).unwrap();
        app.select_task_id(Some(second.id));
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(store.task(second.id).unwrap().status, Status::Open);
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(store.task(second.id).unwrap(), {
            let mut expected = restored.clone();
            expected.version += 2;
            expected
        });
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let changed = external.task(second.id).unwrap();
        let mut edited = TaskInput::from(&changed);
        edited.title = "Edited elsewhere".into();
        external
            .update_task(changed.id, changed.version, &edited)
            .unwrap();
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert!(
            app.message
                .as_ref()
                .unwrap()
                .text
                .contains("changed since the last action")
        );
        assert!(app.undo.is_none());
        assert_eq!(store.task(second.id).unwrap().title, "Edited elsewhere");
        app.clear_filter(&store).unwrap();
        let third = store.create_task(&input(1, "Third")).unwrap();
        let fourth = store.create_task(&input(1, "Fourth")).unwrap();
        app.refresh(&store).unwrap();
        app.select_task_id(Some(third.id));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        let order = |app: &App| {
            visible_ids(app)
                .into_iter()
                .filter(|id| [third.id, fourth.id].contains(id))
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&app), vec![fourth.id, third.id]);
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(order(&app), vec![third.id, fourth.id]);
        assert_eq!(app.selected_task().unwrap().id, third.id);
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        app.select_picker_identity(RowIdentity::List(work.id));
        if let Screen::MoveTaskPicker { picker, .. } = &mut app.screen {
            picker.selection = app.lists.iter().position(|l| l.id == work.id).unwrap();
        }
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(store.task(third.id).unwrap().list_id, work.id);
        assert_eq!(app.message.as_ref().unwrap().text, "Moved to Work");
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(store.task(third.id).unwrap().list_id, 1);
        let danish = Config::from_toml("language = 'da'").unwrap();
        app.handle_key(key(KeyCode::Char('u')), &mut store, &danish);
        assert_eq!(app.message.as_ref().unwrap().text, "Intet at fortryde");
        let before_edit = store.task(third.id).unwrap();
        app.select_task_id(Some(third.id));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Edited description");
        app.handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        app.paste("1112");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let edited = store.task(third.id).unwrap();
        assert_eq!(edited.description, "Edited description");
        assert_eq!(edited.due, Some(parse_date("2026-11-12").unwrap()));
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        let reverted = store.task(third.id).unwrap();
        assert_eq!(reverted.description, before_edit.description);
        assert_eq!(reverted.due, before_edit.due);
        assert_eq!(reverted.title, before_edit.title);
        assert_eq!(reverted.version, edited.version + 1);
        assert_eq!(app.message.as_ref().unwrap().text, "Undone");
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Mistake");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let created = store
            .tasks()
            .unwrap()
            .into_iter()
            .max_by_key(|t| t.id)
            .unwrap();
        assert_eq!(created.title, "Mistake");
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert!(store.task(created.id).unwrap().deleted);
        assert!(app.undo.is_none());
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert!(app.undo.is_some());
        app.open_picker();
        let errands = store.create_list("Errands").unwrap();
        app.refresh(&store).unwrap();
        app.select_picker_identity(RowIdentity::List(errands.id));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        app.handle_key(key(KeyCode::Char('y')), &mut store, &config);
        assert!(app.undo.is_none());
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.set_view(View::Trash, &store).unwrap();
        let trash_before = store.tasks().unwrap();
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(store.tasks().unwrap(), trash_before);
    }

    #[test]
    fn held_rows_keep_their_order_and_reorders_skip_them() {
        let (_directory, mut store, config) = fixture();
        let today = Local::now().date_naive();
        let mut ids = Vec::new();
        for title in ["T0", "T1", "T2"] {
            let mut fields = input(1, title);
            fields.due = Some(today);
            ids.push(store.create_task(&fields).unwrap().id);
        }
        let mut app = App::new(&store, today).unwrap();
        app.selection = 1;
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(app.selected_task().unwrap().id, ids[2]);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert_eq!(visible_ids(&app), vec![ids[2], ids[0], ids[1]]);
        assert_eq!(app.message.as_ref().unwrap().text, "Updated");
        app.handle_key(key(KeyCode::Char('u')), &mut store, &config);
        assert_eq!(visible_ids(&app), ids);
        app.selection = 0;
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        assert_eq!(visible_ids(&app), ids);
        assert_eq!(app.selected_task().unwrap().id, ids[2]);
        app.handle_key(key(KeyCode::Char('c')), &mut store, &config);
        assert_eq!(visible_ids(&app), ids);
        assert_eq!(app.held.len(), 3);
        let tomorrow = today.checked_add_days(chrono::Days::new(1)).unwrap();
        let quit = app.handle_key_on_date(key(KeyCode::Down), &mut store, &config, tomorrow);
        assert!(!quit);
        assert!(app.held.is_empty());
        assert_eq!(app.today, tomorrow);
    }

    #[test]
    fn help_lists_enter_saving_only_while_enter_is_free_in_the_editor() {
        let (_directory, mut store, _) = fixture();
        let default = Config::default();
        let rebound = Config::from_toml("[keys]\nnext_field = ['enter']\nopen = ['f6']").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('?')), &mut store, &default);
        assert!(
            rendered(&mut app, &default, 80, 120).contains(tr(Language::En, "help_enter_saves"))
        );
        assert!(
            !rendered(&mut app, &rebound, 80, 120).contains(tr(Language::En, "help_enter_saves"))
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &default);
        app.handle_key(key(KeyCode::Char('n')), &mut store, &rebound);
        app.paste("Title");
        app.handle_key(key(KeyCode::Enter), &mut store, &rebound);
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.active == LIST));
        assert!(store.tasks().unwrap().is_empty());
    }

    #[test]
    fn move_task_picker_moves_or_cancels_without_touching_other_fields() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let home = store.create_list("Home").unwrap();
        let mut fields = input(work.id, "Movable");
        fields.description = "Keep me".into();
        fields.due = Some(today());
        let task = store.create_task(&fields).unwrap();
        let other = store.create_task(&input(work.id, "Other")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        app.select_task_id(Some(task.id));
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        assert_eq!(app.mode(), Mode::EditorListPicker);
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(screen.contains(tr(Language::En, "move_task_picker")));
        assert!(!screen.contains(tr(Language::En, "trash")));
        let Screen::MoveTaskPicker { picker, .. } = &app.screen else {
            panic!()
        };
        assert_eq!(app.lists[picker.selection].id, work.id);
        app.handle_key(key(KeyCode::Char('j')), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(store.task(task.id).unwrap(), task);
        assert!(app.message.is_none());
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        app.handle_key(key(KeyCode::End), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let moved = store.task(task.id).unwrap();
        assert_eq!(moved.list_id, home.id);
        assert_eq!(moved.version, task.version + 1);
        assert_eq!(moved.description, "Keep me");
        assert_eq!(moved.due, task.due);
        assert_eq!(visible_ids(&app), vec![other.id]);
        assert_eq!(app.message.as_ref().unwrap().text, "Moved to Home");
        app.set_view(View::Planned, &store).unwrap();
        app.select_task_id(Some(task.id));
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        let screen = rendered(&mut app, &config, 80, 24);
        let (rect, _) = app
            .mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorListRow(0)))
            .copied()
            .unwrap();
        assert!(row_text(&screen, 80, "Inbox").is_some());
        let now = Instant::now();
        for _ in 0..2 {
            app.handle_mouse(
                mouse(MouseEventKind::Down(MouseButton::Left), rect.x, rect.y),
                now,
                &mut store,
                &config,
            )
            .unwrap();
        }
        assert_eq!(store.task(task.id).unwrap().list_id, 1);
        assert!(rendered(&mut app, &config, 80, 24).contains("[Inbox] Movable"));
        store
            .set_deleted(other.id, store.task(other.id).unwrap().version, true)
            .unwrap();
        app.set_view(View::Trash, &store).unwrap();
        app.handle_key(key(KeyCode::Char('m')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn enter_saves_from_title_and_planned_date_but_not_description() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Buy milk");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(app.message.as_ref().unwrap().text, "Created");
        let task = store.tasks().unwrap().remove(0);
        assert_eq!(task.title, "Buy milk");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Line one");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste("Line two");
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == "Line one\nLine two")
        );
        app.handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        app.paste("1112");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(app.message.as_ref().unwrap().text, "Saved");
        let saved = store.task(task.id).unwrap();
        assert_eq!(saved.description, "Line one\nLine two");
        assert_eq!(saved.due, Some(parse_date("2026-11-12").unwrap()));
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(app.message.as_ref().unwrap().is_error);
        app.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(store.tasks().unwrap().len(), 1);
    }

    #[test]
    fn back_clears_an_active_filter_before_leaving_the_view() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let milk = store.create_task(&input(work.id, "Milk")).unwrap();
        let bread = store.create_task(&input(work.id, "Bread")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.set_view(View::List(work.id), &store).unwrap();
        app.handle_key(key(KeyCode::Char('/')), &mut store, &config);
        app.paste("bread");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(visible_ids(&app), vec![bread.id]);
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(screen.contains("Esc Clear filter"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        assert!(app.filter.is_empty());
        assert_eq!(visible_ids(&app), vec![milk.id, bread.id]);
        assert_eq!(app.selected_task().unwrap().id, bread.id);
        assert!(rendered(&mut app, &config, 80, 24).contains("Esc Back"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        app.handle_key(key(KeyCode::Char('/')), &mut store, &config);
        app.paste("milk");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(visible_ids(&app), vec![milk.id]);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        assert!(app.filter.is_empty());
        assert_eq!(visible_ids(&app), vec![milk.id, bread.id]);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
    }

    #[test]
    fn picker_preselects_the_last_visited_list_from_all_tasks() {
        let (directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::AllTasks)
        );
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert_eq!(app.view, View::Planned);
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(work.id))
        );
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(app.view, View::List(work.id));
        app.handle_key(key(KeyCode::Char('h')), &mut store, &config);
        app.handle_key(key(KeyCode::Char('l')), &mut store, &config);
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(work.id))
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.set_view(View::Planned, &store).unwrap();
        let mut external = Store::open(&directory.path().join("tasks.db")).unwrap();
        let preview = external.preview_list_deletion(work.id).unwrap();
        external.delete_list(&preview).unwrap();
        app.refresh(&store).unwrap();
        app.open_picker();
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::AllTasks)
        );
    }

    #[test]
    fn row_date_suffixes_appear_only_where_headings_do_not_show_the_date() {
        let (_directory, mut store, config) = fixture();
        let dates = [
            ("Earlier task", Some("2026-11-01")),
            ("Today task", Some("2026-11-08")),
            ("Tomorrow task", Some("2026-11-09")),
            ("Later task", Some("2026-11-20")),
            ("Sometime task", None),
            ("Done task", Some("2026-11-20")),
            ("Trashed task", Some("2026-11-20")),
        ];
        let mut ids = Vec::new();
        for (title, due) in dates {
            let mut fields = input(1, title);
            fields.due = due.map(|value| parse_date(value).unwrap());
            ids.push(store.create_task(&fields).unwrap().id);
        }
        let done = store.task(ids[5]).unwrap();
        store
            .set_status_on(done.id, done.version, Status::Done, today())
            .unwrap();
        let trashed = store.task(ids[6]).unwrap();
        store
            .set_deleted(trashed.id, trashed.version, true)
            .unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let screen = rendered(&mut app, &config, 100, 30);
        let row = |needle: &str| row_text(&screen, 100, needle).unwrap();
        assert!(row("Earlier task").contains("2026-11-01"));
        assert!(!row("Today task").contains("2026-11-08"));
        assert!(!row("Tomorrow task").contains("2026-11-09"));
        assert!(!row("Later task").contains("2026-11-20"));
        assert!(!row("Sometime task").contains("2026"));
        assert!(row("Done task").contains("2026-11-20"));
        assert!(screen.contains("2026-11-20 Fri W47"));
        app.set_view(View::Trash, &store).unwrap();
        let trash = rendered(&mut app, &config, 100, 30);
        assert!(
            row_text(&trash, 100, "Trashed task")
                .unwrap()
                .contains("2026-11-20")
        );
    }

    #[test]
    fn confirmations_accept_y_and_n_letters() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(work.id, "Draft")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste(" changed");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::ConfirmDiscard { .. }));
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[DESCRIPTION].text == " changed")
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(store.task(task.id).unwrap(), task);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste(" again");
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::ConfirmDiscard { .. }));
        app.handle_key(key(KeyCode::Char('y')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(work.id));
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        assert!(matches!(app.screen, Screen::DeleteListConfirm { .. }));
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(screen.contains("y Yes"));
        assert!(screen.contains("n No"));
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(store.lists().unwrap().len(), 2);
        app.handle_key(key(KeyCode::Char('d')), &mut store, &config);
        app.handle_key(key(KeyCode::Char('y')), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListPicker));
        assert_eq!(store.lists().unwrap().len(), 1);
        assert_eq!(store.task(task.id).unwrap().list_id, 1);
    }

    #[test]
    fn paging_moves_by_displayed_rows_without_wrapping() {
        let (_directory, mut store, config) = fixture();
        for index in 0..30 {
            store
                .create_task(&input(1, &format!("Task {index}")))
                .unwrap();
        }
        for index in 0..30 {
            store.create_list(&format!("List {index}")).unwrap();
        }
        let mut app = App::new(&store, today()).unwrap();
        rendered(&mut app, &config, 80, 16);
        let page = app.page_rows;
        assert!(page > 1 && page < 30);
        app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        assert_eq!(app.selection, page);
        for _ in 0..10 {
            app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        }
        assert_eq!(app.selection, 29);
        app.handle_key(key(KeyCode::PageUp), &mut store, &config);
        assert_eq!(app.selection, 29 - page);
        for _ in 0..10 {
            app.handle_key(key(KeyCode::PageUp), &mut store, &config);
        }
        assert_eq!(app.selection, 0);
        app.open_picker();
        rendered(&mut app, &config, 80, 16);
        let popup_rows = usize::from(app.picker.area.unwrap().height);
        app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        assert_eq!(app.picker.selection, popup_rows);
        for _ in 0..10 {
            app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        }
        assert_eq!(app.picker.selection, app.lists.len() + 1);
        app.handle_key(key(KeyCode::PageUp), &mut store, &config);
        assert_eq!(app.picker.selection, app.lists.len() + 1 - popup_rows);
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::EditorListPicker { .. }));
        rendered(&mut app, &config, 80, 24);
        app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        let Screen::EditorListPicker { picker, .. } = &app.screen else {
            panic!()
        };
        assert_eq!(picker.selection, usize::from(picker.area.unwrap().height));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        if let Screen::Editor(editor) = &mut app.screen {
            editor.active = DESCRIPTION;
        }
        app.paste(&"line\n".repeat(40));
        app.open_copy_text();
        rendered(&mut app, &config, 80, 16);
        let copy_page = app.copy_page as u16;
        assert!(copy_page > 1);
        app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        assert!(matches!(app.screen, Screen::CopyText { row, .. } if row == copy_page));
        for _ in 0..10 {
            app.handle_key(key(KeyCode::PageDown), &mut store, &config);
        }
        assert!(matches!(app.screen, Screen::CopyText { row: 41, .. }));
        app.handle_key(key(KeyCode::PageUp), &mut store, &config);
        assert!(matches!(app.screen, Screen::CopyText { row, .. } if row == 41 - copy_page));
    }

    #[test]
    fn ctrl_c_quits_from_navigation_and_shift_letters_reorder_tasks_and_lists() {
        let (_directory, mut store, config) = fixture();
        let first = store.create_task(&input(1, "First")).unwrap();
        let second = store.create_task(&input(1, "Second")).unwrap();
        let alpha = store.create_list("Alpha").unwrap();
        let beta = store.create_list("Beta").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let control_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert_eq!(visible_ids(&app), vec![second.id, first.id]);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert_eq!(visible_ids(&app), vec![first.id, second.id]);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.handle_key(control_c, &mut store, &config);
        assert!(matches!(app.screen, Screen::Editor(_)));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.open_picker();
        app.select_picker_identity(RowIdentity::List(beta.id));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT),
            &mut store,
            &config,
        );
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|l| l.id)
                .collect::<Vec<_>>(),
            vec![1, beta.id, alpha.id]
        );
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(beta.id))
        );
        assert!(app.handle_key(control_c, &mut store, &config));
        app.screen = Screen::Navigation;
        assert!(app.handle_key(control_c, &mut store, &config));
        assert_eq!(store.task(first.id).unwrap().title, "First");
    }

    #[test]
    fn picker_accepts_lowercase_new_and_rename_keys() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.open_picker();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListName { id: None, .. }));
        app.paste("Errands");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let errands = store.lists().unwrap().pop().unwrap();
        assert_eq!(errands.name, "Errands");
        assert_eq!(
            app.picker_identity(app.picker.selection),
            Some(RowIdentity::List(errands.id))
        );
        app.handle_key(key(KeyCode::Char('r')), &mut store, &config);
        assert!(matches!(app.screen, Screen::ListName { id: Some(id), .. } if id == errands.id));
        app.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        app.paste("Shopping");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert_eq!(store.lists().unwrap().pop().unwrap().name, "Shopping");
        let screen = rendered(&mut app, &config, 80, 24);
        assert!(screen.contains("n New list"));
        assert!(screen.contains("r Rename list"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.handle_key(key(KeyCode::Char('r')), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
    }

    #[test]
    fn footer_shows_the_most_used_keys_and_packs_by_width() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let task = store.create_task(&input(work.id, "Task")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        let screen = rendered(&mut app, &config, 80, 24);
        for text in [
            "Enter Edit",
            "Space Done",
            "c Cancel",
            "d Trash",
            "t/T/w/x Date",
            "u Undo",
            "/ Filter",
            "l Lists",
            "n New task",
            "m Move",
            "h History",
            "? Help",
            "q Quit",
        ] {
            assert!(screen.contains(text), "Missing {text}");
        }
        assert!(!screen.contains("Esc Back"));
        let hints = pack_hints(&app.footer_hints(&config), 80, 2);
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().all(|line| line.width() <= 80));
        let one_line = pack_hints(&app.footer_hints(&config), 200, 2);
        assert_eq!(one_line.len(), 1);
        let narrow = pack_hints(&app.footer_hints(&config), 30, 2);
        assert_eq!(narrow.len(), 2);
        assert!(narrow.iter().all(|line| line.width() <= 30));
        assert!(narrow.concat().contains("Enter Edit"));
        app.set_view(View::List(work.id), &store).unwrap();
        assert!(rendered(&mut app, &config, 80, 24).contains("Esc Back"));
        app.open_history(&store).unwrap();
        let history = rendered(&mut app, &config, 100, 24);
        assert!(history.contains("r Reopen"));
        assert!(!history.contains("n New task"));
        store.set_deleted(task.id, task.version, true).unwrap();
        app.set_view(View::Trash, &store).unwrap();
        let trash = rendered(&mut app, &config, 80, 24);
        for text in ["r Restore", "d Delete forever", "D Empty Trash", "F4 Copy"] {
            assert!(trash.contains(text), "Missing {text}");
        }
        assert!(!trash.contains("Space Done"));
        assert!(!trash.contains("m Move"));
        let danish_trash = rendered(
            &mut app,
            &Config::from_toml("language = 'da'").unwrap(),
            80,
            24,
        );
        assert!(danish_trash.contains("d Slet permanent"));
        assert!(danish_trash.contains("D Tøm papirkurven"));
        app.handle_key(key(KeyCode::Char('/')), &mut store, &config);
        let filter = rendered(&mut app, &config, 80, 24);
        assert!(filter.contains(tr(Language::En, "filter_hint")));
        assert!(filter.contains("Enter Apply"));
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        app.set_view(View::Planned, &store).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        let editor = rendered(&mut app, &config, 80, 24);
        assert!(editor.contains("Ctrl+S Save and close"));
        assert!(editor.contains("Esc Discard edits"));
        assert!(editor.contains("Tab Next field"));
        let danish = Config::from_toml("language = 'da'").unwrap();
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        let danish_screen = rendered(&mut app, &danish, 80, 24);
        assert!(danish_screen.contains("Space Udført"));
        assert!(danish_screen.contains("q Afslut"));
        let danish_hints = pack_hints(&app.footer_hints(&danish), 80, 2);
        assert!(danish_hints.iter().all(|line| line.width() <= 80));
    }

    #[test]
    fn group_headings_show_their_row_counts() {
        let (_directory, mut store, config) = fixture();
        let today = Local::now().date_naive();
        let mut ids = Vec::new();
        for (title, offset) in [("A", 0), ("B", 0), ("C", 1), ("D", 1), ("E", 1)] {
            let mut fields = input(1, title);
            fields.due = today.checked_add_days(chrono::Days::new(offset));
            ids.push(store.create_task(&fields).unwrap().id);
        }
        store.create_task(&input(1, "Undated")).unwrap();
        let mut app = App::new(&store, today).unwrap();
        let screen = rendered(&mut app, &config, 100, 30);
        assert!(row_text(&screen, 100, "Today -").unwrap().contains("(2)"));
        assert!(
            row_text(&screen, 100, "Tomorrow -")
                .unwrap()
                .contains("(3)")
        );
        assert!(
            row_text(&screen, 100, "Sometime")
                .unwrap()
                .contains("Sometime (1)")
        );
        app.select_task_id(Some(ids[2]));
        app.handle_key(key(KeyCode::Char(' ')), &mut store, &config);
        let held = rendered(&mut app, &config, 100, 30);
        assert!(row_text(&held, 100, "Tomorrow -").unwrap().contains("(3)"));
        assert!(!held.contains("Done today"));
        app.handle_key(key(KeyCode::Char('/')), &mut store, &config);
        app.paste("s:open");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let filtered = rendered(&mut app, &config, 100, 30);
        assert!(
            row_text(&filtered, 100, "Tomorrow -")
                .unwrap()
                .contains("(2)")
        );
        assert!(!filtered.contains("Done today"));
        app.clear_filter(&store).unwrap();
        let released = rendered(&mut app, &config, 100, 30);
        assert!(
            row_text(&released, 100, "Tomorrow -")
                .unwrap()
                .contains("(2)")
        );
        assert!(
            row_text(&released, 100, "Done today -")
                .unwrap()
                .contains("(1)")
        );
        app.open_history(&store).unwrap();
        let history = rendered(&mut app, &config, 100, 30);
        assert!(row_text(&history, 100, "Today -").unwrap().contains("(1)"));
        store
            .set_deleted(ids[0], store.task(ids[0]).unwrap().version, true)
            .unwrap();
        app.set_view(View::Trash, &store).unwrap();
        let trash = rendered(&mut app, &config, 100, 30);
        assert!(trash.contains("[ ] A"));
        assert!(!trash.contains("(1)"));
        let danish = Config::from_toml("language = 'da'").unwrap();
        app.set_view(View::Planned, &store).unwrap();
        let danish_screen = rendered(&mut app, &danish, 100, 30);
        assert!(
            row_text(&danish_screen, 100, "I dag -")
                .unwrap()
                .contains("(1)")
        );
        assert!(
            row_text(&danish_screen, 100, "I morgen -")
                .unwrap()
                .contains("(2)")
        );
        assert!(
            row_text(&danish_screen, 100, "Engang")
                .unwrap()
                .contains("Engang (1)")
        );
    }

    fn newest_task(store: &Store) -> Task {
        store
            .tasks()
            .unwrap()
            .into_iter()
            .max_by_key(|task| task.id)
            .unwrap()
    }

    fn editor_target(app: &App, field: usize) -> Rect {
        app.mouse_targets
            .iter()
            .find(|(_, target)| matches!(target, MouseTarget::EditorField { index, .. } if *index == field))
            .map(|(rect, _)| *rect)
            .unwrap()
    }

    fn screen_row(screen: &str, width: u16, row: u16) -> String {
        screen
            .chars()
            .skip(usize::from(row) * usize::from(width))
            .take(usize::from(width))
            .collect()
    }

    #[test]
    fn new_tasks_from_all_tasks_start_in_the_most_common_recent_list() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        let private = store.create_list("Private").unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(1))
        );
        app.handle_key(key(KeyCode::Esc), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        for list_id in [private.id, work.id, work.id, 1] {
            store.create_task(&input(list_id, "Task")).unwrap();
        }
        // Trash does not count, so Private has one vote and Work has two.
        for _ in 0..2 {
            let trashed = store.create_task(&input(private.id, "Mistake")).unwrap();
            store
                .set_deleted(trashed.id, trashed.version, true)
                .unwrap();
        }
        app.refresh(&store).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        let Screen::Editor(editor) = &app.screen else {
            panic!()
        };
        assert_eq!(editor.selected_list_id(), Some(work.id));
        assert_eq!(editor.active, TITLE);
        assert!(editor.fields[TITLE].text.is_empty());
        assert!(editor.fields[DESCRIPTION].text.is_empty());
        assert!(editor.fields[DUE].text.is_empty());
        assert!(!editor.is_dirty());
        let screen = rendered(&mut app, &config, 80, 24);
        assert_eq!(
            screen_row(&screen, 80, editor_target(&app, LIST).y).trim(),
            "Work"
        );
        app.paste("Suggested");
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        let created = newest_task(&store);
        assert_eq!(created.title, "Suggested");
        assert_eq!(created.list_id, work.id);
        assert_eq!(created.due, None);

        app.set_view(View::List(private.id), &store).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.selected_list_id() == Some(private.id))
        );
    }

    #[test]
    fn editor_fields_follow_title_list_date_description_order() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        let active = |app: &App| match &app.screen {
            Screen::Editor(editor) => editor.active,
            _ => panic!(),
        };
        assert_eq!(active(&app), TITLE);
        for expected in [LIST, DUE, DESCRIPTION, TITLE] {
            app.handle_key(key(KeyCode::Tab), &mut store, &config);
            assert_eq!(active(&app), expected);
        }
        for expected in [DESCRIPTION, DUE, LIST, TITLE] {
            app.handle_key(
                KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
                &mut store,
                &config,
            );
            assert_eq!(active(&app), expected);
        }
        let screen = rendered(&mut app, &config, 80, 24);
        let rects = FIELD_ORDER.map(|field| editor_target(&app, field));
        assert!(rects.windows(2).all(|pair| pair[0].y < pair[1].y));
        assert_eq!(rects[0].height, 1);
        assert!(rects[3].height > 1, "Description fills the remaining rows");
        for (rect, label) in rects
            .iter()
            .zip(["Title", "List", "Planned date", "Description"])
        {
            assert!(screen_row(&screen, 80, rect.y - 1).contains(label));
        }
        // A click on Planned date focuses it in its new position.
        app.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                rects[2].x,
                rects[2].y,
            ),
            Instant::now(),
            &mut store,
            &config,
        )
        .unwrap();
        assert_eq!(active(&app), DUE);
    }

    #[test]
    fn two_tabs_plan_a_new_task_for_today() {
        let (_directory, mut store, config) = fixture();
        let work = store.create_list("Work").unwrap();
        store.create_task(&input(work.id, "Earlier")).unwrap();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        app.paste("Pay rent");
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        assert_eq!(app.mode(), Mode::ListField);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        assert_eq!(app.mode(), Mode::DueField);
        app.handle_key(key(KeyCode::Char('t')), &mut store, &config);
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        assert!(matches!(app.screen, Screen::Navigation));
        assert_eq!(app.message.as_ref().unwrap().text, "Created");
        let created = newest_task(&store);
        assert_eq!(created.title, "Pay rent");
        assert_eq!(created.list_id, work.id);
        assert_eq!(created.due, Some(today()));
    }

    #[test]
    fn empty_planned_date_shows_a_localized_sometime_placeholder() {
        let (_directory, mut store, config) = fixture();
        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut store, &config);
        let screen = rendered(&mut app, &config, 80, 24);
        let due = editor_target(&app, DUE);
        assert_eq!(screen_row(&screen, 80, due.y).trim(), "Sometime");
        assert!(
            screen_row(&screen, 80, due.y - 1).contains("YYYY-MM-DD / YYYYMMDD / YYMMDD / MMDD")
        );

        let danish = Config::from_toml("language = 'da'").unwrap();
        let screen = rendered(&mut app, &danish, 80, 24);
        assert_eq!(screen_row(&screen, 80, due.y).trim(), "Engang");
        let title = screen_row(&screen, 80, due.y - 1);
        assert!(title.contains("Planlagt dato"));
        assert!(title.contains("YYYY-MM-DD / YYYYMMDD / YYMMDD / MMDD"));

        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.handle_key(key(KeyCode::Tab), &mut store, &config);
        app.paste("1109");
        let screen = rendered(&mut app, &config, 80, 24);
        assert_eq!(screen_row(&screen, 80, due.y).trim(), "1109");
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.fields[DUE].text == "1109"));
    }

    /// The connection a sync worker would use, plus a second device and their shared sync file.
    struct SyncFixture {
        worker: Store,
        other: Store,
        remote: Arc<MemoryRemote>,
        _other_directory: TempDir,
    }

    fn sync_fixture(directory: &TempDir) -> SyncFixture {
        let remote = MemoryRemote::shared();
        let mut worker = Store::open(&directory.path().join("tasks.db")).unwrap();
        sync_once(&mut worker, &remote).unwrap();
        let other_directory = tempfile::tempdir().unwrap();
        let mut other = Store::open(&other_directory.path().join("tasks.db")).unwrap();
        sync_once(&mut other, &remote).unwrap();
        SyncFixture {
            worker,
            other,
            remote,
            _other_directory: other_directory,
        }
    }

    #[test]
    fn sync_commits_show_a_message_instead_of_the_external_notice() {
        let (directory, mut store, mut config) = fixture();
        let mut sync = sync_fixture(&directory);
        let mut app = App::new(&store, today()).unwrap();

        // A task from another device arrives through the worker's connection.
        sync.other
            .create_task(&input(1, "From another device"))
            .unwrap();
        sync_once(&mut sync.other, &sync.remote).unwrap();
        sync_once(&mut sync.worker, &sync.remote).unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(!app.external_update);
        app.report_sync(None, &config, Instant::now());
        assert_eq!(message(&app), "Synced changes from another device.");
        let screen = rendered(&mut app, &config, 120, 24);
        assert!(screen.contains("From another device"));
        assert!(!screen.contains(tr(config.language, "external_update")));

        // Uploading a local change commits sync state only: no notice and no message.
        app.message = None;
        store.create_task(&input(1, "Created here")).unwrap();
        app.refresh(&store).unwrap();
        let report = sync_once(&mut sync.worker, &sync.remote).unwrap();
        assert_eq!((report.received, report.sent), (0, 1));
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        app.report_sync(None, &config, Instant::now());
        assert!(!app.external_update);
        assert!(app.message.is_none());

        // A script that adds a task and syncs within one refresh interval commits twice:
        // the task, and the sync state. The task still raises the notice.
        sync.worker
            .create_task(&input(1, "Added and synced by a script"))
            .unwrap();
        sync_once(&mut sync.worker, &sync.remote).unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(app.external_update);
        app.report_sync(None, &config, Instant::now());
        assert!(app.message.is_none());
        app.handle_key(key(KeyCode::F(5)), &mut store, &config);
        assert!(!app.external_update);
        app.sync_requested = false;
        app.message = None;

        // A sync that changes nothing commits nothing.
        let version = store.data_version().unwrap();
        sync_once(&mut sync.worker, &sync.remote).unwrap();
        assert_eq!(store.data_version().unwrap(), version);

        // A change by another program still raises the notice that stays until F5.
        sync.worker
            .create_task(&input(1, "Added by a script"))
            .unwrap();
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        assert!(app.external_update);
        app.report_sync(None, &config, Instant::now());
        assert!(app.message.is_none());

        // F5 during a sync commit acknowledges the notice and shows the Danish sync message.
        config.language = Language::Da;
        sync.other.create_task(&input(1, "Endnu en")).unwrap();
        sync_once(&mut sync.other, &sync.remote).unwrap();
        sync_once(&mut sync.worker, &sync.remote).unwrap();
        app.handle_key(key(KeyCode::F(5)), &mut store, &config);
        assert!(!app.external_update);
        app.report_sync(None, &config, Instant::now());
        assert_eq!(
            message(&app),
            "Synkroniserede ændringer fra en anden enhed."
        );
    }

    #[test]
    fn a_conflict_copy_is_announced_and_an_open_draft_is_kept() {
        let (directory, mut store, config) = fixture();
        let task = store.create_task(&input(1, "Buy milk")).unwrap();
        let mut sync = sync_fixture(&directory);
        sync_once(&mut sync.other, &sync.remote).unwrap();
        let theirs = sync.other.tasks().unwrap().remove(0);
        sync.other
            .update_task(theirs.id, theirs.version, &input(1, "Buy oat milk"))
            .unwrap();
        sync_once(&mut sync.other, &sync.remote).unwrap();
        store
            .update_task(task.id, task.version, &input(1, "Buy whole milk"))
            .unwrap();

        let mut app = App::new(&store, today()).unwrap();
        app.handle_key(key(KeyCode::Enter), &mut store, &config);
        app.paste(" tomorrow");
        let report = sync_once(&mut sync.worker, &sync.remote).unwrap();
        assert_eq!(report.conflicts, 1);
        app.refresh_changes(&store, &config, today(), false)
            .unwrap();
        app.report_sync(None, &config, Instant::now());
        assert!(!app.external_update);
        assert_eq!(
            message(&app),
            "Sync kept both versions of a task. Search for #conflict."
        );
        // The draft and the version it was read at survive, so the save reports the change.
        assert!(
            matches!(&app.screen, Screen::Editor(editor) if editor.fields[TITLE].text == "Buy whole milk" && editor.fields[DESCRIPTION].text == " tomorrow")
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut store,
            &config,
        );
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert!(app.message.as_ref().unwrap().is_error);
        let mut titles: Vec<String> = store
            .tasks()
            .unwrap()
            .into_iter()
            .map(|task| task.title)
            .collect();
        titles.sort();
        assert_eq!(titles, ["Buy oat milk", "Buy whole milk #conflict"]);
    }

    #[test]
    fn refresh_asks_the_worker_to_sync_and_a_failure_is_shown_once() {
        let (directory, mut store, mut config) = fixture();
        let remote = MemoryRemote::shared();
        let worker = SyncWorker::spawn(
            directory.path().join("tasks.db"),
            Box::new(Arc::clone(&remote)),
            Cadence {
                debounce: Duration::from_secs(3600),
                poll: Duration::from_secs(3600),
                wake_after: None,
                tick: Duration::from_millis(20),
            },
            true,
            false,
        );
        let wait = |condition: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !condition() {
                assert!(Instant::now() < deadline, "condition not met in time");
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        wait(&|| remote.snapshot().is_some());
        let mut app = App::new(&store, today()).unwrap();
        app.report_sync(Some(&worker), &config, Instant::now());
        assert!(app.message.is_none());

        // F5 starts a sync without waiting for it.
        let downloads = remote.counts().0;
        app.handle_key(key(KeyCode::F(5)), &mut store, &config);
        assert!(app.sync_requested);
        app.report_sync(Some(&worker), &config, Instant::now());
        assert!(!app.sync_requested);
        wait(&|| remote.counts().0 > downloads);

        // A failure appears once, as an error, in the interface language.
        remote.fail(FailureKind::Offline);
        config.language = Language::Da;
        app.handle_key(key(KeyCode::F(5)), &mut store, &config);
        app.message = None;
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.message.is_none() {
            assert!(Instant::now() < deadline, "no failure message");
            std::thread::sleep(Duration::from_millis(20));
            app.report_sync(Some(&worker), &config, Instant::now());
        }
        assert!(app.message.as_ref().unwrap().is_error);
        assert_eq!(
            message(&app),
            "Synkronisering mislykkedes: Ingen forbindelse til synkroniseringslageret. Dine ændringer bliver på denne computer."
        );
        app.message = None;
        app.handle_key(key(KeyCode::F(5)), &mut store, &config);
        app.report_sync(Some(&worker), &config, Instant::now());
        std::thread::sleep(Duration::from_millis(300));
        app.report_sync(Some(&worker), &config, Instant::now());
        assert!(
            app.message.as_ref().is_none_or(|message| !message.is_error),
            "the same failure streak is reported once"
        );
        assert_eq!(
            sync_failure_message(Language::En, FailureKind::Denied),
            "Sync failed: The sync storage refused access. Your changes stay on this computer."
        );
    }
}
