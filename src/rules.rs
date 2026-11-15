//! Task and list rules shared by the store and the online server.
//!
//! The error sentences are part of the interface. `i18n` translates them by
//! their exact text.

use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};

use crate::model::Status;

/// Trims a task title and refuses an empty one.
pub fn normalize_title(title: &str) -> Result<String> {
    let title = title.trim().to_owned();
    ensure!(!title.is_empty(), "A task title cannot be empty.");
    Ok(title)
}

/// Trims a list name and returns it with the key that decides whether two names are equal.
pub fn normalize_list_name(name: &str) -> Result<(String, String)> {
    let name = name.trim().to_owned();
    ensure!(!name.is_empty(), "A list name cannot be empty.");
    let name_key = name.to_lowercase();
    ensure!(
        name_key != "trash" && name_key != "papirkurv",
        "Trash and Papirkurv are reserved list names."
    );
    Ok((name, name_key))
}

/// The status date and time a task has after its status is set to `status`.
///
/// Reopening clears both. Setting the status a task already has keeps both.
/// Any other change stores the given date and time.
pub fn status_stamp(
    current: Status,
    stored: (Option<NaiveDate>, Option<DateTime<Utc>>),
    status: Status,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> (Option<NaiveDate>, Option<DateTime<Utc>>) {
    match status {
        Status::Open => (None, None),
        _ if status == current => stored,
        _ => (Some(today), Some(now)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_trimmed_and_must_hold_text() {
        assert_eq!(normalize_title("  Buy milk ").unwrap(), "Buy milk");
        assert_eq!(
            normalize_title("   ").unwrap_err().to_string(),
            "A task title cannot be empty."
        );
    }

    #[test]
    fn list_names_are_trimmed_keyed_and_not_reserved() {
        assert_eq!(
            normalize_list_name(" Work ").unwrap(),
            ("Work".to_owned(), "work".to_owned())
        );
        assert!(normalize_list_name(" ").is_err());
        assert!(normalize_list_name("TRASH").is_err());
        assert!(normalize_list_name("papirkurv").is_err());
    }

    #[test]
    fn status_stamps_follow_the_three_cases() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let now = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let earlier = (
            NaiveDate::from_ymd_opt(2026, 11, 1),
            DateTime::from_timestamp(1_700_000_000, 0),
        );
        assert_eq!(
            status_stamp(Status::Open, (None, None), Status::Done, today, now),
            (Some(today), Some(now))
        );
        assert_eq!(
            status_stamp(Status::Done, earlier, Status::Done, today, now),
            earlier
        );
        assert_eq!(
            status_stamp(Status::Done, earlier, Status::Cancelled, today, now),
            (Some(today), Some(now))
        );
        assert_eq!(
            status_stamp(Status::Done, earlier, Status::Open, today, now),
            (None, None)
        );
    }
}
