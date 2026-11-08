use std::{fmt, str::FromStr};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Da,
}

impl FromStr for Language {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "en" => Ok(Self::En),
            "da" => Ok(Self::Da),
            _ => bail!("Unsupported language '{value}'. Use 'en' or 'da'."),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::En => "en",
            Self::Da => "da",
        })
    }
}

pub fn tr(language: Language, key: &str) -> &'static str {
    let (english, danish) = match key {
        "app_title" => ("Husker", "Husker"),
        "lists" => ("Lists", "Lister"),
        "planned" => ("Planned", "Planlagt"),
        "trash" => ("Trash", "Papirkurv"),
        "no_tasks" => ("No tasks", "Ingen opgaver"),
        "no_lists" => ("No lists", "Ingen lister"),
        "filter" => ("Filter", "Filter"),
        "editor" => ("Edit task", "Rediger opgave"),
        "new_task" => ("New task", "Ny opgave"),
        "new_list" => ("New list", "Ny liste"),
        "rename_list" => ("Rename list", "Omdøb liste"),
        "title" => ("Title", "Titel"),
        "description" => ("Description", "Beskrivelse"),
        "due" => ("Due date", "Forfaldsdato"),
        "list" => ("List", "Liste"),
        "overdue" => ("Overdue", "Overskredet"),
        "today" => ("Today", "I dag"),
        "tomorrow" => ("Tomorrow", "I morgen"),
        "soon" => ("Soon", "Snart"),
        "next" => ("Next", "Senere"),
        "undated" => ("Sometime", "Engang"),
        "open" => ("Open", "Åben"),
        "done" => ("Done", "Udført"),
        "cancelled" => ("Cancelled", "Annulleret"),
        "saved" => ("Saved", "Gemt"),
        "copied" => ("Copied selection", "Markering kopieret"),
        "copy_failed" => (
            "Could not copy selected text",
            "Kunne ikke kopiere markeret tekst",
        ),
        "copy_short" => ("Copy", "Kopiér"),
        "action_copy_selection" => ("Copy selected text", "Kopiér markeret tekst"),
        "created" => ("Created", "Oprettet"),
        "restored" => ("Restored", "Gendannet"),
        "deleted" => ("Moved to Trash", "Flyttet til papirkurven"),
        "updated" => ("Updated", "Opdateret"),
        "invalid_date" => (
            "Use a valid date in YYYY-MM-DD format",
            "Brug en gyldig dato i formatet YYYY-MM-DD",
        ),
        "unknown_list" => ("List not found", "Listen findes ikke"),
        "error" => ("Error", "Fejl"),
        "save_failed" => (
            "Could not save. Your edits are still open.",
            "Kunne ikke gemme. Dine ændringer er stadig åbne.",
        ),
        "refresh_failed" => ("Could not refresh tasks", "Kunne ikke opdatere opgaver"),
        "refreshed" => (
            "Refreshed. Unsaved text is kept.",
            "Opdateret. Ugemt tekst er bevaret.",
        ),
        "external_update" => (
            "Database updated elsewhere. Unsaved text is kept.",
            "Databasen er opdateret et andet sted. Ugemt tekst er bevaret.",
        ),
        "mouse_on" => ("Mouse navigation on", "Musenavigation til"),
        "mouse_off" => (
            "Mouse navigation off. Select text with your terminal.",
            "Musenavigation fra. Markér tekst i terminalen.",
        ),
        "copy_hint" => (
            "Select text with your terminal. Arrow keys scroll.",
            "Markér tekst i terminalen. Piletasterne ruller.",
        ),
        "terminal_required" => (
            "The interactive interface requires a terminal",
            "Den interaktive visning kræver en terminal",
        ),
        "terminal_too_small" => (
            "Enlarge the terminal to see all fields",
            "Gør terminalen større for at se alle felter",
        ),
        "help" => ("Help", "Hjælp"),
        "unbound" => ("unbound", "ingen genvej"),
        "editor_hint" => (
            "Edit the fields. A blank due date removes the date.",
            "Rediger felterne. En tom forfaldsdato fjerner datoen.",
        ),
        "editor_keys_hint" => (
            "Ctrl+Left/Right word | Shift select | Ctrl+Z/Y undo/redo",
            "Ctrl+Venstre/Højre ord | Shift markér | Ctrl+Z/Y fortryd/gentag",
        ),
        "discard_confirm" => ("Discard unsaved changes?", "Kassér ugemte ændringer?"),
        "discard_confirm_hint" => ("Arrows choose | Enter confirm", "Pile vælg | Enter bekræft"),
        "keep_editing" => ("Keep editing", "Fortsæt redigering"),
        "no" => ("No", "Nej"),
        "yes" => ("Yes", "Ja"),
        "filter_hint" => (
            "Combine text, l:list, due:today, and s:done. #words are text.",
            "Kombiner tekst, l:liste, due:today og s:done. #ord er tekst.",
        ),
        "list_hint" => ("Enter a list name", "Skriv et listenavn"),
        "navigation_hint" => (
            "Select a task to edit or change its status",
            "Vælg en opgave for at redigere eller ændre status",
        ),
        "conflict_hint" => (
            "This task changed elsewhere. Discard and reopen to load it.",
            "Opgaven er ændret et andet sted. Kassér og åbn igen for at indlæse den.",
        ),
        "action_up" => ("Move up", "Gå op"),
        "action_down" => ("Move down", "Gå ned"),
        "action_open" => ("Open", "Åbn"),
        "action_new_task" => ("New task", "Ny opgave"),
        "action_new_list" => ("New list", "Ny liste"),
        "action_rename_list" => ("Rename list", "Omdøb liste"),
        "action_lists" => ("Show lists", "Vis lister"),
        "action_planned" => ("Show planned tasks", "Vis planlagte opgaver"),
        "action_filter" => ("Filter tasks", "Filtrer opgaver"),
        "action_toggle_done" => ("Toggle done", "Skift udført"),
        "action_toggle_cancelled" => ("Toggle cancelled", "Skift annulleret"),
        "action_trash" => ("Move to Trash", "Flyt til papirkurv"),
        "action_restore" => ("Restore", "Gendan"),
        "action_move_up" => ("Reorder up", "Flyt op i rækkefølgen"),
        "action_move_down" => ("Reorder down", "Flyt ned i rækkefølgen"),
        "action_help" => ("Show help", "Vis hjælp"),
        "action_quit" => ("Quit", "Afslut"),
        "action_back" => ("Back to lists", "Tilbage til lister"),
        "action_save" => ("Save and close", "Gem og luk"),
        "action_discard" => ("Discard edits", "Kassér ændringer"),
        "action_next_field" => ("Next field", "Næste felt"),
        "action_previous_field" => ("Previous field", "Forrige felt"),
        "action_apply" => ("Apply", "Anvend"),
        "action_close" => ("Close view", "Luk visning"),
        "action_refresh" => ("Refresh data", "Opdater data"),
        "action_toggle_mouse" => (
            "Toggle mouse navigation",
            "Slå musenavigation til eller fra",
        ),
        "action_copy_text" => ("Show text for copying", "Vis tekst til kopiering"),
        "action_scroll_left" => ("Scroll text left", "Rul tekst til venstre"),
        "action_scroll_right" => ("Scroll text right", "Rul tekst til højre"),
        _ => ("Unknown label", "Ukendt tekst"),
    };
    match language {
        Language::En => english,
        Language::Da => danish,
    }
}

