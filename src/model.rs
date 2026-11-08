use std::{fmt, str::FromStr};

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Open,
    Done,
    Cancelled,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "done" => Ok(Self::Done),
            "cancelled" => Ok(Self::Cancelled),
            _ => anyhow::bail!("Unknown status '{value}'. Use open, done, or cancelled."),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct Task {
    pub id: i64,
    pub list_id: i64,
    pub title: String,
    pub description: String,
    pub due: Option<NaiveDate>,
    pub status: Status,
    pub status_changed_on: Option<NaiveDate>,
    pub status_changed_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub position: i64,
    pub version: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TaskInput {
    pub list_id: i64,
    pub title: String,
    pub description: String,
    pub due: Option<NaiveDate>,
}

impl From<&Task> for TaskInput {
    fn from(task: &Task) -> Self {
        Self {
            list_id: task.list_id,
            title: task.title.clone(),
            description: task.description.clone(),
            due: task.due,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct TaskList {
    pub id: i64,
    pub name: String,
    pub count: usize,
}
