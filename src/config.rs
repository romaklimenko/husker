use std::{collections::BTreeMap, fs, io::Write, path::Path};

use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde::Deserialize;

use crate::i18n::{Language, tr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Tasks,
    History,
    Lists,
    Editor,
    DueField,
    ConfirmDiscard,
    DeleteListConfirm,
    Filter,
    ListName,
    Language,
    Help,
    CopyText,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Action {
    Up,
    Down,
    Open,
    NewTask,
    NewList,
    RenameList,
    DeleteList,
    Language,
    Lists,
    AllTasks,
    History,
    Reopen,
    Filter,
    ToggleDone,
    DueToday,
    DueTomorrow,
    DueNextWeek,
    ClearDue,
    ToggleCancelled,
    Trash,
    Restore,
    MoveUp,
    MoveDown,
    Help,
    Quit,
    Back,
    Save,
    Discard,
    NextField,
    PreviousField,
    Apply,
    Close,
    Refresh,
    ToggleMouse,
    CopyText,
    CopySelection,
    ScrollLeft,
    ScrollRight,
}

impl Action {
    pub const ALL: [Self; 38] = [
        Self::Up,
        Self::Down,
        Self::Open,
        Self::NewTask,
        Self::NewList,
        Self::RenameList,
        Self::DeleteList,
        Self::Language,
        Self::Lists,
        Self::AllTasks,
        Self::History,
        Self::Reopen,
        Self::Filter,
        Self::DueToday,
        Self::DueTomorrow,
        Self::DueNextWeek,
        Self::ClearDue,
        Self::ToggleDone,
        Self::ToggleCancelled,
        Self::Trash,
        Self::Restore,
        Self::MoveUp,
        Self::MoveDown,
        Self::Help,
        Self::Quit,
        Self::Back,
        Self::Save,
        Self::Discard,
        Self::NextField,
        Self::PreviousField,
        Self::Apply,
        Self::Close,
        Self::Refresh,
        Self::ToggleMouse,
        Self::CopyText,
        Self::CopySelection,
        Self::ScrollLeft,
        Self::ScrollRight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Open => "open",
            Self::NewTask => "new_task",
            Self::NewList => "new_list",
            Self::RenameList => "rename_list",
            Self::DeleteList => "delete_list",
            Self::Language => "language",
            Self::Lists => "lists",
            Self::AllTasks => "all_tasks",
            Self::History => "history",
            Self::Reopen => "reopen",
            Self::Filter => "filter",
            Self::DueToday => "due_today",
            Self::DueTomorrow => "due_tomorrow",
            Self::DueNextWeek => "due_next_week",
            Self::ClearDue => "clear_due",
            Self::ToggleDone => "toggle_done",
            Self::ToggleCancelled => "toggle_cancelled",
            Self::Trash => "trash",
            Self::Restore => "restore",
            Self::MoveUp => "move_up",
            Self::MoveDown => "move_down",
            Self::Help => "help",
            Self::Quit => "quit",
            Self::Back => "back",
            Self::Save => "save",
            Self::Discard => "discard",
            Self::NextField => "next_field",
            Self::PreviousField => "previous_field",
            Self::Apply => "apply",
            Self::Close => "close",
            Self::Refresh => "refresh",
            Self::ToggleMouse => "toggle_mouse",
            Self::CopyText => "copy_text",
            Self::CopySelection => "copy_selection",
            Self::ScrollLeft => "scroll_left",
            Self::ScrollRight => "scroll_right",
        }
    }

    pub fn available_in(self, mode: Mode) -> bool {
        if mode == Mode::DueField {
            return matches!(
                self,
                Self::DueToday | Self::DueTomorrow | Self::DueNextWeek | Self::ClearDue
            ) || self.available_in(Mode::Editor);
        }
        match self {
            Self::DueToday | Self::DueTomorrow | Self::DueNextWeek | Self::ClearDue => {
                matches!(mode, Mode::Tasks | Mode::History)
            }
            Self::Up | Self::Down => {
                matches!(
                    mode,
                    Mode::Tasks
                        | Mode::History
                        | Mode::Lists
                        | Mode::Help
                        | Mode::CopyText
                        | Mode::Language
                )
            }
            Self::Open | Self::Lists | Self::AllTasks | Self::Help | Self::Quit => {
                matches!(mode, Mode::Tasks | Mode::History | Mode::Lists)
            }
            Self::History => matches!(mode, Mode::Tasks | Mode::History),
            Self::Language => matches!(
                mode,
                Mode::Tasks
                    | Mode::History
                    | Mode::Lists
                    | Mode::Editor
                    | Mode::Filter
                    | Mode::ListName
                    | Mode::Help
            ),
            Self::Reopen => mode == Mode::History,
            Self::NewTask | Self::Restore => mode == Mode::Tasks,
            Self::Filter | Self::ToggleDone | Self::ToggleCancelled | Self::Trash => {
                matches!(mode, Mode::Tasks | Mode::History)
            }
            Self::MoveUp | Self::MoveDown => matches!(mode, Mode::Tasks | Mode::Lists),
            Self::NewList | Self::RenameList | Self::DeleteList => mode == Mode::Lists,
            Self::Back => matches!(
                mode,
                Mode::Tasks
                    | Mode::History
                    | Mode::Lists
                    | Mode::Filter
                    | Mode::ListName
                    | Mode::DeleteListConfirm
                    | Mode::Language
            ),
            Self::Save | Self::NextField | Self::PreviousField => mode == Mode::Editor,
            Self::Discard => matches!(mode, Mode::Editor | Mode::ConfirmDiscard),
            Self::Apply => matches!(mode, Mode::Filter | Mode::ListName | Mode::Language),
            Self::Close => matches!(mode, Mode::Help | Mode::CopyText),
            Self::Refresh => true,
            Self::ToggleMouse => mode != Mode::CopyText,
            Self::CopyText => matches!(mode, Mode::Tasks | Mode::History | Mode::Editor),
            Self::CopySelection => matches!(mode, Mode::Editor | Mode::Filter | Mode::ListName),
            Self::ScrollLeft | Self::ScrollRight => mode == Mode::CopyText,
        }
    }

    fn defaults(self) -> &'static [&'static str] {
        match self {
            Self::Up => &["up", "k"],
            Self::Down => &["down", "j"],
            Self::Open => &["enter"],
            Self::NewTask => &["n"],
            Self::NewList => &["N"],
            Self::RenameList => &["R"],
            Self::DeleteList => &["d", "delete"],
            Self::Language => &["f3"],
            Self::Lists => &["l"],
            Self::AllTasks => &[],
            Self::History => &["h"],
            Self::Reopen => &["r"],
            Self::Filter => &["/"],
            Self::DueToday => &["t"],
            Self::DueTomorrow => &["T"],
            Self::DueNextWeek => &["w"],
            Self::ClearDue => &["x"],
            Self::ToggleDone => &["space"],
            Self::ToggleCancelled => &["c"],
            Self::Trash => &["d", "delete"],
            Self::Restore => &["r"],
            Self::MoveUp => &["alt+up"],
            Self::MoveDown => &["alt+down"],
            Self::Help => &["?"],
            Self::Quit => &["q"],
            Self::Back => &["esc"],
            Self::Save => &["ctrl+enter", "ctrl+s"],
            Self::Discard => &["esc", "ctrl+g"],
            Self::NextField => &["tab"],
            Self::PreviousField => &["shift+tab"],
            Self::Apply => &["enter"],
            Self::Close => &["esc", "?"],
            Self::Refresh => &["f5"],
            Self::ToggleMouse => &["f2"],
            Self::CopyText => &["f4"],
            Self::CopySelection => &["ctrl+c"],
            Self::ScrollLeft => &["left"],
            Self::ScrollRight => &["right"],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub language: Language,
    keys: BTreeMap<Action, Vec<Binding>>,
}

#[derive(Clone, Debug)]
struct Binding {
    name: String,
    code: KeyCode,
    modifiers: KeyModifiers,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    language: Language,
    #[serde(default)]
    keys: BTreeMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self::from_toml("").expect("The built-in key bindings must be valid")
    }
}

