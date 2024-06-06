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
        if let Some(cached) = self.cache.get(path) {
            cached
        } else {
            let client = reqwest::blocking::Client::new();

            let ret = client
                .get(&format!("{}/api/v1{}", self.host, path))
                .header("Authorization", format!("Bearer {}", self.token))
                .send()
                .unwrap()
                .text()
                .unwrap();

            self.cache.insert(path.to_string(), ret.clone());
            ret
        }
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

    pub fn get_task_page(&self, page: usize) -> Vec<Task> {
        let resp = self.get_request(&format!("/tasks/all?page={page}"));
        serde_json::from_str(&resp).unwrap()
    }

    // tasks
    pub fn get_all_tasks(&self) -> Vec<Task> {
        let mut ret = Vec::new();
        let mut page = 0;
        loop {
            let current_page = self.get_task_page(page);
            if current_page.is_empty() {
                break;
            }
            ret.extend(current_page);
            page += 1;
        }
        ret
    }

    pub fn get_task(&self, id: isize) -> Task {
        let resp = self.get_request(&format!("/tasks/{id}"));
        serde_json::from_str(&resp).unwrap()
    }
}
