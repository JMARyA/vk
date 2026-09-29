//! `vk changes`: what happened since a point in time, so an agent (or a
//! person back from a break) can catch up in one call.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Local, Utc};
use serde::Serialize;
use serde_json::json;
use vikunjars::models::{ModelsProject, ModelsTask};

use crate::api::VikunjaAPI;
use crate::error::{Result, VkError};
use crate::ui::{parse_datetime, time_relative};

/// What happened to a task within the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Created,
    Done,
    Updated,
}

impl Change {
    fn label(self) -> &'static str {
        match self {
            Change::Created => "created",
            Change::Done => "done",
            Change::Updated => "updated",
        }
    }
}

fn time(value: &Option<String>) -> Option<DateTime<Utc>> {
    parse_datetime(value.as_deref()?)
}

/// Created in the window wins over done, and done over a plain update.
pub fn classify(task: &ModelsTask, since: DateTime<Utc>) -> Change {
    if time(&task.created).is_some_and(|t| t > since) {
        Change::Created
    } else if task.done.unwrap_or_default() && time(&task.done_at).is_some_and(|t| t > since) {
        Change::Done
    } else {
        Change::Updated
    }
}

/// `30m`, `12h`, `3d` or `2w` ago, or an absolute date/time.
pub fn parse_since(input: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let input = input.trim();
    let split = input
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(input.len());
    let (num, unit) = input.split_at(split);

    if let Ok(n) = num.parse::<i64>() {
        let span = match unit {
            "m" | "min" => Some(Duration::minutes(n)),
            "h" => Some(Duration::hours(n)),
            "d" => Some(Duration::days(n)),
            "w" => Some(Duration::weeks(n)),
            _ => None,
        };
        if let Some(span) = span {
            return Ok(now - span);
        }
    }

    crate::parse_datetime(input).ok_or_else(|| {
        VkError::usage(format!(
            "Could not parse --since '{input}': use e.g. 30m, 12h, 3d, 2w, or a date like 2026-09-28"
        ))
    })
}

pub struct Changes {
    pub since: DateTime<Utc>,
    /// Most recently updated first.
    pub tasks: Vec<(Change, ModelsTask)>,
}

impl Changes {
    pub async fn fetch(
        api: &VikunjaAPI,
        since: DateTime<Utc>,
        project: Option<i32>,
    ) -> Result<Self> {
        let filter = format!("updated > '{}'", since.to_rfc3339());
        let mut tasks = api.get_all_tasks_filtered(Some(&filter)).await?;

        // Re-check locally rather than trusting the filter expression alone.
        tasks.retain(|t| time(&t.updated).is_some_and(|u| u > since));
        if let Some(project) = project {
            tasks.retain(|t| t.project_id == Some(project));
        }
        tasks.sort_by_key(|t| std::cmp::Reverse(time(&t.updated)));

        Ok(Changes {
            since,
            tasks: tasks
                .into_iter()
                .map(|t| (classify(&t, since), t))
                .collect(),
        })
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "since": self.since.to_rfc3339(),
            "changes": self
                .tasks
                .iter()
                .map(|(change, task)| json!({ "change": change, "task": task }))
                .collect::<Vec<_>>(),
        })
    }

    pub fn print(&self, projects: &[ModelsProject]) {
        let since_local = self.since.with_timezone(&Local).format("%Y-%m-%d %H:%M");
        let count = self.tasks.len();
        let noun = if count == 1 { "task" } else { "tasks" };
        println!("{count} {noun} changed since {since_local}");
        if count == 0 {
            return;
        }
        println!();

        let names: HashMap<i32, &str> = projects
            .iter()
            .filter_map(|p| Some((p.id?, p.title.as_deref()?)))
            .collect();
        let id_width = self
            .tasks
            .iter()
            .map(|(_, t)| t.id.unwrap_or_default().to_string().len() + 1)
            .max()
            .unwrap_or(2);

        for (change, task) in &self.tasks {
            let project = task
                .project_id
                .and_then(|id| names.get(&id).copied())
                .unwrap_or("");
            let when = time(&task.updated).map(time_relative).unwrap_or_default();
            println!(
                "  {:<8} {:>id_width$}  {}  · {project} · {when}",
                change.label(),
                format!("#{}", task.id.unwrap_or_default()),
                task.title.as_deref().unwrap_or(""),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, d, h, 0, 0).unwrap()
    }

    fn task(created: DateTime<Utc>, done_at: Option<DateTime<Utc>>) -> ModelsTask {
        ModelsTask {
            created: Some(created.to_rfc3339()),
            done: Some(done_at.is_some()),
            done_at: done_at.map(|t| t.to_rfc3339()),
            ..Default::default()
        }
    }

    #[test]
    fn test_classify() {
        let since = at(28, 12);
        assert_eq!(classify(&task(at(28, 13), None), since), Change::Created);
        // Created and finished in the window still reads as created.
        assert_eq!(
            classify(&task(at(28, 13), Some(at(28, 14))), since),
            Change::Created
        );
        assert_eq!(
            classify(&task(at(1, 0), Some(at(29, 9))), since),
            Change::Done
        );
        // Done before the window, touched since: an update.
        assert_eq!(
            classify(&task(at(1, 0), Some(at(20, 9))), since),
            Change::Updated
        );
        assert_eq!(classify(&task(at(1, 0), None), since), Change::Updated);
    }

    #[test]
    fn test_parse_since_relative() {
        let now = at(29, 12);
        assert_eq!(
            parse_since("30m", now).unwrap(),
            now - Duration::minutes(30)
        );
        assert_eq!(parse_since("12h", now).unwrap(), at(29, 0));
        assert_eq!(parse_since("3d", now).unwrap(), at(26, 12));
        assert_eq!(parse_since("1w", now).unwrap(), at(22, 12));
    }

    #[test]
    fn test_parse_since_absolute_and_invalid() {
        let now = at(29, 12);
        assert_eq!(parse_since("2026-09-28", now).unwrap(), at(28, 0));
        assert_eq!(parse_since("2026-09-28T06:00:00Z", now).unwrap(), at(28, 6));
        let err = parse_since("lately", now).unwrap_err();
        assert_eq!(err.kind, crate::error::Kind::Usage);
        assert!(parse_since("5y", now).is_err());
    }
}