impl Config {
    pub fn save_language(&self, path: &Path, language: Language) -> Result<()> {
        save_language_file(path, language).with_context(|| {
            format!(
                "Could not save language to configuration '{}'.",
                path.display()
            )
        })
    }

    pub fn load(path: &Path, explicit: bool) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_toml(&text)
                .with_context(|| format!("Invalid configuration '{}'.", path.display())),
            Err(error) if !explicit && error.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(error) => Err(error)
                .with_context(|| format!("Could not read configuration '{}'.", path.display())),
        }
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        let mut file: ConfigFile =
            toml::from_str(text).context("Could not parse TOML configuration.")?;
        if let Some(legacy_keys) = file.keys.remove("planned") {
            if file.keys.contains_key("all_tasks") {
                bail!(
                    "Both 'all_tasks' and legacy 'planned' are set in [keys]. Use only 'all_tasks'."
                );
            }
            file.keys.insert("all_tasks".into(), legacy_keys);
        }
        let mut keys = BTreeMap::new();
        for action in Action::ALL {
            let names = file.keys.remove(action.name()).unwrap_or_else(|| {
                action
                    .defaults()
                    .iter()
                    .map(|name| (*name).into())
                    .collect()
            });
            let bindings = names
                .into_iter()
                .map(|name| parse_binding(&name))
                .collect::<Result<Vec<_>>>()
                .with_context(|| format!("Invalid keys for '{}'.", action.name()))?;
            keys.insert(action, bindings);
        }
        if let Some(name) = file.keys.keys().next() {
            bail!("Unknown action '{name}' in [keys].");
        }
        let config = Self {
            language: file.language,
            keys,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn action(&self, event: KeyEvent, mode: Mode) -> Option<Action> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        let (code, modifiers) = normalize_key(event.code, event.modifiers);
        self.keys.iter().find_map(|(action, bindings)| {
            (action.available_in(mode)
                && bindings
                    .iter()
                    .any(|binding| binding.code == code && binding.modifiers == modifiers))
            .then_some(*action)
        })
    }

    pub fn keys_for(&self, action: Action) -> String {
        let keys: Vec<&str> = self
            .keys
            .get(&action)
            .into_iter()
            .flatten()
            .map(|binding| binding.name.as_str())
            .collect();
        if keys.is_empty() {
            tr(self.language, "unbound").into()
        } else {
            keys.join(" / ")
        }
    }

    pub fn example() -> String {
        let mut example = String::from(
            "# Save as config.toml in your profile, or pass --config FILE.\n# Use en for English or da for Danish.\nlanguage = \"en\"\n\n# Each array replaces the action's default shortcuts. [] disables it.\n# Keys may be reused in different modes. Conflicts in one mode are errors.\n[keys]\n",
        );
        for action in Action::ALL {
            let names: Vec<String> = action
                .defaults()
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect();
            example.push_str(&format!("{} = [{}]\n", action.name(), names.join(", ")));
        }
        example
    }

    fn validate(&self) -> Result<()> {
        for mode in [
            Mode::Tasks,
            Mode::History,
            Mode::Lists,
            Mode::Editor,
            Mode::DueField,
            Mode::ConfirmDiscard,
            Mode::DeleteListConfirm,
            Mode::Filter,
            Mode::ListName,
            Mode::Language,
            Mode::Help,
            Mode::CopyText,
        ] {
            let mut seen: Vec<(&Binding, Action)> = Vec::new();
            for (action, bindings) in &self.keys {
                if !action.available_in(mode) {
                    continue;
                }
                for binding in bindings {
                    if let Some((_, previous)) = seen.iter().find(|(previous, previous_action)| {
                        previous.code == binding.code
                            && previous.modifiers == binding.modifiers
                            && *previous_action != *action
                    }) {
                        bail!(
                            "Key '{}' conflicts between '{}' and '{}' in {mode:?} mode.",
                            binding.name,
                            previous.name(),
                            action.name()
                        );
                    }
                    seen.push((binding, *action));
                }
            }
        }
        Ok(())
    }
}