/// Localizes Husker diagnostics while retaining unknown library and OS details.
pub fn error_message(language: Language, error: &anyhow::Error) -> String {
    if language == Language::En {
        return format!("{error:#}");
    }
    error
        .chain()
        .map(|cause| danish_error(&cause.to_string()))
        .collect::<Vec<_>>()
        .join(": ")
}

fn danish_error(message: &str) -> String {
    let translated = match message {
        "A list name cannot be empty." => "Et listenavn må ikke være tomt.",
        "A task title cannot be empty." => "En opgavetitel må ikke være tom.",
        "Selected text contains a null character." => "Den markerede tekst indeholder et nultegn.",
        "A list with that name already exists." => "Der findes allerede en liste med det navn.",
        "Trash and Papirkurv are reserved list names." => {
            "Trash og Papirkurv er reserverede listenavne."
        }
        "Select two different tasks to reorder." => {
            "Vælg to forskellige opgaver for at ændre rækkefølgen."
        }
        "Tasks in Trash cannot be reordered." => "Opgaver i papirkurven kan ikke ændre rækkefølge.",
        "Reorder tasks within the same date or undated group." => {
            "Flyt opgaver inden for samme dato eller gruppen Engang."
        }
        "Reorder tasks within the same open or closed category." => {
            "Flyt opgaver inden for samme åbne eller afsluttede kategori."
        }
        "Unclosed quoted filter value." => "Filterværdien mangler et afsluttende anførselstegn.",
        "A filter term cannot be empty." => "Et filterudtryk må ikke være tomt.",
        "The database contains unversioned tables. No changes were made." => {
            "Databasen indeholder tabeller uden versionsnummer. Ingen ændringer blev gemt."
        }
        _ => "",
    };
    if !translated.is_empty() {
        return translated.into();
    }
    if let Some(id) = between(
        message,
        "Task ",
        " changed in another process. Reload it before saving.",
    ) {
        return format!("Opgave {id} er ændret i en anden proces. Indlæs den igen, før du gemmer.");
    }
    if let Some(id) = between(message, "Task ", " does not exist.") {
        return format!("Opgave {id} findes ikke.");
    }
    if let Some(id) = between(message, "List ", " does not exist.") {
        return format!("Liste {id} findes ikke.");
    }
    if let Some(value) = between(
        message,
        "Invalid date '",
        "'. Use a valid date in YYYY-MM-DD format.",
    ) {
        return format!("Ugyldig dato '{value}'. Brug en gyldig dato i formatet YYYY-MM-DD.");
    }
    if let Some(value) = between(
        message,
        "Unknown status '",
        "'. Use open, done, cancelled, or all.",
    ) {
        return format!("Ukendt status '{value}'. Brug open, done, cancelled eller all.");
    }
    if let Some(value) = between(message, "Unknown filter '", "'. Use l:, due:, or s:.") {
        return format!("Ukendt filter '{value}'. Brug l:, due: eller s:.");
    }
    if let Some(value) = between(message, "The '", "' filter needs a value.") {
        return format!("Filteret '{value}' kræver en værdi.");
    }
    if let Some(value) = between(message, "The '", "' filter may appear only once.") {
        return format!("Filteret '{value}' må kun bruges én gang.");
    }
    if let Some(value) = between(
        message,
        "Unsupported escape '",
        "'. Escape only quotes and backslashes.",
    ) {
        return format!(
            "Ukendt escape-sekvens '{value}'. Brug kun escape til anførselstegn og omvendte skråstreger."
        );
    }
    if let Some(value) = message.strip_prefix("Cannot open database ") {
        return format!("Kan ikke åbne databasen {value}");
    }
    if let Some(value) = between(
        message,
        "Unsupported database schema version ",
        ". This Husker supports version 3.",
    ) {
        return format!(
            "Databaseversion {value} understøttes ikke. Denne Husker understøtter version 3."
        );
    }
    if let Some(version) = between(
        message,
        "Database schema version ",
        " needs an upgrade. Open this profile with writable Husker before connecting read-only.",
    ) {
        return format!(
            "Databaseversion {version} skal opgraderes. Åbn profilen med skriveadgang i Husker, før du forbinder med læseadgang."
        );
    }
    message.into()
}

