use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde::Deserialize;

use crate::i18n::{Language, tr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Tasks,
    Lists,
    Editor,
    Filter,
    ListName,
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
    Lists,
    Planned,
    Filter,
    ToggleDone,
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
    ScrollLeft,
    ScrollRight,
}

impl Action {
    pub const ALL: [Self; 29] = [
        Self::Up,
        Self::Down,
        Self::Open,
        Self::NewTask,
        Self::NewList,
        Self::RenameList,
        Self::Lists,
        Self::Planned,
        Self::Filter,
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
            Self::Lists => "lists",
            Self::Planned => "planned",
            Self::Filter => "filter",
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
            Self::ScrollLeft => "scroll_left",
            Self::ScrollRight => "scroll_right",
        }
    }

    pub fn available_in(self, mode: Mode) -> bool {
        match self {
            Self::Up | Self::Down => {
                matches!(
                    mode,
                    Mode::Tasks | Mode::Lists | Mode::Help | Mode::CopyText
                )
            }
            Self::Open | Self::Lists | Self::Planned | Self::Help | Self::Quit => {
                matches!(mode, Mode::Tasks | Mode::Lists)
            }
            Self::NewTask
            | Self::Filter
            | Self::ToggleDone
            | Self::ToggleCancelled
            | Self::Trash
            | Self::Restore => mode == Mode::Tasks,
            Self::MoveUp | Self::MoveDown => matches!(mode, Mode::Tasks | Mode::Lists),
            Self::NewList | Self::RenameList => mode == Mode::Lists,
            Self::Back => matches!(
                mode,
                Mode::Tasks | Mode::Lists | Mode::Filter | Mode::ListName
            ),
            Self::Save | Self::Discard | Self::NextField | Self::PreviousField => {
                mode == Mode::Editor
            }
            Self::Apply => matches!(mode, Mode::Filter | Mode::ListName),
            Self::Close => matches!(mode, Mode::Help | Mode::CopyText),
            Self::Refresh => true,
            Self::ToggleMouse => mode != Mode::CopyText,
            Self::CopyText => matches!(mode, Mode::Tasks | Mode::Editor),
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
            Self::Lists => &["l"],
            Self::Planned => &["p"],
            Self::Filter => &["/"],
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
            Mode::Lists,
            Mode::Editor,
            Mode::Filter,
            Mode::ListName,
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
