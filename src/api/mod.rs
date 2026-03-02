use serde::{Deserialize, Serialize};

mod task;

pub use task::Relation;

use moka::sync::Cache;
use vikunjars::apis::configuration::ApiKey;
use vikunjars::apis::configuration::Configuration;
use vikunjars::apis::labels_api::LabelsPutError;
use vikunjars::apis::labels_api::TasksTaskLabelsPutError;
use vikunjars::apis::project_api::ProjectsGetError;
use vikunjars::apis::project_api::ProjectsIdGetError;
use vikunjars::apis::project_api::ProjectsPutError;
use vikunjars::apis::task_api::TasksGetError;
use vikunjars::apis::task_api::TasksIdGetError;
use vikunjars::apis::task_api::TasksIdPostError;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VikunjaError {
    pub code: Option<isize>,
    pub message: String,
}

impl VikunjaError {}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: usize,
    pub name: String,
    pub username: String,
    pub created: String,
    pub updated: String,
}

pub async fn get_all_items<F, T, E>(mut get_page: F) -> Vec<T>
where
    F: AsyncFnMut(usize) -> Result<Vec<T>, E>,
{
    let mut ret = Vec::new();
    let mut page = 1;
    loop {
        let current_page = get_page(page).await;
        if let Ok(current_page) = current_page {
            if current_page.is_empty() {
                break;
            }
            ret.extend(current_page);
        } else {
            break;
        }
        page += 1;
    }
    ret
}

pub struct ProjectID(pub isize);

impl ProjectID {
    pub async fn parse(api: &VikunjaAPI, project: &str) -> Option<Self> {
        let project = project.trim_start_matches('#');

        if let Ok(num) = project.parse() {
            Some(Self(num))
        } else {
            Some(Self(
                api.get_all_projects()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|x| {
                        x.title
                            .as_ref()
                            .unwrap()
                            .to_lowercase()
                            .contains(&project.to_lowercase())
                    })?
                    .id
                    .unwrap() as isize,
            ))
        }
    }
}

pub struct VikunjaAPI {
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

    pub async fn get_project_name_from_id(&self, id: isize) -> String {
        let all_prj = self.get_all_projects().await.unwrap();

        let found = all_prj
            .into_iter()
            .find(|x| x.id.unwrap() == id as i32)
            .unwrap();

        found.title.unwrap()
    }

    pub async fn get_all_projects(
        &self,
    ) -> Result<Vec<vikunjars::models::ModelsProject>, Error<ProjectsGetError>> {
        vikunjars::apis::project_api::projects_get(&self.configuration, None, None, None, None, None)
            .await
    }

    pub async fn delete_project(&self, project_id: &ProjectID) {
        vikunjars::apis::project_api::projects_id_delete(&self.configuration, project_id.0 as i32)
            .await
            .unwrap();
    }

    pub async fn new_project(
        &self,
        title: &str,
        description: Option<String>,
        color: Option<String>,
        parent: Option<ProjectID>,
    ) -> Result<ModelsProject, vikunjars::apis::Error<ProjectsPutError>> {
        let mut data = ModelsProject::default();

        data.description = description;
        data.hex_color = color;
        data.parent_project_id = parent.map(|x| x.0 as i32);
        data.title = Some(title.to_string());

        vikunjars::apis::project_api::projects_put(&self.configuration, data).await
    }

    pub async fn get_project(
        &self,
        project: &ProjectID,
    ) -> Result<ModelsProject, vikunjars::apis::Error<ProjectsIdGetError>> {
        vikunjars::apis::project_api::projects_id_get(&self.configuration, project.0 as i32).await
    }

    // labels
    pub async fn get_all_labels(&self) -> Vec<ModelsLabel> {
        get_all_items(async |x| {
            vikunjars::apis::labels_api::labels_get(&self.configuration, Some(x as i32), None, None)
                .await
        })
        .await
    }

    pub async fn new_label(
        &self,
        title: &str,
        description: Option<String>,
        color: Option<String>,
    ) -> Result<ModelsLabel, vikunjars::apis::Error<LabelsPutError>> {
        let label = ModelsLabel {
            title: Some(title.to_string()),
            description: description,
            hex_color: color,
            ..Default::default()
        };
        vikunjars::apis::labels_api::labels_put(&self.configuration, label).await
    }

    pub async fn remove_label(&self, title: &str) {
        let labels = self.get_all_labels();

        let label_id = labels
            .await
            .into_iter()
            .find(|x| x.title.as_ref().unwrap().trim() == title)
            .unwrap()
            .id
            .unwrap();

        vikunjars::apis::labels_api::labels_id_delete(&self.configuration, label_id)
            .await
            .unwrap();
    }

    pub async fn label_task_remove(&self, label: &str, task_id: i32) {
        let labels = self.get_all_labels().await;

        let label_id = labels
            .into_iter()
            .find(|x| x.title.as_ref().unwrap().trim() == label)
            .unwrap()
            .id
            .unwrap();

        vikunjars::apis::labels_api::tasks_task_labels_label_delete(
            &self.configuration,
            task_id,
            label_id,
        )
        .await
        .unwrap();
    }

