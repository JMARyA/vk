//! `vk claim` / `vk unclaim`: take a task by assigning yourself, so several
//! agents (or an agent and a person) can share a board without doing the
//! same work twice.
//!
//! Claiming is not atomic on the server: two claims at the same moment can
//! both assign themselves. After assigning, the task is read back. If a
//! rival appeared meanwhile, the claimant with the higher user id backs
//! off, so exactly one of them keeps it.

use vikunjars::models::ModelsTask;

use crate::api::VikunjaAPI;
use crate::config::Config;
use crate::error::{Kind, Result, VkError};

/// (user id, display name) of each assignee.
pub fn assignees(task: &ModelsTask) -> Vec<(i32, String)> {
    task.assignees
        .iter()
        .flatten()
        .filter_map(|u| {
            let name = u
                .username
                .clone()
                .filter(|n| !n.is_empty())
                .or_else(|| u.name.clone())
                .unwrap_or_default();
            Some((u.id?, name))
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Already assigned to us; nothing to do.
    AlreadyMine,
    /// Assign ourselves.
    Assign,
}

/// Whether `me` may claim `task`.
pub fn decide(task: &ModelsTask, me: i32, force: bool) -> Result<Decision> {
    let id = task.id.unwrap_or_default();
    if task.done.unwrap_or_default() {
        return Err(VkError::new(
            Kind::Conflict,
            format!("Task #{id} is already done"),
        ));
    }

    let current = assignees(task);
    if current.iter().any(|(uid, _)| *uid == me) {
        return Ok(Decision::AlreadyMine);
    }
    if !current.is_empty() && !force {
        let names: Vec<String> = current
            .iter()
            .map(|(uid, name)| format!("{name} (#{uid})"))
            .collect();
        return Err(VkError::new(
            Kind::Conflict,
            format!(
                "Task #{id} is claimed by {}. Use --force to claim it alongside them.",
                names.join(", ")
            ),
        ));
    }
    Ok(Decision::Assign)
}

/// After assigning ourselves: did a rival claim at the same moment and win?
/// `before` are the assignees seen before claiming (kept with --force).
/// Among newcomers the lowest user id wins, so every racer agrees.
pub fn lost_race(before: &[(i32, String)], after: &ModelsTask, me: i32) -> Option<(i32, String)> {
    assignees(after)
        .into_iter()
        .filter(|(uid, _)| *uid != me && !before.iter().any(|(b, _)| b == uid))
        .filter(|(uid, _)| *uid < me)
        .min_by_key(|(uid, _)| *uid)
}

/// The user id vk acts as: `VK_USER_ID`, else `user_id` in the config, else
/// whoever the login belongs to. API tokens cannot read the latter, so they
/// need one of the first two.
pub async fn resolve_me(api: &VikunjaAPI, config: &Config) -> Result<i32> {
    if let Ok(value) = std::env::var("VK_USER_ID") {
        return value
            .trim()
            .parse()
            .map_err(|_| VkError::usage(format!("VK_USER_ID must be a number, got '{value}'")));
    }
    if let Some(id) = config.user_id {
        return Ok(id);
    }

    match api.current_user().await {
        Ok(user) => user
            .id
            .ok_or_else(|| VkError::other("The server did not say which user you are")),
        Err(e) if e.kind == Kind::Auth => Err(VkError::usage(
            "Could not tell which user you are: this login (probably an API token) cannot read \
             /user. Set `user_id = <your id>` in ~/.config/vk.toml or VK_USER_ID. Your id is \
             `created_by.id` on a task you created: `vk info <id> --json`.",
        )),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vikunjars::models::UserUser;

    fn user(id: i32, name: &str) -> UserUser {
        UserUser {
            id: Some(id),
            username: Some(name.into()),
            ..Default::default()
        }
    }

    fn task(assigned: &[(i32, &str)]) -> ModelsTask {
        ModelsTask {
            id: Some(42),
            assignees: Some(assigned.iter().map(|(id, n)| user(*id, n)).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn test_free_task_can_be_claimed() {
        assert_eq!(decide(&task(&[]), 5, false).unwrap(), Decision::Assign);
        let unassigned = ModelsTask {
            assignees: None,
            ..task(&[])
        };
        assert_eq!(decide(&unassigned, 5, false).unwrap(), Decision::Assign);
    }

    #[test]
    fn test_claiming_twice_is_a_no_op() {
        assert_eq!(
            decide(&task(&[(5, "bot")]), 5, false).unwrap(),
            Decision::AlreadyMine
        );
    }

    #[test]
    fn test_someone_elses_task_is_a_conflict_unless_forced() {
        let err = decide(&task(&[(1, "angelo")]), 5, false).unwrap_err();
        assert_eq!(err.kind, Kind::Conflict);
        assert!(err.message.contains("angelo (#1)"));
        assert_eq!(
            decide(&task(&[(1, "angelo")]), 5, true).unwrap(),
            Decision::Assign
        );
    }

    #[test]
    fn test_done_task_cannot_be_claimed() {
        let done = ModelsTask {
            done: Some(true),
            ..task(&[])
        };
        assert_eq!(decide(&done, 5, true).unwrap_err().kind, Kind::Conflict);
    }

    #[test]
    fn test_race_lowest_newcomer_wins() {
        let before = vec![];
        // A rival with a lower id arrived at the same time: we back off.
        let after = task(&[(3, "other"), (5, "me")]);
        assert_eq!(lost_race(&before, &after, 5), Some((3, "other".into())));
        // A rival with a higher id: we keep it, they back off.
        let after = task(&[(5, "me"), (8, "other")]);
        assert_eq!(lost_race(&before, &after, 5), None);
    }

    #[test]
    fn test_race_ignores_assignees_that_were_already_there() {
        // With --force, pre-existing assignees are not rivals.
        let before = vec![(1, "angelo".to_string())];
        let after = task(&[(1, "angelo"), (5, "me")]);
        assert_eq!(lost_race(&before, &after, 5), None);
    }
}
