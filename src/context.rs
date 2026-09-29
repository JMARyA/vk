//! `vk context`: everything about one task in a single read, as markdown
//! for people and language models, or as JSON.

use chrono::Local;
use serde_json::{json, Value};
use vikunjars::models::{ModelsProject, ModelsTask, ModelsTaskComment, UserUser};

use crate::api::{Relation, VikunjaAPI};
use crate::description;
use crate::error::Result;
use crate::ui::parse_datetime;

pub struct Context {
    pub task: ModelsTask,
    pub project: Option<ModelsProject>,
    pub comments: Vec<ModelsTaskComment>,
}

impl Context {
    pub async fn fetch(api: &VikunjaAPI, task_id: i32) -> Result<Self> {
        let (task, comments) = tokio::join!(api.get_task(task_id), api.get_task_comments(task_id));
        let task = task?;
        let project = match task.project_id {
            // A project the token cannot read should not hide the task.
            Some(id) => api
                .get_project(&crate::api::ProjectID(id as isize))
                .await
                .ok(),
            None => None,
        };
        Ok(Context {
            task,
            project,
            comments: comments?,
        })
    }

    fn checklist(&self) -> Vec<(bool, String)> {
        description::parse_task_items(self.task.description.as_deref().unwrap_or(""))
    }

