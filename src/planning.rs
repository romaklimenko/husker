use chrono::{Datelike, NaiveDate};

use crate::{
    i18n::Language,
    model::{Status, Task},
};

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
                    right
                        .status_changed_on
                        .cmp(&left.status_changed_on)
                        .then_with(|| right.id.cmp(&left.id))
                }
            })
    });
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
