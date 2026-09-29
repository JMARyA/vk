use serde::{Deserialize, Serialize};

mod task;

pub use task::Relation;

use moka::sync::Cache;
use vikunjars::apis::configuration::ApiKey;
use vikunjars::apis::configuration::Configuration;
use vikunjars::apis::labels_api::LabelsPutError;
use vikunjars::apis::project_api::ProjectsGetError;
use vikunjars::apis::project_api::ProjectsIdGetError;
use vikunjars::apis::project_api::ProjectsPutError;
use vikunjars::apis::task_api::TasksGetError;
use vikunjars::apis::task_api::TasksIdGetError;
use vikunjars::apis::task_api::TasksTaskIdCommentsGetError;
use vikunjars::apis::task_api::TasksTaskIdCommentsPutError;
use vikunjars::apis::task_api::TasksTaskIdRelationsPutError;
use vikunjars::apis::user_api::UsersGetError;
use vikunjars::apis::Error;
use vikunjars::models::ModelsLabel;
use vikunjars::models::ModelsLabelTask;
use vikunjars::models::ModelsProject;
use vikunjars::models::ModelsTask;
use vikunjars::models::ModelsTaskComment;
use vikunjars::models::ModelsTaskRelation;
use vikunjars::models::UserUser;

use crate::error::{Result, VkError};

// Hand-rolled models predating the generated `vikunjars` client, which now
// supplies the equivalents. Unused, kept until their removal is confirmed.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VikunjaError {
    pub code: Option<isize>,
    pub message: String,
}

impl VikunjaError {}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: usize,
    pub title: String,
    pub description: String,
    pub hex_color: String,
    pub created_by: User,
    pub updated: String,
    pub created: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: usize,
    pub name: String,
    pub username: String,
    pub created: String,
    pub updated: String,
}

/// Collect every page. A failed page is an error rather than the end of the
/// list, so callers never mistake a truncated result for a complete one.
pub async fn get_all_items<F, T, E>(mut get_page: F) -> std::result::Result<Vec<T>, E>
where
    F: AsyncFnMut(usize) -> std::result::Result<Vec<T>, E>,
{
    let mut ret = Vec::new();
    let mut page = 1;
    loop {
        let current_page = get_page(page).await?;
        if current_page.is_empty() {
            break;
        }
        ret.extend(current_page);
        page += 1;
    }
    Ok(ret)
}

pub struct ProjectID(pub isize);

impl ProjectID {
    /// Resolve a project by id (`12` or `#12`) or title. A title must match
    /// exactly (ignoring case), or be part of exactly one project's title.
    pub async fn parse(api: &VikunjaAPI, project: &str) -> Result<Self> {
        let project = project.trim_start_matches('#');

        if let Ok(num) = project.parse() {
            return Ok(Self(num));
        }

        let projects = api.get_all_projects().await?;
        let titled: Vec<(i32, &str)> = projects
            .iter()
            .filter_map(|p| Some((p.id?, p.title.as_deref()?)))
            .collect();
        resolve_by_title(&titled, project, "project").map(|id| Self(id as isize))
    }
}

/// Pick the id whose title equals `wanted` (ignoring case), else the single
/// title containing it. Several partial matches are an error listing them,
/// rather than a guess.
pub fn resolve_by_title(items: &[(i32, &str)], wanted: &str, what: &str) -> Result<i32> {
    let wanted_lc = wanted.trim().to_lowercase();

    if let Some((id, _)) = items
        .iter()
        .find(|(_, t)| t.trim().to_lowercase() == wanted_lc)
    {
        return Ok(*id);
    }

    let partial: Vec<&(i32, &str)> = items
        .iter()
        .filter(|(_, t)| t.to_lowercase().contains(&wanted_lc))
        .collect();

    match partial.as_slice() {
        [(id, _)] => Ok(*id),
        [] => Err(VkError::not_found(format!("No {what} matches '{wanted}'"))),
        many => {
            let names: Vec<String> = many.iter().map(|(id, t)| format!("{t} (#{id})")).collect();
            Err(VkError::usage(format!(
                "'{wanted}' matches several {what}s: {}. Use the exact title or the id.",
                names.join(", ")
            )))
        }
    }
}