    /// Related tasks as (label, task), in a stable order.
    fn relations(&self) -> Vec<(String, &ModelsTask)> {
        let mut out: Vec<(String, &ModelsTask)> = self
            .task
            .related_tasks
            .iter()
            .flatten()
            .flat_map(|(kind, tasks)| {
                let label = Relation::try_parse(kind)
                    .map(|r| r.repr())
                    .unwrap_or_else(|| kind.clone());
                tasks.iter().map(move |t| (label.clone(), t))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        out
    }

    pub fn to_json(&self) -> Value {
        json!({
            "task": self.task,
            "project": self.project.as_ref().map(|p| json!({ "id": p.id, "title": p.title })),
            "description_markdown": description::html_to_markdown(
                self.task.description.as_deref().unwrap_or("")
            ),
            "checklist": self
                .checklist()
                .into_iter()
                .map(|(done, text)| json!({ "done": done, "text": text }))
                .collect::<Vec<_>>(),
            "relations": self
                .relations()
                .into_iter()
                .map(|(kind, t)| json!({
                    "kind": kind,
                    "task": { "id": t.id, "title": t.title, "done": t.done.unwrap_or_default() },
                }))
                .collect::<Vec<_>>(),
            "comments": self
                .comments
                .iter()
                .map(|c| json!({
                    "id": c.id,
                    "author": c.author.as_deref().map(user_name),
                    "created": c.created,
                    "markdown": description::html_to_markdown(c.comment.as_deref().unwrap_or("")),
                }))
                .collect::<Vec<_>>(),
        })
    }

    pub fn to_markdown(&self) -> String {
        let t = &self.task;
        let mut out = format!(
            "# #{} {}\n\n",
            t.id.unwrap_or_default(),
            t.title.as_deref().unwrap_or("")
        );

        let mut facts: Vec<String> = Vec::new();
        if let Some(p) = &self.project {
            facts.push(format!(
                "**Project:** {} (#{})",
                p.title.as_deref().unwrap_or(""),
                p.id.unwrap_or_default()
            ));
        }

        let mut status = vec![if t.done.unwrap_or_default() {
            "done".to_string()
        } else {
            "open".to_string()
        }];
        if let Some(p) = t.priority.filter(|p| *p > 0) {
            status.push(format!("priority {p}"));
        }
        if let Some(pct) = t.percent_done.filter(|p| *p > 0.0) {
            status.push(format!("{}% done", (pct * 100.0).round()));
        }
        if t.is_favorite.unwrap_or_default() {
            status.push("favorite".into());
        }
        facts.push(format!("**Status:** {}", status.join(" · ")));

        for (label, value) in [
            ("Start", &t.start_date),
            ("End", &t.end_date),
            ("Due", &t.due_date),
            ("Done at", &t.done_at),
        ] {
            if let Some(when) = local_time(value) {
                facts.push(format!("**{label}:** {when}"));
            }
        }

        let labels: Vec<&str> = t
            .labels
            .iter()
            .flatten()
            .filter_map(|l| l.title.as_deref())
            .map(str::trim)
            .collect();
        if !labels.is_empty() {
            facts.push(format!("**Labels:** {}", labels.join(", ")));
        }
        let assignees: Vec<String> = t.assignees.iter().flatten().map(user_name).collect();
        if !assignees.is_empty() {
            facts.push(format!("**Assignees:** {}", assignees.join(", ")));
        }

        let mut created = format!(
            "**Created:** {}",
            local_time(&t.created).unwrap_or_default()
        );
        if let Some(by) = t.created_by.as_deref() {
            created.push_str(&format!(" by {}", user_name(by)));
        }
        facts.push(created);
        // Kept verbatim: this is the value --expect-updated compares against.
        facts.push(format!(
            "**Updated:** {}",
            t.updated.as_deref().unwrap_or("")
        ));

        for fact in facts {
            out.push_str(&format!("- {fact}\n"));
        }

        let body = description::html_to_markdown(t.description.as_deref().unwrap_or(""));
        if !body.trim().is_empty() {
            out.push_str(&format!("\n## Description\n\n{}\n", body.trim()));
        }

        let checklist = self.checklist();
        if !checklist.is_empty() {
            let done = checklist.iter().filter(|(d, _)| *d).count();
            out.push_str(&format!("\n## Checklist ({done}/{})\n\n", checklist.len()));
            for (checked, text) in &checklist {
                out.push_str(&format!(
                    "- [{}] {text}\n",
                    if *checked { "x" } else { " " }
                ));
            }
        }

        let relations = self.relations();
        if !relations.is_empty() {
            out.push_str("\n## Relations\n\n");
            for (kind, rt) in relations {
                out.push_str(&format!(
                    "- {kind}: #{} {}{}\n",
                    rt.id.unwrap_or_default(),
                    rt.title.as_deref().unwrap_or(""),
                    if rt.done.unwrap_or_default() {
                        " (done)"
                    } else {
                        ""
                    }
                ));
            }
        }

        if !self.comments.is_empty() {
            out.push_str(&format!("\n## Comments ({})\n", self.comments.len()));
            for c in &self.comments {
                out.push_str(&format!(
                    "\n### {} · {}\n\n{}\n",
                    c.author.as_deref().map(user_name).unwrap_or_default(),
                    local_time(&c.created).unwrap_or_default(),
                    description::html_to_markdown(c.comment.as_deref().unwrap_or("")).trim()
                ));
            }
        }

        out
    }
}

fn user_name(u: &UserUser) -> String {
    u.username
        .clone()
        .filter(|n| !n.is_empty())
        .or_else(|| u.name.clone())
        .unwrap_or_default()
}

/// A Vikunja timestamp as local `YYYY-MM-DD HH:MM`; `None` when unset.
fn local_time(value: &Option<String>) -> Option<String> {
    let dt = parse_datetime(value.as_deref()?)?;
    Some(
        dt.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn user(name: &str) -> UserUser {
        UserUser {
            username: Some(name.into()),
            ..Default::default()
        }
    }

    fn context() -> Context {
        let blocker = ModelsTask {
            id: Some(7),
            title: Some("Get approval".into()),
            done: Some(true),
            ..Default::default()
        };
        Context {
            task: ModelsTask {
                id: Some(42),
                title: Some("Ship it".into()),
                description: Some(
                    r#"<p>Context here.</p><ul data-type="taskList"><li data-checked="true" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>build</p></div></li><li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>deploy</p></div></li></ul>"#
                        .into(),
                ),
                priority: Some(3),
                assignees: Some(vec![user("angelo")]),
                updated: Some("2026-09-29T10:00:00+02:00".into()),
                due_date: Some("0001-01-01T00:00:00Z".into()),
                related_tasks: Some(HashMap::from([("blocked".to_string(), vec![blocker])])),
                ..Default::default()
            },
            project: Some(ModelsProject {
                id: Some(3),
                title: Some("Work".into()),
                ..Default::default()
            }),
            comments: vec![ModelsTaskComment {
                id: Some(1),
                author: Some(Box::new(user("claude"))),
                comment: Some("<p>On it.</p>".into()),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn test_markdown_has_every_section() {
        let md = context().to_markdown();
        assert!(md.starts_with("# #42 Ship it\n"));
        assert!(md.contains("**Project:** Work (#3)"));
        assert!(md.contains("**Status:** open · priority 3"));
        assert!(md.contains("**Assignees:** angelo"));
        assert!(md.contains("**Updated:** 2026-09-29T10:00:00+02:00"));
        assert!(md.contains("## Checklist (1/2)\n\n- [x] build\n- [ ] deploy"));
        assert!(md.contains("- Blocked by: #7 Get approval (done)"));
        assert!(md.contains("## Comments (1)"));
        assert!(md.contains("### claude"));
        assert!(md.contains("On it."));
    }

    #[test]
    fn test_markdown_skips_unset_dates_and_empty_sections() {
        let mut ctx = context();
        ctx.task.description = None;
        ctx.task.related_tasks = None;
        ctx.comments.clear();
        let md = ctx.to_markdown();
        assert!(
            !md.contains("**Due:**"),
            "zero date must be treated as unset"
        );
        assert!(!md.contains("## Description"));
        assert!(!md.contains("## Relations"));
        assert!(!md.contains("## Comments"));
    }

    #[test]
    fn test_json_carries_parsed_sections() {
        let v = context().to_json();
        assert_eq!(v["task"]["id"], 42);
        assert_eq!(v["project"]["title"], "Work");
        assert_eq!(
            v["checklist"][1],
            json!({ "done": false, "text": "deploy" })
        );
        assert_eq!(v["relations"][0]["kind"], "Blocked by");
        assert_eq!(v["relations"][0]["task"]["done"], true);
        assert_eq!(v["comments"][0]["author"], "claude");
    }
}
