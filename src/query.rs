use anyhow::{Result, bail};
use chrono::NaiveDate;

use crate::{
    model::{Status, Task, TaskList},
    planning::{Bucket, sort_tasks},
};

#[derive(Clone, Debug)]
pub struct Query {
    list: Option<String>,
    text: Vec<String>,
    due: Option<DueFilter>,
    status: Option<Status>,
    explicit_status: bool,
}

#[derive(Clone, Debug)]
enum DueFilter {
    Bucket(Bucket),
    Date(NaiveDate),
    None,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            list: None,
            text: Vec::new(),
            due: None,
            status: Some(Status::Open),
            explicit_status: false,
        }
    }
}

impl Query {
    pub fn parse(input: &str) -> Result<Self> {
        let mut query = Self::default();
        for token in tokenize(input)? {
            if token.literal || token.text.starts_with('#') {
                query.text.push(token.text.to_lowercase());
            } else if let Some((field, value)) = token.text.split_once(':') {
                if value.is_empty() {
                    bail!("The '{field}:' filter needs a value.");
                }
                match field {
                    "l" => {
                        if query.list.is_some() {
                            bail!("The 'l:' filter may appear only once.");
                        }
                        query.list = Some(value.to_lowercase());
                    }
                    "due" => {
                        if query.due.is_some() {
                            bail!("The 'due:' filter may appear only once.");
                        }
                        query.due = Some(if value == "none" {
                            DueFilter::None
                        } else if let Some(bucket) = Bucket::from_key(value) {
                            DueFilter::Bucket(bucket)
                        } else {
                            DueFilter::Date(parse_date(value)?)
                        });
                    }
                    "s" => {
                        if query.explicit_status {
                            bail!("The 's:' filter may appear only once.");
                        }
                        query.status = match value {
                            "open" => Some(Status::Open),
                            "done" => Some(Status::Done),
                            "cancelled" => Some(Status::Cancelled),
                            "all" => None,
                            _ => bail!(
                                "Unknown status '{value}'. Use open, done, cancelled, or all."
                            ),
                        };
                        query.explicit_status = true;
                    }
                    _ => bail!("Unknown filter '{field}:'. Use l:, due:, or s:."),
                }
            } else {
                query.text.push(token.text.to_lowercase());
            }
        }
        Ok(query)
    }

    fn matches(
        &self,
        task: &Task,
        list_name: &str,
        include_all_statuses: bool,
        today: NaiveDate,
    ) -> bool {
        let status_matches = if self.explicit_status {
            self.status.is_none_or(|status| task.status == status)
        } else {
            include_all_statuses
                || task.status == Status::Open
                || task.status_changed_on == Some(today)
        };
        if !status_matches
            || self
                .list
                .as_ref()
                .is_some_and(|name| *name != list_name.to_lowercase())
        {
            return false;
        }
        if self.due.as_ref().is_some_and(|due| match due {
            DueFilter::Bucket(bucket) => Bucket::for_task(task, today) != *bucket,
            DueFilter::Date(date) => task.due != Some(*date),
            DueFilter::None => task.due.is_some(),
        }) {
            return false;
        }
        if self.text.is_empty() {
            return true;
        }
        let title = task.title.to_lowercase();
        let description = task.description.to_lowercase();
        self.text
            .iter()
            .all(|text| title.contains(text) || description.contains(text))
    }
}

pub fn visible_tasks(
    tasks: &[Task],
    lists: &[TaskList],
    query: &Query,
    list_id: Option<i64>,
    trash: bool,
    today: NaiveDate,
) -> Vec<Task> {
    let mut visible: Vec<Task> = tasks
        .iter()
        .filter(|task| {
            task.deleted == trash
                && list_id.is_none_or(|id| task.list_id == id)
                && query.matches(
                    task,
                    lists
                        .iter()
                        .find(|list| list.id == task.list_id)
                        .map_or("", |list| list.name.as_str()),
                    trash,
                    today,
                )
        })
        .cloned()
        .collect();
    sort_tasks(&mut visible, today);
    visible
}

pub fn visible_history(
    tasks: &[Task],
    lists: &[TaskList],
    query: &Query,
    list_id: Option<i64>,
    today: NaiveDate,
) -> Vec<Task> {
    let mut history = tasks
        .iter()
        .filter(|task| {
            !task.deleted
                && task.status != Status::Open
                && list_id.is_none_or(|id| task.list_id == id)
                && query.matches(
                    task,
                    lists
                        .iter()
                        .find(|list| list.id == task.list_id)
                        .map_or("", |list| list.name.as_str()),
                    true,
                    today,
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    history.sort_by(|left, right| {
        right
            .status_changed_on
            .cmp(&left.status_changed_on)
            .then_with(|| right.id.cmp(&left.id))
    });
    history
}

pub fn parse_date(value: &str) -> Result<NaiveDate> {
    let valid_shape = value.len() == 10
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        });
    if valid_shape && let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(date);
    }
    bail!("Invalid date '{value}'. Use a valid date in YYYY-MM-DD format.")
}