fn save_language_file(path: &Path, language: Language) -> Result<()> {
    // Resolve existing symlinks so replacing a config does not replace the link itself.
    let path = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error.into()),
    };
    let original = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut text = original.clone().unwrap_or_default();
    Config::from_toml(&text)?;

    #[derive(Deserialize)]
    struct LanguageLocation {
        language: Option<toml::Spanned<String>>,
    }
    let location: LanguageLocation = toml::from_str(&text)?;
    if let Some(value) = location.language {
        text.replace_range(value.span(), &format!("\"{language}\""));
    } else {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let start = if text.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        text.insert_str(start, &format!("language = \"{language}\"{newline}"));
    }
    Config::from_toml(&text)?;
    let permissions = match fs::metadata(&path) {
        Ok(metadata) => {
            let permissions = metadata.permissions();
            if permissions.readonly() {
                bail!("The configuration file is read-only.");
            }
            Some(permissions)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(text.as_bytes())?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
    temporary.as_file().sync_all()?;
    // Do not replace edits that arrived while preparing the new file.
    let current = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if current != original {
        bail!("The configuration changed while saving. Try again.");
    }
    if original.is_some() {
        temporary.persist(path)?;
    } else {
        temporary.persist_noclobber(path)?;
    }
    Ok(())
}

fn parse_binding(name: &str) -> Result<Binding> {
    let parts: Vec<&str> = name.split('+').collect();
    let (key, modifiers) = parts.split_last().context("A key cannot be empty.")?;
    let mut flags = KeyModifiers::NONE;
    for modifier in modifiers {
        let flag = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => bail!("Unknown key modifier '{modifier}' in '{name}'."),
        };
        if flags.contains(flag) {
            bail!("Repeated modifier '{modifier}' in '{name}'.");
        }
        flags.insert(flag);
    }
    let code = if key.chars().count() == 1 {
        KeyCode::Char(key.chars().next().unwrap())
    } else {
        match key.to_ascii_lowercase().as_str() {
            "enter" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "backspace" => KeyCode::Backspace,
            "delete" => KeyCode::Delete,
            "insert" => KeyCode::Insert,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "plus" => KeyCode::Char('+'),
            key if key.starts_with('f') => {
                let number = key[1..]
                    .parse::<u8>()
                    .ok()
                    .filter(|number| (1..=24).contains(number));
                KeyCode::F(number.with_context(|| {
                    format!("Invalid function key '{name}'. Use f1 through f24.")
                })?)
            }
            _ => bail!("Unknown key '{name}'."),
        }
    };
    let (code, modifiers) = normalize_key(code, flags);
    Ok(Binding {
        name: name.into(),
        code,
        modifiers,
    })
}

