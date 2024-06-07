use serde::{Deserialize, Serialize};

mod project;
mod task;

pub use project::Project;
pub use task::Comment;
pub use task::Task;

use moka::sync::Cache;

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

pub fn get_all_items<F, T>(mut get_page: F) -> Vec<T>
where
    F: FnMut(usize) -> Vec<T>,
{
    let mut ret = Vec::new();
    let mut page = 1;
    loop {
        let current_page = get_page(page);
        if current_page.is_empty() {
            break;
        }
        ret.extend(current_page);
        page += 1;
    }
    ret
}

pub struct ProjectID(pub isize);

impl ProjectID {
    pub fn parse(api: &VikunjaAPI, project: &str) -> Option<Self> {
        let project = project.trim_start_matches('#');

        if let Ok(num) = project.parse() {
            Some(Self(num))
        } else {
            Some(Self(
                api.get_all_projects()
                    .into_iter()
                    .find(|x| x.title.contains(project))?
                    .id,
            ))
        }
    }
}

pub struct VikunjaAPI {
    host: String,
    token: String,
    cache: Cache<String, String>,
}

impl VikunjaAPI {
    pub fn new(host: &str, token: &str) -> Self {
        Self {
            host: host.to_string(),
            token: token.to_string(),
            cache: Cache::new(100),
        }
    }

    fn get_request(&self, path: &str) -> String {
        self.cache.get(path).map_or_else(
            || {
                let client = reqwest::blocking::Client::new();

                let ret = client
                    .get(format!("{}/api/v1{}", self.host, path))
                    .header("Authorization", format!("Bearer {}", self.token))
                    .send()
                    .unwrap()
                    .text()
                    .unwrap();

                self.cache.insert(path.to_string(), ret.clone());
                ret
            },
            |cached| cached,
        )
    }

