use std::borrow::Cow;

use anyhow::{Result, bail};
use chrono::NaiveDate;

use crate::{
    i18n::{Language, tr},
    model::{Status, Task, TaskList},
    planning::{Bucket, GroupLayout, compare_closed_tasks, sort_tasks},
};

#[derive(Clone, Debug)]
pub struct Query {
    list: Option<String>,
    text: Vec<TextTerm>,
    due: Option<DueFilter>,
    status: Option<Status>,
    explicit_status: bool,
    /// The language of the date group names that unquoted text terms also match.
    /// `None` matches text terms against titles and descriptions only.
    group_names: Option<Language>,
}

#[derive(Clone, Debug)]
struct TextTerm {
    /// The lowercase text to find.
    text: String,
    /// Quoted terms and `#` terms match only titles and descriptions.
    literal: bool,
    /// The position of the term among all terms of the input, field terms included.
    /// Only terms at consecutive positions can join into a phrase.
    position: usize,
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
            group_names: None,
        }
    }
}

impl Query {
    pub fn parse(input: &str) -> Result<Self> {
        let mut query = Self::default();
        for (position, token) in tokenize(input)?.into_iter().enumerate() {
            if token.literal || token.text.starts_with('#') {
                query.text.push(TextTerm {
                    text: token.text.to_lowercase(),
                    literal: true,
                    position,
                });
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
                query.text.push(TextTerm {
                    text: token.text.to_lowercase(),
                    literal: false,
                    position,
                });
            }
        }
        Ok(query)
    }

    /// Lets unquoted text terms also match the day name of the heading each task is
    /// listed under in `language`. A term matches when a word of the name starts with
    /// it, so `to` matches Today and Tomorrow.
    ///
    /// Adjacent unquoted terms that together start a day name of the view become one
    /// phrase term, so the Danish `i mo` matches I morgen and not `i` and `mo` alone.
    pub fn with_group_names(mut self, language: Language) -> Self {
        self.group_names = Some(language);
        self
    }

