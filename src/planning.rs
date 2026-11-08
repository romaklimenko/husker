use chrono::{Datelike, NaiveDate};

use crate::{i18n::Language, model::Task};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Bucket {
    Overdue,
    Today,
    Tomorrow,
    Soon,
    Next,
    Anytime,
    Maybe,
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
            None if task.ready => Self::Anytime,
            None => Self::Maybe,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Overdue => "overdue",
            Self::Today => "today",
            Self::Tomorrow => "tomorrow",
            Self::Soon => "soon",
            Self::Next => "next",
            Self::Anytime => "anytime",
            Self::Maybe => "maybe",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "overdue" => Some(Self::Overdue),
            "today" => Some(Self::Today),
            "tomorrow" => Some(Self::Tomorrow),
            "soon" => Some(Self::Soon),
            "next" => Some(Self::Next),
            "anytime" => Some(Self::Anytime),
            "maybe" => Some(Self::Maybe),
            _ => None,
        }
    }
}

pub fn sort_tasks(tasks: &mut [Task], today: NaiveDate) {
    tasks.sort_by_key(|task| {
        (
            Bucket::for_task(task, today),
            task.due,
            task.position,
            task.id,
        )
    });
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

    fn task(due: Option<NaiveDate>, ready: bool) -> Task {
        Task {
            id: 1,
            list_id: 1,
            title: "Task".into(),
            description: String::new(),
            due,
            status: Status::Open,
            ready,
            tags: Vec::new(),
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
            assert_eq!(Bucket::for_task(&task(Some(due), true), today), expected);
        }
        assert_eq!(Bucket::for_task(&task(None, true), today), Bucket::Anytime);
        assert_eq!(Bucket::for_task(&task(None, false), today), Bucket::Maybe);
    }

    #[test]
    fn sorts_dates_before_positions_and_ids_break_ties() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut later = task(Some(today + chrono::Duration::days(3)), false);
        later.id = 3;
        let mut earlier = task(Some(today + chrono::Duration::days(2)), false);
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