    fn put_request(&self, path: &str, data: &serde_json::Value) -> String {
        let client = reqwest::blocking::Client::new();

        client
            .put(format!("{}/api/v1{}", self.host, path))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&data)
            .send()
            .unwrap()
            .text()
            .unwrap()
    }

    fn post_request(&self, path: &str, data: &serde_json::Value) -> String {
        let client = reqwest::blocking::Client::new();

        client
            .post(format!("{}/api/v1{}", self.host, path))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&data)
            .send()
            .unwrap()
            .text()
            .unwrap()
    }

    fn delete_request(&self, path: &str) -> String {
        let client = reqwest::blocking::Client::new();

        client
            .delete(format!("{}/api/v1{}", self.host, path))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .unwrap()
            .text()
            .unwrap()
    }

    // projects

    pub fn get_project_name_from_id(&self, id: isize) -> String {
        let all_prj = self.get_all_projects();

        let found = all_prj.into_iter().find(|x| x.id == id).unwrap();

        found.title
    }

    pub fn get_all_projects(&self) -> Vec<Project> {
        let resp = self.get_request("/projects");
        serde_json::from_str(&resp).unwrap()
    }

    pub fn delete_project(&self, project_id: ProjectID) {
        self.delete_request(&format!("/projects/{}", project_id.0));
    }

    pub fn new_project(
        &self,
        title: &str,
        description: Option<&str>,
        color: Option<&str>,
        parent: Option<ProjectID>,
    ) -> Project {
        let data = serde_json::json!({
            "description": description,
            "hex_color": color,
            "parent_project_id": parent.map(|x| x.0),
            "title": title
        });

        let resp = self.put_request("/projects", &data);
        serde_json::from_str(&resp).unwrap()
    }

    pub fn get_project(&self, project: &ProjectID) -> Project {
        let resp = self.get_request(&format!("/projects/{}", project.0));
        serde_json::from_str(&resp).unwrap()
    }

    // labels
    pub fn get_all_labels(&self) -> Vec<Label> {
        get_all_items(|x| {
            let resp = self.get_request(&format!("/labels?page={x}"));
            if resp.trim() == "null" {
                return Vec::new();
            }
            serde_json::from_str(&resp).unwrap()
        })
    }

    pub fn new_label(&self, title: &str, description: Option<&str>, color: Option<&str>) -> Label {
        let resp = self.put_request(
            "/labels",
            &serde_json::json!({
                "title": title,
                "description": description,
                "hex_color": color
            }),
        );

        serde_json::from_str(&resp).unwrap()
    }

    pub fn remove_label(&self, title: &str) {
        let labels = self.get_all_labels();

        let label_id = labels
            .into_iter()
            .find(|x| x.title.trim() == title)
            .unwrap()
            .id;

        self.delete_request(&format!("/labels/{label_id}"));
    }

    pub fn label_task_remove(&self, label: &str, task_id: isize) {
        let labels = self.get_all_labels();

        let label_id = labels
            .into_iter()
            .find(|x| x.title.trim() == label)
            .unwrap()
            .id;

        self.delete_request(&format!("/tasks/{task_id}/labels/{label_id}"));
    }

    pub fn label_task(&self, label: &str, task_id: isize) {
        let labels = self.get_all_labels();

        let label_id = labels
            .into_iter()
            .find(|x| x.title.trim() == label)
            .unwrap()
            .id;

        self.put_request(
            &format!("/tasks/{task_id}/labels"),
            &serde_json::json!({
                "label_id": label_id
            }),
        );
    }

    // tasks
    pub fn get_task_page(&self, page: usize) -> Vec<Task> {
        let resp = self.get_request(&format!("/tasks/all?page={page}"));
        serde_json::from_str(&resp).unwrap()
    }

    pub fn get_all_tasks(&self) -> Vec<Task> {
        get_all_items(|x| self.get_task_page(x))
    }

    pub fn get_latest_tasks(&self) -> Vec<Task> {
        let resp = self.get_request("/tasks/all?per_page=60&sort_by=created&order_by=desc");
        serde_json::from_str(&resp).unwrap()
    }

    pub fn get_task(&self, id: isize) -> Task {
        let resp = self.get_request(&format!("/tasks/{id}"));
        serde_json::from_str(&resp).unwrap()
    }

    pub fn delete_task(&self, id: isize) {
        self.delete_request(&format!("/tasks/{id}"));
    }

    pub fn new_task(&self, title: &str, project: &ProjectID) -> Task {
        let id = project.0;

        let data = serde_json::json!({
            "title": title
        });

        // todo :
        // description
        // due_date
        // end_date
        // is_favorite
        // labels
        // priority

        let resp = self.put_request(&format!("/projects/{id}/tasks"), &data);
        serde_json::from_str(&resp).unwrap()
    }

    pub fn done_task(&self, task_id: isize, done: bool) -> Task {
        let resp = self.post_request(
            &format!("/tasks/{task_id}"),
            &serde_json::json!({
                "done": done,
                "done_at": if done { Some(chrono::Utc::now().to_rfc3339()) } else { None }
            }),
        );
        serde_json::from_str(&resp).unwrap()
    }

    pub fn fav_task(&self, task_id: isize, fav: bool) -> Task {
        let resp = self.post_request(
            &format!("/tasks/{task_id}"),
            &serde_json::json!({
                "is_favorite": fav
            }),
        );
        serde_json::from_str(&resp).unwrap()
    }

    pub fn login(&self, username: &str, password: &str, totp: Option<&str>) -> String {
        let resp = self.post_request(
            "/login",
            &serde_json::json!({
                "username": username,
                "password": password,
                "totp_passcode": totp
            }),
        );

        let val: serde_json::Value = serde_json::from_str(&resp).unwrap();
        val.as_object()
            .unwrap()
            .get("token")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    pub fn search_user(&self, search: &str) -> Option<Vec<User>> {
        let resp = self.get_request(&format!("/users?s={search}"));
        serde_json::from_str(&resp).ok()
    }

    pub fn assign_to_task(&self, user: &str, task_id: isize) {
        let user = self.search_user(user).unwrap();

        self.put_request(
            &format!("/tasks/{task_id}/assignees"),
            &serde_json::json!({
                "user_id": user.first().unwrap().id
            }),
        );
    }

    pub fn remove_assign_to_task(&self, user: &str, task_id: isize) {
        let user = self.search_user(user).unwrap();
        let user_id = user.first().unwrap().id;
        self.delete_request(&format!("/tasks/{task_id}/assignees/{user_id}"));
    }

    pub fn get_task_comments(&self, task_id: isize) -> Vec<Comment> {
        let resp = self.get_request(&format!("/tasks/{task_id}/comments"));
        serde_json::from_str(&resp).unwrap()
    }

    pub fn new_comment(&self, task_id: isize, comment: &str) -> Comment {
        let resp = self.put_request(
            &format!("/tasks/{task_id}/comments"),
            &serde_json::json!({
                "comment": comment
            }),
        );
        serde_json::from_str(&resp).unwrap()
    }
}
