use chrono::{Datelike, NaiveDate};

use crate::{i18n::Language, model::Task};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Bucket {
    Overdue,
    Today,
    Tomorrow,
    Soon,
    Next,
    Undated,
}

impl Bucket {
    pub fn for_task(task: &Task, today: NaiveDate) -> Self {
        match task.due {
            Some(due) => match due.signed_duration_since(today).num_days() {
                ..0 => Self::Overdue,
                0 => Self::Today,
                1 => Self::Tomorrow,
                2..=7 => Self::Soon,
                _ => Self::Next,
            },
            None => Self::Undated,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Overdue => "overdue",
            Self::Today => "today",
            Self::Tomorrow => "tomorrow",
            Self::Soon => "soon",
            Self::Next => "next",
            Self::Undated => "undated",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "overdue" => Some(Self::Overdue),
            "today" => Some(Self::Today),
            "tomorrow" => Some(Self::Tomorrow),
            "soon" => Some(Self::Soon),
            "next" => Some(Self::Next),
            "undated" => Some(Self::Undated),
            _ => None,
        }
    }
}

pub fn sort_tasks(tasks: &mut [Task], _today: NaiveDate) {
    tasks.sort_by_key(|task| (task.due.is_none(), task.due, task.position, task.id));
}

pub fn date_label(date: NaiveDate, language: Language) -> String {
    let weekdays = match language {
        Language::En => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
        Language::Da => ["man", "tir", "ons", "tor", "fre", "lør", "søn"],
    };
    let weekday = weekdays[date.weekday().num_days_from_monday() as usize];
    let week = date.iso_week();
    if week.year() == date.year() {
        format!("{date} {weekday} W{:02}", week.week())
    } else {
        format!("{date} {weekday} {}-W{:02}", week.year(), week.week())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn task(due: Option<NaiveDate>) -> Task {
        Task {
            id: 1,
            list_id: 1,
            title: "Task".into(),
            description: String::new(),
            due,
            status: Status::Open,
            status_changed_on: None,
            deleted: false,
            position: 0,
            version: 1,
        }
    }

    #[test]
    fn groups_every_boundary_across_a_year() {
        let today = NaiveDate::from_ymd_opt(2026, 12, 28).unwrap();
        for (offset, expected) in [
            (-1, Bucket::Overdue),
            (0, Bucket::Today),
            (1, Bucket::Tomorrow),
            (2, Bucket::Soon),
            (7, Bucket::Soon),
            (8, Bucket::Next),
        ] {
            let due = today + chrono::Duration::days(offset);
            assert_eq!(Bucket::for_task(&task(Some(due)), today), expected);
        }
        assert_eq!(Bucket::for_task(&task(None), today), Bucket::Undated);
    }

    #[test]
    fn sorts_dates_before_positions_and_ids_break_ties() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut later = task(Some(today + chrono::Duration::days(3)));
        later.id = 3;
        let mut earlier = task(Some(today + chrono::Duration::days(2)));
        earlier.id = 2;
        earlier.position = 100;
        let mut tie = earlier.clone();
        tie.id = 1;
        let mut tasks = vec![later, earlier, tie];
        sort_tasks(&mut tasks, today);
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn undated_tasks_follow_all_dates_regardless_of_status() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut undated = task(None);
        undated.id = 1;
        let mut future = task(Some(today + chrono::Duration::days(100)));
        future.id = 2;
        future.position = 100;
        future.status = Status::Done;
        let mut overdue = task(Some(today - chrono::Duration::days(1)));
        overdue.id = 3;
        overdue.status = Status::Cancelled;
        let mut tasks = vec![undated, future, overdue];
        sort_tasks(&mut tasks, today);
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            [3, 2, 1]
        );
    }

    #[test]
    fn formats_localized_weekdays_and_iso_year_boundary() {
        let sunday = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        assert_eq!(date_label(sunday, Language::En), "2026-11-08 Sun W45");
        assert_eq!(date_label(sunday, Language::Da), "2026-11-08 søn W45");
        let boundary = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        assert_eq!(
            date_label(boundary, Language::En),
            "2027-01-01 Fri 2026-W53"
        );
    }
}