#[derive(Debug)]
struct Token {
    text: String,
    literal: bool,
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(&first) = chars.peek() {
        if first.is_whitespace() {
            chars.next();
            continue;
        }
        let mut text = String::new();
        let mut quoted = false;
        let mut had_quote = false;
        let mut had_unquoted_text = false;
        while let Some(&character) = chars.peek() {
            if character.is_whitespace() && !quoted {
                break;
            }
            chars.next();
            if character == '"' {
                quoted = !quoted;
                had_quote = true;
            } else if character == '\\' && quoted {
                match chars.next() {
                    Some(escaped @ ('"' | '\\')) => text.push(escaped),
                    Some(escaped) => bail!(
                        "Unsupported escape '\\{escaped}'. Escape only quotes and backslashes."
                    ),
                    None => bail!("Unclosed quoted filter value."),
                }
            } else {
                text.push(character);
                had_unquoted_text |= !quoted;
            }
        }
        if quoted {
            bail!("Unclosed quoted filter value.");
        }
        if text.is_empty() {
            bail!("A filter term cannot be empty.");
        }
        tokens.push(Token {
            text,
            literal: had_quote && !had_unquoted_text,
        });
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: i64, title: &str) -> Task {
        Task {
            id,
            list_id: 1,
            title: title.into(),
            description: String::new(),
            due: None,
            status: Status::Open,
            status_changed_on: None,
            deleted: false,
            position: id,
            version: 1,
        }
    }

    fn search(tasks: &[Task], input: &str, trash: bool) -> Vec<i64> {
        let lists = [TaskList {
            id: 1,
            name: "Home admin".into(),
            count: tasks.len(),
        }];
        visible_tasks(
            tasks,
            &lists,
            &Query::parse(input).unwrap(),
            None,
            trash,
            parse_date("2026-11-08").unwrap(),
        )
        .iter()
        .map(|task| task.id)
        .collect()
    }