    /// The query for `layout` with its phrase terms joined. A phrase term is the longest
    /// run of two or more unquoted text terms at consecutive positions whose text,
    /// joined by single spaces, starts a day name that `layout` shows.
    fn with_phrases(&self, layout: GroupLayout) -> Cow<'_, Self> {
        let Some(language) = self.group_names else {
            return Cow::Borrowed(self);
        };
        let names = day_name_keys(layout)
            .iter()
            .map(|key| tr(language, key).to_lowercase())
            .collect::<Vec<_>>();
        if names.is_empty() {
            return Cow::Borrowed(self);
        }
        let phrase = |terms: &[TextTerm]| {
            terms
                .iter()
                .map(|term| term.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let starts_a_name = |run: &[TextTerm]| {
            run.iter().all(|term| !term.literal)
                && run
                    .windows(2)
                    .all(|pair| pair[1].position == pair[0].position + 1)
                && {
                    let joined = phrase(run);
                    names.iter().any(|name| name.starts_with(&joined))
                }
        };
        let mut text = Vec::with_capacity(self.text.len());
        let mut start = 0;
        while start < self.text.len() {
            // A run that starts no name cannot grow into one, so the search stops at the
            // first failure. That keeps long pasted filters linear.
            let end = (start + 2..=self.text.len())
                .take_while(|&end| starts_a_name(&self.text[start..end]))
                .last();
            match end {
                Some(end) => {
                    text.push(TextTerm {
                        text: phrase(&self.text[start..end]),
                        literal: false,
                        position: self.text[start].position,
                    });
                    start = end;
                }
                None => {
                    text.push(self.text[start].clone());
                    start += 1;
                }
            }
        }
        let mut query = self.clone();
        query.text = text;
        Cow::Owned(query)
    }

    /// `layout` is the view the results are for. It decides the default status rule
    /// and which day names text terms can match.
    fn matches(&self, task: &Task, list_name: &str, layout: GroupLayout, today: NaiveDate) -> bool {
        let status_matches = if self.explicit_status {
            self.status.is_none_or(|status| task.status == status)
        } else {
            layout != GroupLayout::Planned
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
        let group_name = self
            .group_names
            .and_then(|language| day_name(task, layout, today, language));
        self.text.iter().all(|term| {
            title.contains(&term.text)
                || description.contains(&term.text)
                || !term.literal
                    && group_name
                        .as_deref()
                        .is_some_and(|name| day_name_matches(name, &term.text))
        })
    }
}

/// The catalog keys of the day names that the headings of `layout` show.
fn day_name_keys(layout: GroupLayout) -> &'static [&'static str] {
    match layout {
        GroupLayout::Planned => &["overdue", "today", "tomorrow", "undated"],
        GroupLayout::History => &["today", "yesterday"],
        GroupLayout::Trash => &[],
    }
}

/// Whether `name`, read from the start of one of its words, starts with `term`.
/// Spaces separate the words of a name, so `i dag` matches `dag` and `i d`.
fn day_name_matches(name: &str, term: &str) -> bool {
    std::iter::once(0)
        .chain(name.match_indices(' ').map(|(index, _)| index + 1))
        .any(|start| name[start..].starts_with(term))
}

/// The lowercase day name of the heading the task is listed under in `layout`.
///
/// All tasks and list views name the planned date group: Earlier, Today, Tomorrow, or
/// Sometime. Soon and Next headings show dates instead. A closed task keeps the name of
/// its planned date, which is the heading its held row shows. History names the day the
/// task was closed: Today or Yesterday. Trash has no headings.
fn day_name(
    task: &Task,
    layout: GroupLayout,
    today: NaiveDate,
    language: Language,
) -> Option<String> {
    let key = match layout {
        GroupLayout::Planned => match Bucket::for_task(task, today) {
            bucket @ (Bucket::Overdue | Bucket::Today | Bucket::Tomorrow | Bucket::Undated) => {
                bucket.key()
            }
            Bucket::Soon | Bucket::Next => return None,
        },
        GroupLayout::History => match task.status_changed_on {
            Some(date) if date == today => "today",
            Some(date) if today.pred_opt() == Some(date) => "yesterday",
            _ => return None,
        },
        GroupLayout::Trash => return None,
    };
    debug_assert!(day_name_keys(layout).contains(&key));
    Some(tr(language, key).to_lowercase())
}

pub fn visible_tasks(
    tasks: &[Task],
    lists: &[TaskList],
    query: &Query,
    list_id: Option<i64>,
    trash: bool,
    today: NaiveDate,
) -> Vec<Task> {
    let layout = if trash {
        GroupLayout::Trash
    } else {
        GroupLayout::Planned
    };
    let query = query.with_phrases(layout);
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
                    layout,
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
    let query = query.with_phrases(GroupLayout::History);
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
                    GroupLayout::History,
                    today,
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    history.sort_by(compare_closed_tasks);
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
            status_changed_at: None,
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
    fn history_orders_mixed_closed_statuses_by_completion_time() {
        let today = parse_date("2026-11-08").unwrap();
        let mut newest = task(1, "Newest");
        newest.status = Status::Done;
        newest.status_changed_on = Some(today);
        newest.status_changed_at = Some("2026-11-08T16:00:00Z".parse().unwrap());
        let mut earlier = task(2, "Earlier");
        earlier.status = Status::Cancelled;
        earlier.status_changed_on = Some(today);
        earlier.status_changed_at = Some("2026-11-08T15:00:00Z".parse().unwrap());
        let mut legacy = task(3, "Unknown time");
        legacy.status = Status::Done;
        legacy.status_changed_on = Some(today);
        let tasks = [legacy, earlier, newest];
        let lists = [TaskList {
            id: 1,
            name: "Work".into(),
            count: 0,
        }];
        let history = visible_history(&tasks, &lists, &Query::default(), None, today);
        assert_eq!(
            history.iter().map(|task| task.id).collect::<Vec<_>>(),
            [1, 2, 3]
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
    fn unquoted_terms_match_word_starts_of_date_group_names() {
        let today = parse_date("2026-11-18").unwrap();
        let dated = |id, title, days: Option<i64>| {
            let mut value = task(id, title);
            value.due = days.map(|days| today + chrono::Duration::days(days));
            value
        };
        let mut tasks = vec![
            dated(1, "Call mom", Some(0)),
            dated(2, "Pay rent", Some(1)),
            dated(3, "Book tickets", Some(2)),
            dated(4, "Fix the toaster", None),
            dated(5, "Renew passport", Some(-1)),
            dated(6, "Visit museum", Some(8)),
        ];
        let lists = [TaskList {
            id: 1,
            name: "Home".into(),
            count: tasks.len(),
        }];
        let search = |tasks: &[Task], input: &str, language: Option<Language>| {
            let mut query = Query::parse(input).unwrap();
            if let Some(language) = language {
                query = query.with_group_names(language);
            }
            visible_tasks(tasks, &lists, &query, None, false, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>()
        };
        let english = Some(Language::En);
        let danish = Some(Language::Da);
        assert_eq!(search(&tasks, "to", english), [1, 2, 4]);
        assert_eq!(search(&tasks, "TOD", english), [1]);
        assert_eq!(search(&tasks, "tomorrow", english), [2]);
        assert_eq!(search(&tasks, "ear", english), [5]);
        assert_eq!(search(&tasks, "some", english), [4]);
        // Soon and Next headings show dates, and names match only from a word start.
        assert!(search(&tasks, "soon", english).is_empty());
        assert!(search(&tasks, "next", english).is_empty());
        assert!(search(&tasks, "day", english).is_empty());
        // Every term must still match.
        assert_eq!(search(&tasks, "to call", english), [1]);
        // A wholly quoted term matches only task text, and so does automation.
        assert_eq!(search(&tasks, "\"to\"", english), [4]);
        assert!(search(&tasks, "\"tod\"", english).is_empty());
        assert_eq!(search(&tasks, "to", None), [4]);
        // Danish names match from either word, and English names do not.
        assert_eq!(search(&tasks, "dag", danish), [1]);
        assert_eq!(search(&tasks, "mor", danish), [2]);
        assert_eq!(search(&tasks, "i", danish), [1, 2, 3, 6, 4]);
        assert_eq!(search(&tasks, "tid", danish), [5]);
        assert_eq!(search(&tasks, "ENG", danish), [4]);
        assert!(search(&tasks, "tomorrow", danish).is_empty());
        // A closed task keeps the name of its planned date, which its held row shows.
        tasks[1].status = Status::Done;
        tasks[1].status_changed_on = Some(today);
        assert_eq!(search(&tasks, "tom", english), [2]);
        // History names the day a task was closed, as its headings do.
        tasks[0].status = Status::Done;
        tasks[0].status_changed_on = Some(today - chrono::Duration::days(1));
        tasks[4].status = Status::Cancelled;
        tasks[4].status_changed_on = Some(today - chrono::Duration::days(3));
        let history = |input: &str, language: Language| {
            let query = Query::parse(input).unwrap().with_group_names(language);
            visible_history(&tasks, &lists, &query, None, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(history("tod", Language::En), [2]);
        assert_eq!(history("yes", Language::En), [1]);
        assert!(history("tom", Language::En).is_empty());
        assert!(history("ear", Language::En).is_empty());
        assert_eq!(history("går", Language::Da), [1]);
        assert_eq!(history("i", Language::Da), [2, 1]);
        // Trash has no headings, so only task text matches there.
        for task in &mut tasks {
            task.deleted = true;
        }
        let trash = Query::parse("to").unwrap().with_group_names(Language::En);
        assert_eq!(
            visible_tasks(&tasks, &lists, &trash, None, true, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>(),
            [4]
        );
    }

    #[test]
    fn adjacent_words_that_start_a_day_name_form_one_phrase() {
        let today = parse_date("2026-11-18").unwrap();
        let dated = |id, title, days: Option<i64>| {
            let mut value = task(id, title);
            value.due = days.map(|days| today + chrono::Duration::days(days));
            value
        };
        let mut tasks = vec![
            dated(1, "Pay rent", Some(1)),
            dated(2, "Configure model source", Some(0)),
            dated(3, "Buy oat milk", None),
            dated(4, "Ring til mor", Some(-1)),
            dated(5, "Notes from i mo meeting", Some(3)),
        ];
        let lists = [TaskList {
            id: 1,
            name: "Home".into(),
            count: tasks.len(),
        }];
        let search = |tasks: &[Task], input: &str, language: Option<Language>| {
            let mut query = Query::parse(input).unwrap();
            if let Some(language) = language {
                query = query.with_group_names(language);
            }
            visible_tasks(tasks, &lists, &query, None, false, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>()
        };
        let danish = Some(Language::Da);
        let english = Some(Language::En);
        // `i mo` starts I morgen, so it matches that name or the text `i mo` only.
        assert_eq!(search(&tasks, "i mo", danish), [1, 5]);
        assert_eq!(search(&tasks, "I  MO", danish), [1, 5]);
        assert_eq!(search(&tasks, "i m", danish), [1, 5]);
        assert_eq!(search(&tasks, "i morgen", danish), [1]);
        assert_eq!(search(&tasks, "i d", danish), [2]);
        // A single `i` stays one word and matches both names and text.
        assert_eq!(search(&tasks, "i", danish), [4, 2, 1, 5, 3]);
        // Words after the phrase stay separate terms.
        assert_eq!(search(&tasks, "i mo pay", danish), [1]);
        assert!(search(&tasks, "i mo milk", danish).is_empty());
        // A quoted word never joins a phrase, and words that start no name stay apart.
        assert_eq!(search(&tasks, "\"i\" mo", danish), [4, 2, 5]);
        assert_eq!(search(&tasks, "i \"mo\"", danish), [4, 2, 5]);
        assert_eq!(search(&tasks, "i i mo", danish), [1, 5]);
        assert_eq!(search(&tasks, "mo i", danish), [4, 2, 1, 5]);
        assert_eq!(search(&tasks, "buy milk", danish), [3]);
        // A field term between two words keeps them apart.
        assert_eq!(search(&tasks, "i l:home mo", danish), [4, 2, 1, 5]);
        assert_eq!(search(&tasks, "l:home i mo", danish), [1, 5]);
        // I går is a History name, so All tasks keeps `i g` as two words.
        assert_eq!(search(&tasks, "i g", danish), [4, 2, 5]);
        // English names have one word each, and without names nothing joins.
        assert_eq!(search(&tasks, "i mo", english), [4, 2, 5]);
        assert_eq!(search(&tasks, "i mo", None), [4, 2, 5]);
        assert_eq!(search(&tasks, "pay rent", english), [1]);
        // History joins I går in the same way.
        tasks[3].status = Status::Done;
        tasks[3].status_changed_on = today.pred_opt();
        tasks[1].status = Status::Done;
        tasks[1].status_changed_on = Some(today);
        let history = |input: &str| {
            let query = Query::parse(input).unwrap().with_group_names(Language::Da);
            visible_history(&tasks, &lists, &query, None, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(history("i g"), [4]);
        assert_eq!(history("i går"), [4]);
        assert_eq!(history("i d"), [2]);
        // History shows no I morgen heading, so `i mo` stays two words there.
        assert_eq!(history("i mo"), [2, 4]);
        // Trash has no headings, so Danish words never join there.
        for task in &mut tasks {
            task.deleted = true;
        }
        let trash = |language: Language| {
            let query = Query::parse("i mo").unwrap().with_group_names(language);
            visible_tasks(&tasks, &lists, &query, None, true, today)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(trash(Language::Da), trash(Language::En));
    }

    #[test]
    fn joining_phrases_stays_fast_for_a_long_pasted_filter() {
        let today = parse_date("2026-11-18").unwrap();
        let input = vec!["mo"; 2_000].join(" ");
        let query = Query::parse(&input).unwrap().with_group_names(Language::Da);
        let started = std::time::Instant::now();
        let visible = visible_tasks(&[task(1, "mo")], &[], &query, None, false, today);
        assert_eq!(visible.len(), 1);
        // The search stops at the first run that starts no name, so this takes microseconds.
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
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