    pub async fn label_task(
        &self,
        label: &str,
        task_id: i32,
    ) -> Result<ModelsLabelTask, vikunjars::apis::Error<TasksTaskLabelsPutError>> {
        let labels = self.get_all_labels().await;

        let label_id = labels
            .into_iter()
            .find(|x| x.title.as_ref().unwrap().trim() == label)
            .unwrap()
            //.map_or_else(|| Err(format!("Label '{label}' not found")), Ok)?
            .id
            .unwrap();

        vikunjars::apis::labels_api::tasks_task_labels_put(
            &self.configuration,
            task_id,
            ModelsLabelTask {
                label_id: Some(label_id),
                ..Default::default()
            },
        )
        .await
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

    pub async fn get_all_tasks(&self) -> Vec<ModelsTask> {
        get_all_items(async |x| self.get_task_page(x).await).await
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

    pub async fn delete_task(&self, id: i32) {
        vikunjars::apis::task_api::tasks_id_delete(&self.configuration, id)
            .await
            .unwrap();
    }

    pub async fn new_task(
        &self,
        title: &str,
        project: &ProjectID,
        description: Option<String>,
        due_date: Option<String>,
        fav: bool,
        label: Option<String>,
        priority: Option<i32>,
    ) -> Result<ModelsTask, String> {
        let id = project.0;

        let labels = if let Some(label) = label {
            let label = self
                .get_all_labels()
                .await
                .into_iter()
                .find(|x| x.title.as_ref().unwrap().trim() == label)
                .map_or_else(|| Err(format!("Label '{label}' not found")), Ok)?;
            vec![label]
        } else {
            vec![]
        };

        let data = ModelsTask {
            title: Some(title.to_string()),
            description: description,
            due_date: due_date,
            is_favorite: Some(fav),
            priority: priority,
            labels: Some(labels),
            ..Default::default()
        };

        let ret =
            vikunjars::apis::task_api::projects_id_tasks_put(&self.configuration, id as i32, data)
                .await
                .map_err(|e| format!("API Error: {e}"))?;

        Ok(ret)
    }

    pub async fn edit_task(
        &self,
        task_id: i32,
        title: Option<String>,
        description: Option<String>,
        due_date: Option<String>,
        priority: Option<i32>,
    ) -> Result<ModelsTask, Error<TasksIdPostError>> {
        let existing = self
            .get_task(task_id)
            .await
            .expect("Failed to fetch task before editing");
        let task = ModelsTask {
            title: title.or(existing.title.clone()),
            description: description.or(existing.description.clone()),
            due_date: due_date.or(existing.due_date.clone()),
            priority: priority.or(existing.priority),
            ..existing
        };
        vikunjars::apis::task_api::tasks_id_post(&self.configuration, task_id, task).await
    }

    pub async fn done_task(
        &self,
        task_id: i32,
        done: bool,
    ) -> Result<ModelsTask, Error<TasksIdPostError>> {
        let task = ModelsTask {
            done: Some(done),
            done_at: if done {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                None
            },
            ..Default::default()
        };
        vikunjars::apis::task_api::tasks_id_post(&self.configuration, task_id, task).await
    }

    pub async fn fav_task(
        &self,
        task_id: i32,
        fav: bool,
    ) -> Result<ModelsTask, Error<TasksIdPostError>> {
        let task = ModelsTask {
            is_favorite: Some(fav),
            ..Default::default()
        };
        vikunjars::apis::task_api::tasks_id_post(&self.configuration, task_id, task).await
    }

    pub async fn login(&self, username: &str, password: &str, totp: Option<String>) -> String {
        let credentials = vikunjars::models::UserLogin {
            long_token: Some(true),
            password: Some(password.to_string()),
            totp_passcode: totp,
            username: Some(username.to_string()),
        };
        let ret = vikunjars::apis::auth_api::login_post(&self.configuration, credentials)
            .await
            .unwrap();
        ret.token.unwrap()
    }

    pub async fn search_user(
        &self,
        search: &str,
    ) -> Result<Vec<UserUser>, vikunjars::apis::Error<UsersGetError>> {
        vikunjars::apis::user_api::users_get(&self.configuration, Some(search)).await
    }

    pub async fn assign_to_task(&self, user: &str, task_id: i32) -> Result<(), String> {
        let user = self
            .search_user(user)
            .await
            .map_err(|_| String::from("User not found"))?;

        let assignee = vikunjars::models::ModelsTaskAssginee {
            user_id: Some(user.first().unwrap().id.unwrap()),
            ..Default::default()
        };
        vikunjars::apis::assignees_api::tasks_task_id_assignees_put(
            &self.configuration,
            task_id,
            assignee,
        )
        .await
        .map_err(|e| format!("API Error: {e}"))?;

        Ok(())
    }

    pub async fn remove_assign_to_task(&self, user: &str, task_id: i32) {
        let user = self.search_user(user).await.unwrap();
        let user_id = user.first().unwrap().id.unwrap();
        vikunjars::apis::assignees_api::tasks_task_id_assignees_user_id_delete(
            &self.configuration,
            task_id,
            user_id,
        )
        .await
        .unwrap();
    }

    pub async fn get_task_comments(
        &self,
        task_id: i32,
    ) -> Result<Vec<ModelsTaskComment>, vikunjars::apis::Error<TasksTaskIdCommentsGetError>> {
        vikunjars::apis::task_api::tasks_task_id_comments_get(&self.configuration, task_id, None).await
    }

    pub async fn remove_relation(&self, task_id: i32, relation: &Relation, other_task_id: i32) {
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
        .await
        .unwrap();
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
            task_id as i32,
            relation,
        )
        .await
    }
}