fn between<'a>(text: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    text.strip_prefix(prefix)?.strip_suffix(suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_values_are_stable() {
        assert_eq!("da".parse::<Language>().unwrap(), Language::Da);
        assert_eq!(Language::En.to_string(), "en");
        assert!("dk".parse::<Language>().is_err());
        assert_eq!(tr(Language::Da, "undated"), "Engang");
        assert_eq!(tr(Language::En, "undated"), "Sometime");
    }

    #[test]
    fn application_errors_keep_values_and_unknown_causes() {
        let conflict =
            anyhow::anyhow!("Task 42 changed in another process. Reload it before saving.");
        assert_eq!(
            error_message(Language::Da, &conflict),
            "Opgave 42 er ændret i en anden proces. Indlæs den igen, før du gemmer."
        );
        let date = crate::query::Query::parse("due:2026-02-30").unwrap_err();
        assert!(error_message(Language::Da, &date).contains("Ugyldig dato '2026-02-30'"));
        let duplicate = crate::query::Query::parse("l:one l:two").unwrap_err();
        assert!(error_message(Language::Da, &duplicate).contains("'l:'"));
        assert!(!error_message(Language::Da, &duplicate).contains("filter may"));
        let external = anyhow::anyhow!("OS error 123");
        assert_eq!(error_message(Language::Da, &external), "OS error 123");
        let null_text = anyhow::anyhow!("Selected text contains a null character.");
        assert_eq!(
            error_message(Language::Da, &null_text),
            "Den markerede tekst indeholder et nultegn."
        );
    }
}
