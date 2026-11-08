use std::cmp::Ordering;

use anyhow::{Context, Result};
use chrono::{Datelike, Days, NaiveDate};

use crate::{
    i18n::{Language, tr},
    model::{Status, Task},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuePreset {
    Today,
    Tomorrow,
    NextMonday,
    Clear,
}

impl DuePreset {
    pub fn resolve(self, today: NaiveDate) -> Result<Option<NaiveDate>> {
        let days = match self {
            Self::Today => 0,
            Self::Tomorrow => 1,
            Self::NextMonday => 7 - u64::from(today.weekday().num_days_from_monday()),
            Self::Clear => return Ok(None),
        };
        today
            .checked_add_days(Days::new(days))
            .map(Some)
            .context("The due date is outside the supported calendar range.")
    }
}

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

pub fn sort_tasks(tasks: &mut [Task], today: NaiveDate) {
    tasks.sort_by(|left, right| {
        planned_group(left, today)
            .cmp(&planned_group(right, today))
            .then_with(|| {
                if left.status == Status::Open {
                    (left.due, left.position, left.id).cmp(&(right.due, right.position, right.id))
                } else {
                    compare_closed_tasks(left, right)
                }
            })
    });
}

pub fn compare_closed_tasks(left: &Task, right: &Task) -> Ordering {
    right
        .status_changed_on
        .cmp(&left.status_changed_on)
        .then_with(|| right.status_changed_at.cmp(&left.status_changed_at))
        .then_with(|| right.id.cmp(&left.id))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlannedGroup {
    Due(Bucket),
    DoneToday,
    CancelledToday,
    Done,
    Cancelled,
}

impl PlannedGroup {
    pub fn key(self) -> &'static str {
        match self {
            Self::Due(bucket) => bucket.key(),
            Self::DoneToday => "done_today",
            Self::CancelledToday => "cancelled_today",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}

pub fn planned_group(task: &Task, today: NaiveDate) -> PlannedGroup {
    match (task.status, task.status_changed_on == Some(today)) {
        (Status::Open, _) => PlannedGroup::Due(Bucket::for_task(task, today)),
        (Status::Done, true) => PlannedGroup::DoneToday,
        (Status::Cancelled, true) => PlannedGroup::CancelledToday,
        (Status::Done, false) => PlannedGroup::Done,
        (Status::Cancelled, false) => PlannedGroup::Cancelled,
    }
}

pub fn ordinary_group_label(task: &Task, today: NaiveDate, language: Language) -> String {
    let group = planned_group(task, today);
    match (group, task.due) {
        (PlannedGroup::Due(Bucket::Today | Bucket::Tomorrow), Some(date)) => {
            relative_date_label(group.key(), date, language)
        }
        (PlannedGroup::Due(Bucket::Soon | Bucket::Next), Some(date)) => date_label(date, language),
        (PlannedGroup::DoneToday | PlannedGroup::CancelledToday, _) => {
            relative_date_label(group.key(), today, language)
        }
        _ => tr(language, group.key()).to_owned(),
    }
}

pub fn history_group_label(
    date: Option<NaiveDate>,
    today: NaiveDate,
    language: Language,
) -> String {
    match date {
        Some(date) if date == today => relative_date_label("today", date, language),
        Some(date) if today.pred_opt() == Some(date) => {
            relative_date_label("yesterday", date, language)
        }
        Some(date) => date_label(date, language),
        None => tr(language, "unknown_date").to_owned(),
    }
}

fn relative_date_label(key: &str, date: NaiveDate, language: Language) -> String {
    format!("{} - {}", tr(language, key), date_label(date, language))
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

    #[test]
    fn due_presets_use_calendar_days_and_a_strictly_future_monday() {
        let date = |value: &str| NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap();
        for (today, tomorrow, monday) in [
            ("2026-11-08", "2026-11-09", "2026-11-09"),
            ("2026-11-09", "2026-11-10", "2026-11-16"),
            ("2026-04-30", "2026-05-01", "2026-05-04"),
            ("2026-12-31", "2027-01-01", "2027-01-04"),
            ("2024-02-28", "2024-02-29", "2024-03-04"),
            ("2024-02-29", "2024-03-01", "2024-03-04"),
            ("2026-03-29", "2026-03-30", "2026-03-30"),
        ] {
            let today = date(today);
            assert_eq!(DuePreset::Today.resolve(today).unwrap(), Some(today));
            assert_eq!(
                DuePreset::Tomorrow.resolve(today).unwrap(),
                Some(date(tomorrow))
            );
            assert_eq!(
                DuePreset::NextMonday.resolve(today).unwrap(),
                Some(date(monday))
            );
            assert_eq!(DuePreset::Clear.resolve(today).unwrap(), None);
        }
        for offset in 0..7 {
            let today = date("2026-11-09")
                .checked_add_days(Days::new(offset))
                .unwrap();
            assert_eq!(
                DuePreset::NextMonday.resolve(today).unwrap(),
                Some(date("2026-11-16"))
            );
        }
    }

    #[test]
    fn due_presets_report_calendar_overflow_without_panicking() {
        assert_eq!(
            DuePreset::Today.resolve(NaiveDate::MAX).unwrap(),
            Some(NaiveDate::MAX)
        );
        assert_eq!(DuePreset::Clear.resolve(NaiveDate::MAX).unwrap(), None);
        assert!(DuePreset::Tomorrow.resolve(NaiveDate::MAX).is_err());
        assert!(DuePreset::NextMonday.resolve(NaiveDate::MAX).is_err());
    }

    fn task(due: Option<NaiveDate>) -> Task {
        Task {
            id: 1,
            list_id: 1,
            title: "Task".into(),
            description: String::new(),
            due,
            status: Status::Open,
            status_changed_on: None,
            status_changed_at: None,
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
    fn open_undated_tasks_precede_finished_work_regardless_of_due_date() {
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
            [1, 2, 3]
        );
    }

    #[test]
    fn open_due_groups_precede_separate_closed_sections() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let cases = [
            (1, Some(-5), Status::Done),
            (2, Some(-1), Status::Open),
            (3, Some(0), Status::Cancelled),
            (4, Some(0), Status::Open),
            (5, Some(2), Status::Done),
            (6, Some(7), Status::Open),
            (7, None, Status::Cancelled),
            (8, None, Status::Open),
            (9, None, Status::Done),
        ];
        let mut tasks = cases
            .into_iter()
            .map(|(id, offset, status)| {
                let mut value = task(offset.map(|days| today + chrono::Duration::days(days)));
                value.id = id;
                value.position = id;
                value.status = status;
                value
            })
            .collect::<Vec<_>>();
        sort_tasks(&mut tasks, today);
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            [2, 4, 6, 8, 9, 5, 1, 7, 3]
        );
    }

    #[test]
    fn completion_groups_do_not_change_due_buckets_or_invent_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut done = task(None);
        done.status = Status::Done;
        done.status_changed_on = Some(today);
        assert_eq!(planned_group(&done, today), PlannedGroup::DoneToday);
        assert_eq!(Bucket::for_task(&done, today), Bucket::Undated);
        assert_eq!(
            planned_group(&done, today.succ_opt().unwrap()),
            PlannedGroup::Done
        );
        done.due = Some(today.pred_opt().unwrap());
        done.status = Status::Cancelled;
        assert_eq!(planned_group(&done, today), PlannedGroup::CancelledToday);
        assert_eq!(Bucket::for_task(&done, today), Bucket::Overdue);
        done.status_changed_on = None;
        assert_eq!(planned_group(&done, today), PlannedGroup::Cancelled);
    }

    #[test]
    fn closed_sections_use_newest_day_then_id_and_put_unknown_dates_last() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut tasks = [
            (1, None),
            (2, Some(today.pred_opt().unwrap())),
            (3, Some(today)),
            (4, Some(today)),
        ]
        .into_iter()
        .map(|(id, date)| {
            let mut value = task(None);
            value.id = id;
            value.status = Status::Done;
            value.status_changed_on = date;
            value
        })
        .collect::<Vec<_>>();
        sort_tasks(&mut tasks, today);
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            [4, 3, 2, 1]
        );
    }

    #[test]
    fn closed_sections_use_completion_instants_before_ids_and_keep_unknown_times_last() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut tasks = [
            (1, Some(today), Some("2026-11-08T10:00:00.000001Z")),
            (2, Some(today), Some("2026-11-08T11:00:00.000002+01:00")),
            (3, Some(today), Some("2026-11-08T10:00:00.000002Z")),
            (90, Some(today), Some("2026-11-08T09:00:00Z")),
            (91, Some(today), None),
            (99, today.pred_opt(), Some("2026-11-07T23:59:59Z")),
            (100, None, None),
        ]
        .into_iter()
        .map(|(id, day, instant)| {
            let mut value = task(None);
            value.id = id;
            value.position = 200 - id;
            value.status = Status::Done;
            value.status_changed_on = day;
            value.status_changed_at = instant.map(|value| value.parse().unwrap());
            value
        })
        .collect::<Vec<_>>();
        sort_tasks(&mut tasks, today);
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            [3, 2, 1, 90, 91, 99, 100]
        );
        tasks[0].due = today.succ_opt();
        tasks[1].position = -100;
        sort_tasks(&mut tasks, today);
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            [3, 2, 1, 90, 91, 99, 100]
        );
    }

    #[test]
    fn ordinary_headings_show_each_future_date_without_changing_due_buckets() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        for (date, bucket, english, danish) in [
            (
                Some((2026, 11, 7)),
                Bucket::Overdue,
                "Overdue",
                "Overskredet",
            ),
            (
                Some((2026, 11, 8)),
                Bucket::Today,
                "Today - 2026-11-08 Sun W45",
                "I dag - 2026-11-08 søn W45",
            ),
            (
                Some((2026, 11, 9)),
                Bucket::Tomorrow,
                "Tomorrow - 2026-11-09 Mon W46",
                "I morgen - 2026-11-09 man W46",
            ),
            (
                Some((2026, 11, 10)),
                Bucket::Soon,
                "2026-11-10 Tue W46",
                "2026-11-10 tir W46",
            ),
            (
                Some((2026, 11, 15)),
                Bucket::Soon,
                "2026-11-15 Sun W46",
                "2026-11-15 søn W46",
            ),
            (
                Some((2026, 11, 16)),
                Bucket::Next,
                "2026-11-16 Mon W47",
                "2026-11-16 man W47",
            ),
            (
                Some((2027, 1, 1)),
                Bucket::Next,
                "2027-01-01 Fri 2026-W53",
                "2027-01-01 fre 2026-W53",
            ),
            (None, Bucket::Undated, "Sometime", "Engang"),
        ] {
            let task = task(
                date.map(|(year, month, day)| NaiveDate::from_ymd_opt(year, month, day).unwrap()),
            );
            assert_eq!(ordinary_group_label(&task, today, Language::En), english);
            assert_eq!(ordinary_group_label(&task, today, Language::Da), danish);
            assert_eq!(Bucket::for_task(&task, today), bucket);
            assert_eq!(planned_group(&task, today), PlannedGroup::Due(bucket));
        }
    }

    #[test]
    fn completion_headings_use_status_dates_without_dating_aggregate_sections() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        for (status, english, danish, older_english, older_danish) in [
            (
                Status::Done,
                "Done today - 2026-11-08 Sun W45",
                "Udført i dag - 2026-11-08 søn W45",
                "Done",
                "Udført",
            ),
            (
                Status::Cancelled,
                "Cancelled today - 2026-11-08 Sun W45",
                "Annulleret i dag - 2026-11-08 søn W45",
                "Cancelled",
                "Annulleret",
            ),
        ] {
            let mut task = task(NaiveDate::from_ymd_opt(2027, 1, 1));
            task.status = status;
            task.status_changed_on = Some(today);
            assert_eq!(ordinary_group_label(&task, today, Language::En), english);
            assert_eq!(ordinary_group_label(&task, today, Language::Da), danish);
            for date in [today.pred_opt(), None] {
                task.status_changed_on = date;
                assert_eq!(
                    ordinary_group_label(&task, today, Language::En),
                    older_english
                );
                assert_eq!(
                    ordinary_group_label(&task, today, Language::Da),
                    older_danish
                );
            }
        }
    }

    #[test]
    fn history_headings_include_relative_dates_and_iso_week_years() {
        let today = NaiveDate::from_ymd_opt(2027, 1, 2).unwrap();
        for (date, english, danish) in [
            (
                Some(today),
                "Today - 2027-01-02 Sat 2026-W53",
                "I dag - 2027-01-02 lør 2026-W53",
            ),
            (
                today.pred_opt(),
                "Yesterday - 2027-01-01 Fri 2026-W53",
                "I går - 2027-01-01 fre 2026-W53",
            ),
            (
                NaiveDate::from_ymd_opt(2026, 12, 31),
                "2026-12-31 Thu W53",
                "2026-12-31 tor W53",
            ),
            (None, "Unknown date", "Ukendt dato"),
        ] {
            assert_eq!(history_group_label(date, today, Language::En), english);
            assert_eq!(history_group_label(date, today, Language::Da), danish);
        }
        assert_eq!(
            history_group_label(None, NaiveDate::MIN, Language::En),
            "Unknown date"
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