fn normalize_key(code: KeyCode, mut modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    let code = match code {
        KeyCode::BackTab => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::Tab
        }
        KeyCode::Char(character) if character.is_uppercase() => {
            let lowercase: Vec<char> = character.to_lowercase().collect();
            if lowercase.len() == 1 {
                modifiers.insert(KeyModifiers::SHIFT);
                KeyCode::Char(lowercase[0])
            } else {
                KeyCode::Char(character)
            }
        }
        KeyCode::Char(character) if character.is_ascii_punctuation() => {
            // Terminals disagree on whether shifted punctuation retains Shift.
            modifiers.remove(KeyModifiers::SHIFT);
            KeyCode::Char(character)
        }
        code => code,
    };
    (code, modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_presets_are_case_sensitive_and_inherit_editor_actions_only_in_due_field() {
        let config = Config::default();
        let presets = [
            (KeyCode::Char('t'), Action::DueToday),
            (KeyCode::Char('T'), Action::DueTomorrow),
            (KeyCode::Char('w'), Action::DueNextWeek),
            (KeyCode::Char('x'), Action::ClearDue),
        ];
        for (code, action) in presets {
            for mode in [Mode::Tasks, Mode::History, Mode::DueField] {
                assert_eq!(config.action(key(code), mode), Some(action));
            }
            for mode in [
                Mode::Editor,
                Mode::Filter,
                Mode::ListName,
                Mode::Lists,
                Mode::Help,
                Mode::Language,
                Mode::ConfirmDiscard,
                Mode::DeleteListConfirm,
                Mode::CopyText,
            ] {
                assert_eq!(config.action(key(code), mode), None);
            }
            assert_eq!(
                config.action(
                    KeyEvent::new(code, KeyModifiers::CONTROL | KeyModifiers::ALT),
                    Mode::DueField
                ),
                None
            );
        }
        for code in [KeyCode::Char('t'), KeyCode::Char('T')] {
            assert_eq!(
                config.action(KeyEvent::new(code, KeyModifiers::SHIFT), Mode::DueField),
                Some(Action::DueTomorrow)
            );
        }
        for action in Action::ALL {
            if !presets.iter().any(|(_, preset)| *preset == action) {
                assert_eq!(
                    action.available_in(Mode::DueField),
                    action.available_in(Mode::Editor),
                    "{}",
                    action.name()
                );
            }
        }
    }

    #[test]
    fn due_preset_overrides_replace_defaults_and_validate_due_field_conflicts() {
        let config = Config::from_toml("[keys]\ndue_today = ['f6']\nclear_due = []").unwrap();
        assert_eq!(config.action(key(KeyCode::Char('t')), Mode::DueField), None);
        assert_eq!(
            config.action(key(KeyCode::F(6)), Mode::DueField),
            Some(Action::DueToday)
        );
        assert_eq!(config.action(key(KeyCode::Char('x')), Mode::Tasks), None);
        let error = Config::from_toml("[keys]\ndue_today = ['ctrl+s']").unwrap_err();
        assert!(error.to_string().contains("DueField mode"));
        assert!(Config::from_toml("[keys]\ndue_today = ['n']").is_err());
        assert!(Config::from_toml("[keys]\ndue_tomorrow = ['t']").is_err());
        assert!(Config::example().contains("due_next_week = [\"w\"]"));
    }

    #[test]
    fn saving_language_changes_only_the_root_value_and_keeps_current_bindings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shared.toml");
        let config = Config::from_toml("[keys]\nquit = ['ctrl+q']").unwrap();
        for value in [
            "'en'",
            "\"en\"",
            "'''en'''",
            "\"\"\"en\"\"\"",
            "\"\\u0065n\"",
        ] {
            let original = format!(
                "# Sprog æøå\r\n'language' = {value} # keep me\r\n[keys]\r\nquit = ['ctrl+x']\r\nlanguage = ['f6']\r\n"
            );
            fs::write(&path, &original).unwrap();
            config.save_language(&path, Language::Da).unwrap();
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                original.replacen(value, "\"da\"", 1)
            );
            let loaded = Config::load(&path, true).unwrap();
            assert_eq!(loaded.language, Language::Da);
            assert_eq!(loaded.keys_for(Action::Quit), "ctrl+x");
            assert_eq!(loaded.keys_for(Action::Language), "f6");
            assert_eq!(config.language, Language::En);
            assert_eq!(config.keys_for(Action::Quit), "ctrl+q");
        }
    }

    #[test]
    fn saving_language_creates_config_and_adds_missing_root_setting_before_tables() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        let config = Config::default();
        config.save_language(&path, Language::Da).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "language = \"da\"\n");
        assert_eq!(Config::load(&path, false).unwrap().language, Language::Da);
        for original in [
            "# settings\r\n[keys]\r\nlanguage = ['f6']\r\n",
            "\u{feff}# settings\n[keys]\nquit = ['ctrl+q']\n",
        ] {
            fs::write(&path, original).unwrap();
            config.save_language(&path, Language::Da).unwrap();
            let saved = fs::read_to_string(&path).unwrap();
            assert!(saved.contains(original.trim_start_matches('\u{feff}')));
            assert_eq!(
                saved.starts_with('\u{feff}'),
                original.starts_with('\u{feff}')
            );
            assert_eq!(Config::load(&path, true).unwrap().language, Language::Da);
        }
    }

    #[test]
    fn failed_language_saves_preserve_config_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        let config = Config::default();
        for original in [
            "language = 'xx'",
            "language = 'en'\n[keys]\nquit = ['n']",
            "bad TOML",
        ] {
            fs::write(&path, original).unwrap();
            assert!(config.save_language(&path, Language::Da).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
        }
        fs::write(&path, "language = 'en'\n").unwrap();
        let writable = fs::metadata(&path).unwrap().permissions();
        let mut readonly = writable.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = config.save_language(&path, Language::Da);
        fs::set_permissions(&path, writable).unwrap();
        assert!(format!("{:#}", result.unwrap_err()).contains("read-only"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "language = 'en'\n");
        assert!(
            config
                .save_language(directory.path(), Language::Da)
                .is_err()
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn language_action_respects_modes_and_configured_picker_controls() {
        let config = Config::default();
        for mode in [
            Mode::Tasks,
            Mode::History,
            Mode::Lists,
            Mode::Editor,
            Mode::Filter,
            Mode::ListName,
            Mode::Help,
        ] {
            assert_eq!(
                config.action(key(KeyCode::F(3)), mode),
                Some(Action::Language)
            );
        }
        for mode in [
            Mode::Language,
            Mode::ConfirmDiscard,
            Mode::DeleteListConfirm,
            Mode::CopyText,
        ] {
            assert_eq!(config.action(key(KeyCode::F(3)), mode), None);
        }
        assert_eq!(
            config.action(key(KeyCode::Enter), Mode::Language),
            Some(Action::Apply)
        );
        assert_eq!(
            config.action(key(KeyCode::Esc), Mode::Language),
            Some(Action::Back)
        );
        assert!(Config::from_toml("[keys]\nlanguage = ['f5']").is_err());
        assert!(Config::from_toml("[keys]\napply = ['j']").is_err());
        let config = Config::from_toml("[keys]\nlanguage = ['f6']").unwrap();
        assert_eq!(config.action(key(KeyCode::F(3)), Mode::Editor), None);
        assert_eq!(
            config.action(key(KeyCode::F(6)), Mode::Editor),
            Some(Action::Language)
        );
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn defaults_distinguish_navigation_and_input_modes() {
        let config = Config::default();
        assert_eq!(
            config.action(key(KeyCode::Enter), Mode::Tasks),
            Some(Action::Open)
        );
        assert_eq!(
            config.action(key(KeyCode::Enter), Mode::Filter),
            Some(Action::Apply)
        );
        assert_eq!(
            config.action(key(KeyCode::Esc), Mode::Editor),
            Some(Action::Discard)
        );
        for code in [KeyCode::Enter, KeyCode::Char('s')] {
            assert_eq!(
                config.action(KeyEvent::new(code, KeyModifiers::CONTROL), Mode::Editor),
                Some(Action::Save)
            );
        }
        assert_eq!(
            config.action(key(KeyCode::F(4)), Mode::Editor),
            Some(Action::CopyText)
        );
        assert_eq!(
            config.action(key(KeyCode::F(5)), Mode::CopyText),
            Some(Action::Refresh)
        );
        assert_eq!(config.action(key(KeyCode::F(2)), Mode::CopyText), None);
        assert_eq!(
            config.action(key(KeyCode::Esc), Mode::Filter),
            Some(Action::Back)
        );
        assert_eq!(config.action(key(KeyCode::Char('q')), Mode::Editor), None);
        assert_eq!(
            config.action(key(KeyCode::Char('n')), Mode::Tasks),
            Some(Action::NewTask)
        );
        assert_eq!(
            config.action(key(KeyCode::Char('N')), Mode::Lists),
            Some(Action::NewList)
        );
    }

    #[test]
    fn copy_selection_is_input_only_and_can_be_rebound_or_disabled() {
        let copy = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let config = Config::default();
        for mode in [Mode::Editor, Mode::Filter, Mode::ListName] {
            assert_eq!(config.action(copy, mode), Some(Action::CopySelection));
        }
        for mode in [
            Mode::Tasks,
            Mode::History,
            Mode::Lists,
            Mode::ConfirmDiscard,
            Mode::DeleteListConfirm,
            Mode::Help,
            Mode::CopyText,
        ] {
            assert_eq!(config.action(copy, mode), None);
        }
        let config = Config::from_toml("[keys]\ncopy_selection = ['ctrl+k']").unwrap();
        assert_eq!(config.action(copy, Mode::Editor), None);
        assert_eq!(
            config.action(
                KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
                Mode::Editor
            ),
            Some(Action::CopySelection)
        );
        assert!(Config::from_toml("[keys]\nsave = ['ctrl+c']").is_err());
        let disabled = Config::from_toml("[keys]\ncopy_selection = []\nsave = ['ctrl+c']").unwrap();
        assert_eq!(disabled.action(copy, Mode::Editor), Some(Action::Save));
        assert!(Config::from_toml("[keys]\nquit = ['ctrl+c']").is_ok());
    }

    #[test]
    fn all_tasks_is_optional_and_accepts_only_one_configuration_name() {
        let config = Config::default();
        assert_eq!(config.action(key(KeyCode::Char('a')), Mode::Tasks), None);
        assert_eq!(config.action(key(KeyCode::Char('p')), Mode::Tasks), None);
        let legacy = Config::from_toml("[keys]\nplanned = ['p']").unwrap();
        assert_eq!(
            legacy.action(key(KeyCode::Char('p')), Mode::History),
            Some(Action::AllTasks)
        );
        assert_eq!(legacy.action(key(KeyCode::Char('a')), Mode::Tasks), None);
        let disabled = Config::from_toml("[keys]\nplanned = []").unwrap();
        assert_eq!(disabled.action(key(KeyCode::Char('a')), Mode::Tasks), None);
        assert!(Config::from_toml("[keys]\nplanned = ['p']\nall_tasks = ['a']").is_err());
        assert!(Config::example().contains("all_tasks = []"));
        assert!(!Config::example().contains("planned ="));
    }

    #[test]
    fn lifecycle_actions_are_mode_scoped_and_conflicts_are_rejected() {
        let config = Config::default();
        assert_eq!(
            config.action(key(KeyCode::Char('h')), Mode::Tasks),
            Some(Action::History)
        );
        assert_eq!(config.action(key(KeyCode::Char('h')), Mode::Lists), None);
        assert_eq!(
            config.action(key(KeyCode::Char('r')), Mode::History),
            Some(Action::Reopen)
        );
        assert_eq!(
            config.action(key(KeyCode::Char('r')), Mode::Tasks),
            Some(Action::Restore)
        );
        assert_eq!(
            config.action(key(KeyCode::Delete), Mode::Lists),
            Some(Action::DeleteList)
        );
        assert_eq!(
            config.action(key(KeyCode::Delete), Mode::Tasks),
            Some(Action::Trash)
        );
        assert_eq!(config.action(key(KeyCode::Char('n')), Mode::History), None);
        assert_eq!(
            config.action(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT), Mode::History),
            None
        );
        assert_eq!(
            config.action(key(KeyCode::Esc), Mode::DeleteListConfirm),
            Some(Action::Back)
        );
        assert_eq!(
            config.action(key(KeyCode::Char('q')), Mode::DeleteListConfirm),
            None
        );
        assert!(Config::from_toml("[keys]\nhistory = ['a']\nall_tasks = ['a']").is_err());
        assert!(Config::from_toml("[keys]\nreopen = ['d']").is_err());
        assert!(Config::from_toml("[keys]\ndelete_list = ['R']").is_err());
    }

    #[test]
    fn overrides_replace_defaults_and_can_disable_actions() {
        let config =
            Config::from_toml("language = 'da'\n[keys]\nquit = ['ctrl+q']\ntrash = []").unwrap();
        assert_eq!(config.language, Language::Da);
        assert_eq!(config.action(key(KeyCode::Char('q')), Mode::Tasks), None);
        assert_eq!(
            config.action(
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
                Mode::Tasks
            ),
            Some(Action::Quit)
        );
        assert_eq!(config.action(key(KeyCode::Delete), Mode::Tasks), None);
        assert_eq!(config.keys_for(Action::Quit), "ctrl+q");
    }

    #[test]
    fn conflicts_are_checked_only_within_shared_modes() {
        let error = Config::from_toml("[keys]\nquit = ['n']")
            .unwrap_err()
            .to_string();
        assert!(error.contains("new_task") && error.contains("quit"));
        assert!(Config::from_toml("[keys]\nsave = ['q']").is_ok());
        assert!(Config::from_toml("[keys]\nnew_list = ['shift+n']").is_ok());
    }

    #[test]
    fn invalid_settings_actions_and_keys_are_rejected() {
        for input in [
            "langauge = 'da'",
            "language = 'dk'",
            "[keys]\nunknown = ['x']",
            "[keys]\nquit = ['hyper+q']",
            "[keys]\nquit = ['']",
            "[keys]\nquit = ['f25']",
            "[keys]\nanytime = ['a']",
            "[keys]\nmaybe = ['m']",
        ] {
            assert!(
                Config::from_toml(input).is_err(),
                "Accepted invalid settings: {input}"
            );
        }
    }

    #[test]
    fn handles_terminal_key_variants() {
        let config = Config::default();
        assert_eq!(
            config.action(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
                Mode::Tasks
            ),
            Some(Action::Help)
        );
        assert_eq!(
            config.action(key(KeyCode::BackTab), Mode::Editor),
            Some(Action::PreviousField)
        );
        assert_eq!(
            config.action(
                KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
                Mode::Lists
            ),
            Some(Action::NewList)
        );
        let release = KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        );
        assert_eq!(config.action(release, Mode::Tasks), None);
    }

    #[test]
    fn example_configuration_round_trips() {
        let config = Config::from_toml(&Config::example()).unwrap();
        assert_eq!(config.language, Language::En);
        assert_eq!(
            config.action(key(KeyCode::Char('R')), Mode::Lists),
            Some(Action::RenameList)
        );
        for action in Action::ALL {
            assert_ne!(
                tr(Language::Da, &format!("action_{}", action.name())),
                "Ukendt tekst"
            );
        }
    }

    #[test]
    fn missing_explicit_configuration_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing-config.toml");
        assert!(Config::load(&path, false).is_ok());
        assert!(Config::load(&path, true).is_err());
    }
}
