//! `--dry-run` reports: what a command would do, without doing it.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{json, Value};
use vikunjars::models::ModelsTask;

use crate::error::{Result, VkError};

/// One field that would change.
#[derive(Debug, PartialEq, Serialize)]
pub struct Change {
    pub from: Value,
    pub to: Value,
}

/// Fields that differ between two versions of a task, by JSON field name.
pub fn task_diff(before: &ModelsTask, after: &ModelsTask) -> BTreeMap<String, Change> {
    let to_map = |t: &ModelsTask| match serde_json::to_value(t) {
        Ok(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    let (before, after) = (to_map(before), to_map(after));

    before
        .keys()
        .chain(after.keys())
        .filter_map(|key| {
            let from = before.get(key).cloned().unwrap_or(Value::Null);
            let to = after.get(key).cloned().unwrap_or(Value::Null);
            (from != to).then(|| (key.clone(), Change { from, to }))
        })
        .collect()
}

/// A value for a one-line human summary: strings unquoted and shortened.
fn short(value: &Value) -> String {
    const MAX: usize = 60;
    let text = match value {
        Value::Null => return "(none)".into(),
        Value::String(s) => s.replace('\n', " "),
        other => other.to_string(),
    };
    if text.chars().count() > MAX {
        format!("{}…", text.chars().take(MAX - 1).collect::<String>())
    } else {
        text
    }
}

fn print(body: &Value) -> Result<()> {
    let line = serde_json::to_string(body)
        .map_err(|e| VkError::other(format!("Could not encode output: {e}")))?;
    println!("{line}");
    Ok(())
}

/// Report a task update that was not sent.
pub fn report_update(
    action: &str,
    before: &ModelsTask,
    after: &ModelsTask,
    json: bool,
) -> Result<()> {
    let id = before.id.unwrap_or_default();
    let changes = task_diff(before, after);

    if json {
        return print(&json!({
            "dry_run": true,
            "action": action,
            "task": id,
            "changes": changes,
        }));
    }

    if changes.is_empty() {
        println!("Would {action} task #{id}: nothing would change.");
        return Ok(());
    }
    println!("Would {action} task #{id}:");
    for (field, change) in &changes {
        println!("  {field}: {} → {}", short(&change.from), short(&change.to));
    }
    Ok(())
}

/// Report any other action that was not carried out. `details` must be a
/// JSON object; its fields are added to the JSON report.
pub fn report(action: &str, details: Value, summary: &str, json: bool) -> Result<()> {
    if json {
        let mut body = json!({ "dry_run": true, "action": action });
        if let (Value::Object(body), Value::Object(details)) = (&mut body, details) {
            body.extend(details);
        }
        return print(&body);
    }
    println!("Would {summary}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> ModelsTask {
        ModelsTask {
            id: Some(7),
            title: Some("old".into()),
            priority: Some(1),
            ..Default::default()
        }
    }

    #[test]
    fn test_diff_lists_only_changed_fields() {
        let after = ModelsTask {
            title: Some("new".into()),
            ..task()
        };
        let diff = task_diff(&task(), &after);
        assert_eq!(diff.len(), 1);
        assert_eq!(
            diff["title"],
            Change {
                from: json!("old"),
                to: json!("new"),
            }
        );
    }

    #[test]
    fn test_diff_sees_fields_that_appear_or_vanish() {
        let after = ModelsTask {
            due_date: Some("2026-10-01T00:00:00Z".into()),
            priority: None,
            ..task()
        };
        let diff = task_diff(&task(), &after);
        assert_eq!(diff["due_date"].from, Value::Null);
        assert_eq!(diff["priority"].to, Value::Null);
    }

    #[test]
    fn test_identical_tasks_have_no_diff() {
        assert!(task_diff(&task(), &task()).is_empty());
    }

    #[test]
    fn test_short_trims_long_values() {
        assert_eq!(short(&Value::Null), "(none)");
        assert_eq!(short(&json!("a\nb")), "a b");
        assert_eq!(short(&json!(3)), "3");
        let long = short(&json!("x".repeat(100)));
        assert_eq!(long.chars().count(), 60);
        assert!(long.ends_with('…'));
    }
}