pub struct VikunjaAPI {
    // Allocated but never read — no call site caches anything yet.
    #[allow(dead_code)]
    cache: Cache<String, String>,
    configuration: vikunjars::apis::configuration::Configuration,
}

impl VikunjaAPI {
    pub fn new(host: &str, token: &str) -> Self {
        let base_path = format!("{host}/api/v1");
        Self {
            cache: Cache::new(100),
            configuration: Configuration {
                base_path,
                user_agent: Some(format!("vk-{}", env!("CARGO_PKG_VERSION"))),
                client: reqwest::Client::new(),
                basic_auth: None,
                oauth_access_token: None,
                bearer_access_token: None,
                api_key: Some(ApiKey {
                    prefix: Some("Bearer".to_string()),
                    key: token.to_string(),
                }),
            },
        }
    }

    // projects

    #[allow(dead_code)]
    pub async fn get_project_name_from_id(&self, id: isize) -> String {
        let all_prj = self.get_all_projects().await.unwrap();

        let found = all_prj
            .into_iter()
            .find(|x| x.id.unwrap() == id as i32)
            .unwrap();

        found.title.unwrap_or_default()
    }

    pub async fn get_all_projects(
        &self,
    ) -> Result<Vec<vikunjars::models::ModelsProject>, Error<ProjectsGetError>> {
        vikunjars::apis::project_api::projects_get(
            &self.configuration,
            None,
            None,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn delete_project(&self, project_id: &ProjectID) -> Result<()> {
        vikunjars::apis::project_api::projects_id_delete(&self.configuration, project_id.0 as i32)
            .await?;
        Ok(())
    }

    pub async fn new_project(
        &self,
        title: &str,
        description: Option<String>,
        color: Option<String>,
        parent: Option<ProjectID>,
    ) -> Result<ModelsProject, vikunjars::apis::Error<ProjectsPutError>> {
        let data = ModelsProject {
            description,
            hex_color: color,
            parent_project_id: parent.map(|x| x.0 as i32),
            title: Some(title.to_string()),
            ..Default::default()
        };

        vikunjars::apis::project_api::projects_put(&self.configuration, data).await
    }

    #[allow(dead_code)]
    pub async fn get_project(
        &self,
        project: &ProjectID,
    ) -> Result<ModelsProject, vikunjars::apis::Error<ProjectsIdGetError>> {
        vikunjars::apis::project_api::projects_id_get(&self.configuration, project.0 as i32).await
    }

    // labels
    pub async fn get_all_labels(&self) -> Result<Vec<ModelsLabel>> {
        Ok(get_all_items(async |x| {
            vikunjars::apis::labels_api::labels_get(&self.configuration, Some(x as i32), None, None)
                .await
        })
        .await?)
    }

    /// Find a label by its exact (trimmed) title.
    pub async fn find_label(&self, title: &str) -> Result<ModelsLabel> {
        self.get_all_labels()
            .await?
            .into_iter()
            .find(|x| x.title.as_deref().unwrap_or("").trim() == title.trim())
            .ok_or_else(|| VkError::not_found(format!("Label '{title}' not found")))
    }

    pub async fn new_label(
        &self,
        title: &str,
        description: Option<String>,
        color: Option<String>,
    ) -> Result<ModelsLabel, vikunjars::apis::Error<LabelsPutError>> {
        let label = ModelsLabel {
            title: Some(title.to_string()),
            description,
            hex_color: color,
            ..Default::default()
        };
        vikunjars::apis::labels_api::labels_put(&self.configuration, label).await
    }

    pub async fn remove_label(&self, title: &str) -> Result<ModelsLabel> {
        let label = self.find_label(title).await?;
        vikunjars::apis::labels_api::labels_id_delete(
            &self.configuration,
            label.id.unwrap_or_default(),
        )
        .await?;
        Ok(label)
    }

    pub async fn label_task_remove(&self, label: &str, task_id: i32) -> Result<()> {
        let label = self.find_label(label).await?;
        vikunjars::apis::labels_api::tasks_task_labels_label_delete(
            &self.configuration,
            task_id,
            label.id.unwrap_or_default(),
        )
        .await?;
        Ok(())
    }

    pub async fn label_task(&self, label: &str, task_id: i32) -> Result<ModelsLabelTask> {
        let label = self.find_label(label).await?;
        Ok(vikunjars::apis::labels_api::tasks_task_labels_put(
            &self.configuration,
            task_id,
            ModelsLabelTask {
                label_id: label.id,
                ..Default::default()
            },
        )
        .await?)
    }

    // tasks
    pub async fn get_task_page(
        &self,
        page: usize,
    ) -> Result<Vec<ModelsTask>, vikunjars::apis::Error<TasksGetError>> {
        vikunjars::apis::task_api::tasks_get(
            &self.configuration,
            Some(page as i32),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn get_all_tasks(&self) -> Result<Vec<ModelsTask>> {
        Ok(get_all_items(async |x| self.get_task_page(x).await).await?)
    }

    /// Fetch every task matching `filter`, walking all pages.
    ///
    /// Unlike [`Self::get_all_tasks`] this surfaces request failures instead of
    /// silently returning a short list, so callers that need a complete set
    /// (such as the local sync) can tell truncation apart from an empty result.
    pub async fn get_all_tasks_filtered(
        &self,
        filter: Option<&str>,
    ) -> Result<Vec<ModelsTask>, vikunjars::apis::Error<TasksGetError>> {
        let mut ret = Vec::new();
        let mut page = 1;

        loop {
            let items = vikunjars::apis::task_api::tasks_get(
                &self.configuration,
                Some(page),
                None,
                None,
                None,
                None,
                filter,
                None,
                None,
                None,
            )
            .await?;

            if items.is_empty() {
                break;
            }

            ret.extend(items);
            page += 1;
        }

        Ok(ret)
    }

    pub async fn get_latest_tasks(
        &self,
    ) -> Result<Vec<ModelsTask>, vikunjars::apis::Error<TasksGetError>> {
        vikunjars::apis::task_api::tasks_get(
            &self.configuration,
            None,
            Some(25),
            None,
            Some("created"),
            Some("desc"),
            None,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn get_task(
        &self,
        id: i32,
    ) -> Result<ModelsTask, vikunjars::apis::Error<TasksIdGetError>> {
        vikunjars::apis::task_api::tasks_id_get(&self.configuration, id, None).await
    }

    pub async fn delete_task(&self, id: i32) -> Result<()> {
        vikunjars::apis::task_api::tasks_id_delete(&self.configuration, id).await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new_task(
        &self,
        title: &str,
        project: &ProjectID,
        description: Option<String>,
        due_date: Option<String>,
        fav: bool,
        label: Option<String>,
        priority: Option<i32>,
    ) -> Result<ModelsTask> {
        let id = project.0;

        let labels = if let Some(label) = label {
            vec![self.find_label(&label).await?]
        } else {
            vec![]
        };

        let data = ModelsTask {
            title: Some(title.to_string()),
            description,
            due_date,
            is_favorite: Some(fav),
            priority,
            labels: Some(labels),
            ..Default::default()
        };

        Ok(
            vikunjars::apis::task_api::projects_id_tasks_put(&self.configuration, id as i32, data)
                .await?,
        )
    }

    pub async fn edit_task(
        &self,
        task_id: i32,
        title: Option<String>,
        description: Option<String>,
        due_date: Option<String>,
        priority: Option<i32>,
    ) -> Result<ModelsTask> {
        let existing = self.get_task(task_id).await?;
        let task = ModelsTask {
            title: title.or(existing.title.clone()),
            description: description.or(existing.description.clone()),
            due_date: due_date.or(existing.due_date.clone()),
            priority: priority.or(existing.priority),
            ..existing
        };
        Ok(vikunjars::apis::task_api::tasks_id_post(&self.configuration, task_id, task).await?)
    }

    pub async fn done_task(&self, task_id: i32, done: bool) -> Result<ModelsTask> {
        let existing = self.get_task(task_id).await?;
        let task = ModelsTask {
            done: Some(done),
            done_at: if done {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                existing.done_at.clone()
            },
            ..existing
        };
        Ok(vikunjars::apis::task_api::tasks_id_post(&self.configuration, task_id, task).await?)
    }

    pub async fn fav_task(&self, task_id: i32, fav: bool) -> Result<ModelsTask> {
        let existing = self.get_task(task_id).await?;
        let task = ModelsTask {
            is_favorite: Some(fav),
            ..existing
        };
        Ok(vikunjars::apis::task_api::tasks_id_post(&self.configuration, task_id, task).await?)
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        totp: Option<String>,
    ) -> Result<String> {
        let credentials = vikunjars::models::UserLogin {
            long_token: Some(true),
            password: Some(password.to_string()),
            totp_passcode: totp,
            username: Some(username.to_string()),
        };
        let ret = vikunjars::apis::auth_api::login_post(&self.configuration, credentials).await?;
        ret.token
            .ok_or_else(|| VkError::other("The server accepted the login but sent no token"))
    }

    pub async fn search_user(
        &self,
        search: &str,
    ) -> Result<Vec<UserUser>, vikunjars::apis::Error<UsersGetError>> {
        vikunjars::apis::user_api::users_get(&self.configuration, Some(search)).await
    }

    /// Resolve a username to a user id. Prefers an exact username match
    /// over the search's first hit.
    pub async fn find_user_id(&self, user: &str) -> Result<i32> {
        let found = self.search_user(user).await?;
        found
            .iter()
            .find(|u| u.username.as_deref() == Some(user))
            .or(found.first())
            .and_then(|u| u.id)
            .ok_or_else(|| VkError::not_found(format!("User '{user}' not found")))
    }

    pub async fn assign_user_id(&self, user_id: i32, task_id: i32) -> Result<()> {
        let assignee = vikunjars::models::ModelsTaskAssginee {
            user_id: Some(user_id),
            ..Default::default()
        };
        vikunjars::apis::assignees_api::tasks_task_id_assignees_put(
            &self.configuration,
            task_id,
            assignee,
        )
        .await?;
        Ok(())
    }

    pub async fn unassign_user_id(&self, user_id: i32, task_id: i32) -> Result<()> {
        vikunjars::apis::assignees_api::tasks_task_id_assignees_user_id_delete(
            &self.configuration,
            task_id,
            user_id,
        )
        .await?;
        Ok(())
    }

    pub async fn assign_to_task(&self, user: &str, task_id: i32) -> Result<()> {
        let user_id = self.find_user_id(user).await?;
        self.assign_user_id(user_id, task_id).await
    }

    pub async fn remove_assign_to_task(&self, user: &str, task_id: i32) -> Result<()> {
        let user_id = self.find_user_id(user).await?;
        self.unassign_user_id(user_id, task_id).await
    }

    pub async fn get_task_comments(
        &self,
        task_id: i32,
    ) -> Result<Vec<ModelsTaskComment>, vikunjars::apis::Error<TasksTaskIdCommentsGetError>> {
        vikunjars::apis::task_api::tasks_task_id_comments_get(&self.configuration, task_id, None)
            .await
    }

    pub async fn remove_relation(
        &self,
        task_id: i32,
        relation: &Relation,
        other_task_id: i32,
    ) -> Result<()> {
        let rel = ModelsTaskRelation {
            other_task_id: Some(other_task_id),
            relation_kind: Some(relation.model_rel()),
            task_id: Some(task_id),
            ..Default::default()
        };
        vikunjars::apis::task_api::tasks_task_id_relations_relation_kind_other_task_id_delete(
            &self.configuration,
            task_id,
            &relation.api(),
            other_task_id,
            rel,
        )
        .await?;
        Ok(())
    }

    pub async fn add_relation(
        &self,
        task_id: i32,
        relation: &Relation,
        other_task_id: i32,
    ) -> Result<ModelsTaskRelation, vikunjars::apis::Error<TasksTaskIdRelationsPutError>> {
        let relation = ModelsTaskRelation {
            task_id: Some(task_id),
            other_task_id: Some(other_task_id),
            relation_kind: Some(relation.model_rel()),
            ..Default::default()
        };
        vikunjars::apis::task_api::tasks_task_id_relations_put(
            &self.configuration,
            task_id,
            relation,
        )
        .await
    }

    pub async fn new_comment(
        &self,
        task_id: i32,
        comment: String,
    ) -> Result<ModelsTaskComment, vikunjars::apis::Error<TasksTaskIdCommentsPutError>> {
        let relation = ModelsTaskComment {
            comment: Some(comment),
            ..Default::default()
        };
        vikunjars::apis::task_api::tasks_task_id_comments_put(
            &self.configuration,
            task_id,
            relation,
        )
        .await
    }
}
