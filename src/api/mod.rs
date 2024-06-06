use serde::{Deserialize, Serialize};

mod project;
mod task;

pub use project::Project;
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

    // tasks
    pub fn get_task_page(&self, page: usize) -> Vec<Task> {
        let resp = self.get_request(&format!("/tasks/all?page={page}"));
        serde_json::from_str(&resp).unwrap()
    }

    pub fn get_all_tasks(&self) -> Vec<Task> {
        get_all_items(|x| self.get_task_page(x))
    }

    pub fn get_task(&self, id: isize) -> Task {
        let resp = self.get_request(&format!("/tasks/{id}"));
        serde_json::from_str(&resp).unwrap()
    }

    pub fn new_task(&self, title: &str, project: &ProjectID) -> Task {
        let id = project.0;

        let data = serde_json::json!({
            "title": title
        });

        // description
        // due_date
        // end_date
        // is_favorite
        // labels
        // priority

        let resp = self.put_request(&format!("/projects/{id}/tasks"), &data);
        serde_json::from_str(&resp).unwrap()
    }

    pub fn done_task(&self, task_id: isize) -> Task {
        let resp = self.post_request(
            &format!("/tasks/{task_id}"),
            &serde_json::json!({
                "done": true
            }),
        );
        serde_json::from_str(&resp).unwrap()
    }
}