    #[test]
    fn history_intersects_scope_filters_and_closed_status_without_trash() {
        let today = parse_date("2026-11-08").unwrap();
        let yesterday = today.pred_opt().unwrap();
        let lists = [
            TaskList {
                id: 1,
                name: "Work".into(),
                count: 1,
            },
            TaskList {
                id: 2,
                name: "Other".into(),
                count: 0,
            },
        ];
        let mut tasks = vec![task(1, "Open")];
        for (id, list_id, status, changed, deleted, due) in [
            (2, 1, Status::Done, Some(yesterday), false, None),
            (3, 1, Status::Cancelled, Some(today), false, Some(yesterday)),
            (4, 1, Status::Done, Some(today), true, None),
            (5, 2, Status::Done, Some(yesterday), false, None),
            (6, 1, Status::Done, None, false, None),
            (7, 1, Status::Done, Some(today), false, None),
        ] {
            let mut value = task(id, "Closed #review");
            value.list_id = list_id;
            value.status = status;
            value.status_changed_on = changed;
            value.deleted = deleted;
            value.due = due;
            tasks.push(value);
        }
        let history = |text: &str, scope| {
            visible_history(&tasks, &lists, &Query::parse(text).unwrap(), scope, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(history("", Some(1)), [7, 3, 2, 6]);
        assert_eq!(history("s:all #review", None), [7, 3, 5, 2, 6]);
        assert_eq!(history("s:done due:none", Some(1)), [7, 2, 6]);
        assert_eq!(history("due:overdue", Some(1)), [3]);
        assert!(history("l:Other", Some(1)).is_empty());
        assert!(history("s:open", None).is_empty());
        assert_eq!(history("l:Other", None), [5]);
        tasks.last_mut().unwrap().status = Status::Open;
        tasks.last_mut().unwrap().status_changed_on = None;
        assert_eq!(
            visible_history(&tasks, &lists, &Query::default(), Some(1), today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>(),
            [3, 2, 6]
        );
    }

    #[test]
    fn combines_list_phrase_and_literal_hashtags_with_and() {
        let mut first = task(1, "Pay electricity bill");
        first.description = "#Shopping #Monthly".into();
        let second = task(2, "Pay electricity bill");
        assert_eq!(
            search(
                &[first, second],
                "l:\"HOME ADMIN\" \"electricity bill\" #shopping #monthly",
                false
            ),
            [1]
        );
    }

    #[test]
    fn quoted_operators_are_literal_and_escapes_work() {
        let first = task(
            1,
            "Read due:today and #shopping with a \"quote\" at C:\\work",
        );
        let second = task(2, "Unrelated");
        assert_eq!(
            search(
                &[first, second],
                r##""due:today" "#shopping" "\"quote\"" "C:\\work""##,
                false
            ),
            [1]
        );
    }

    #[test]
    fn danish_case_matches_without_removing_accents() {
        let tasks = [task(1, "Køb æbler"), task(2, "Køb abler")];
        assert_eq!(search(&tasks, "ÆBLE", false), [1]);
        assert_eq!(search(&tasks, "kØB", false), [1, 2]);
    }

    #[test]
    fn hashes_are_literal_text_in_titles_and_descriptions() {
        let mut description = task(2, "Other title");
        description.description = "Remember #Shopping-list and #todo:home".into();
        let tasks = [task(1, "Buy #shopping"), description, task(3, "Shopping")];
        assert_eq!(search(&tasks, "#SHOPPING", false), [1, 2]);
        assert_eq!(search(&tasks, "#", false), [1, 2]);
        assert_eq!(search(&tasks, "#todo:HOME", false), [2]);
        assert_eq!(search(&tasks, "#shopping #shopping", false), [1, 2]);
    }

    #[test]
    fn default_status_retains_only_todays_closed_tasks_until_midnight() {
        let today = parse_date("2026-11-08").unwrap();
        let yesterday = today - chrono::Duration::days(1);
        let mut tasks = vec![task(1, "Open")];
        for (id, status, changed_on) in [
            (2, Status::Done, Some(today)),
            (3, Status::Cancelled, Some(today)),
            (4, Status::Done, Some(yesterday)),
            (5, Status::Cancelled, Some(yesterday)),
            (6, Status::Done, None),
            (7, Status::Cancelled, None),
        ] {
            let mut closed = task(id, "Closed");
            closed.status = status;
            closed.status_changed_on = changed_on;
            tasks.push(closed);
        }
        assert_eq!(search(&tasks, "", false), [1, 2, 3]);
        assert_eq!(search(&tasks, "s:open", false), [1]);
        assert_eq!(search(&tasks, "s:done", false), [2, 4, 6]);
        assert_eq!(search(&tasks, "s:cancelled", false), [3, 5, 7]);
        assert_eq!(search(&tasks, "s:all", false), [1, 2, 3, 4, 6, 5, 7]);
        let tomorrow = today + chrono::Duration::days(1);
        let next_day = visible_tasks(&tasks, &[], &Query::default(), None, false, tomorrow);
        assert_eq!(next_day.iter().map(|task| task.id).collect::<Vec<_>>(), [1]);
        tasks[1].deleted = true;
        assert_eq!(search(&tasks, "", false), [1, 3]);
        assert_eq!(search(&tasks, "", true), [2]);
    }

    #[test]
    fn status_defaults_and_trash_are_independent() {
        let open = task(1, "Open");
        let mut done = task(2, "Done");
        done.status = Status::Done;
        let mut deleted = done.clone();
        deleted.id = 3;
        deleted.deleted = true;
        let tasks = [open, done, deleted];
        assert_eq!(search(&tasks, "", false), [1]);
        assert_eq!(search(&tasks, "s:all", false), [1, 2]);
        assert_eq!(search(&tasks, "s:done", false), [2]);
        assert_eq!(search(&tasks, "", true), [3]);
        assert!(search(&tasks, "s:open", true).is_empty());
    }

    #[test]
    fn date_filters_use_exact_dates_groups_and_no_date() {
        let mut today = task(1, "Today");
        today.due = Some(parse_date("2026-11-08").unwrap());
        let mut soon = task(2, "Soon");
        soon.due = Some(parse_date("2026-11-15").unwrap());
        let tasks = [today, soon, task(3, "Undated")];
        assert_eq!(search(&tasks, "due:today", false), [1]);
        assert_eq!(search(&tasks, "due:2026-11-15", false), [2]);
        assert_eq!(search(&tasks, "due:soon", false), [2]);
        assert_eq!(search(&tasks, "due:none", false), [3]);
        assert_eq!(search(&tasks, "due:undated", false), [3]);
    }

    #[test]
    fn invalid_filters_are_rejected() {
        for input in [
            "l:one l:two",
            "due:none due:today",
            "s:all s:done",
            "owner:me",
            "l:",
            "due:maybe",
            "due:anytime",
            "due:2026-02-30",
            "due:2026-2-03",
            "s:unknown",
            "\"unclosed",
            "\"bad\\n\"",
            "\"\"",
        ] {
            assert!(
                Query::parse(input).is_err(),
                "Accepted invalid query: {input}"
            );
        }
    }

    #[test]
    fn list_selection_intersects_the_query() {
        let tasks = [task(1, "Selected")];
        let lists = [TaskList {
            id: 1,
            name: "Home admin".into(),
            count: 1,
        }];
        assert!(
            visible_tasks(
                &tasks,
                &lists,
                &Query::default(),
                Some(2),
                false,
                parse_date("2026-11-08").unwrap()
            )
            .is_empty()
        );
    }
}
