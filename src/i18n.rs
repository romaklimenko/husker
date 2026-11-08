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
        "all_tasks" => ("All tasks", "Alle opgaver"),
        "list_picker" => ("Choose a list", "Vælg en liste"),
        "history" => ("History", "Historik"),
        "language_picker" => ("Choose a language", "Vælg et sprog"),
        "language_saved" => ("Language saved.", "Sproget er gemt."),
        "action_language" => ("Language", "Sprog"),
        "done_today" => ("Done today", "Udført i dag"),
        "cancelled_today" => ("Cancelled today", "Annulleret i dag"),
        "yesterday" => ("Yesterday", "I går"),
        "unknown_date" => ("Unknown date", "Ukendt dato"),
        "no_history" => (
            "No completed or cancelled tasks",
            "Ingen udførte eller annullerede opgaver",
        ),
        "default_list" => ("default", "standard"),
        "delete_list_confirm" => ("Delete this list?", "Slet denne liste?"),
        "list_delete_hint" => (
            "Tasks keep their status and dates. Only the list is removed.",
            "Opgaverne beholder status og datoer. Kun listen fjernes.",
        ),
        "move_tasks_to" => ("Move all tasks to", "Flyt alle opgaver til"),
        "unfinished" => ("Unfinished", "Uafsluttede"),
        "trashed" => ("In Trash", "I papirkurven"),
        "list_deleted" => (
            "List deleted. Tasks kept in the default list.",
            "Listen er slettet. Opgaverne er bevaret i standardlisten.",
        ),
        "list_delete_changed" => (
            "The list changed. Review the counts and confirm again.",
            "Listen er ændret. Kontrollér antallene, og bekræft igen.",
        ),
        "list_delete_missing" => (
            "This list was deleted elsewhere.",
            "Listen blev slettet et andet sted.",
        ),
        "default_list_cannot_delete" => (
            "The default list cannot be deleted.",
            "Standardlisten kan ikke slettes.",
        ),
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
        "due_format_hint" => (
            "YYYY-MM-DD / YYYYMMDD / YYMMDD / MMDD",
            "YYYY-MM-DD / YYYYMMDD / YYMMDD / MMDD",
        ),
        "list" => ("List", "Liste"),
        "list_keys_hint" => (
            "Ctrl+Z undo | Ctrl+Y redo",
            "Ctrl+Z fortryd | Ctrl+Y gentag",
        ),
        "list_unavailable" => (
            "List unavailable. Choose another list.",
            "Listen findes ikke. Vælg en anden liste.",
        ),
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
        "reopened" => ("Reopened", "Genåbnet"),
        "deleted" => ("Moved to Trash", "Flyttet til papirkurven"),
        "updated" => ("Updated", "Opdateret"),
        "invalid_date" => ("Invalid date", "Ugyldig dato"),
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
        "action_lists" => ("Choose a list", "Vælg en liste"),
        "action_all_tasks" => ("Show all tasks", "Vis alle opgaver"),
        "action_history" => ("Show history", "Vis historik"),
        "action_reopen" => ("Reopen task", "Genåbn opgave"),
        "action_delete_list" => ("Delete list", "Slet liste"),
        "action_filter" => ("Filter tasks", "Filtrer opgaver"),
        "action_due_today" => ("Set due date to today", "Sæt forfaldsdato til i dag"),
        "action_due_tomorrow" => ("Set due date to tomorrow", "Sæt forfaldsdato til i morgen"),
        "action_due_next_week" => (
            "Set due date to next Monday",
            "Sæt forfaldsdato til næste mandag",
        ),
        "action_clear_due" => ("Clear due date", "Fjern forfaldsdato"),
        "next_monday" => ("Next Monday", "Næste mandag"),
        "clear_due" => ("Clear date", "Fjern dato"),
        "action_toggle_done" => ("Toggle done", "Skift udført"),
        "action_toggle_cancelled" => ("Toggle cancelled", "Skift annulleret"),
        "action_trash" => ("Move to Trash", "Flyt til papirkurv"),
        "action_restore" => ("Restore", "Gendan"),
        "action_move_up" => ("Reorder up", "Flyt op i rækkefølgen"),
        "action_move_down" => ("Reorder down", "Flyt ned i rækkefølgen"),
        "action_help" => ("Show help", "Vis hjælp"),
        "action_quit" => ("Quit", "Afslut"),
        "action_back" => ("Back", "Tilbage"),
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
        "The default list cannot be deleted." => "Standardlisten kan ikke slettes.",
        "Only open tasks can be reordered." => "Kun åbne opgaver kan ændre rækkefølge.",
        "The due date is outside the supported calendar range." => {
            "Forfaldsdatoen ligger uden for det understøttede datointerval."
        }
        "The configuration file is read-only." => "Konfigurationsfilen er skrivebeskyttet.",
        "The configuration changed while saving. Try again." => {
            "Konfigurationen blev ændret under lagring. Prøv igen."
        }
        "A list-scoped store cannot create, rename, reorder, or delete lists." => {
            "Et lager begrænset til én liste kan ikke oprette, omdøbe, flytte eller slette lister."
        }
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
    if let Some(path) = between(message, "Could not save language to configuration '", "'.") {
        return format!("Kunne ikke gemme sproget i konfigurationen '{path}'.");
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
        "'. Use YYYY-MM-DD, YYYYMMDD, YYMMDD, or MMDD.",
    ) {
        return format!("Ugyldig dato '{value}'. Brug YYYY-MM-DD, YYYYMMDD, YYMMDD eller MMDD.");
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
        ". This Husker supports version 4.",
    ) {
        return format!(
            "Databaseversion {value} understøttes ikke. Denne Husker understøtter version 4."
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

    #[test]
    fn editor_date_errors_explain_compact_formats_without_changing_query_errors() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let error = crate::editor_date::parse_editor_date("0230", today).unwrap_err();
        assert_eq!(
            error_message(Language::En, &error),
            "Invalid date '0230'. Use YYYY-MM-DD, YYYYMMDD, YYMMDD, or MMDD."
        );
        assert_eq!(
            error_message(Language::Da, &error),
            "Ugyldig dato '0230'. Brug YYYY-MM-DD, YYYYMMDD, YYMMDD eller MMDD."
        );
        let query_error = crate::query::parse_date("0230").unwrap_err();
        assert_eq!(
            error_message(Language::Da, &query_error),
            "Ugyldig dato '0230'. Brug en gyldig dato i formatet YYYY-MM-DD."
        );
    }
}
