use std::path::PathBuf;

use vikunjars::models::{ModelsProject, ModelsTask};

use crate::api::VikunjaAPI;

pub struct TaskNote {
    pub id: i32,
    pub title: String,
    pub description: String,
    pub cleaned_meta: serde_json::Value,
}

impl TaskNote {
    pub fn write_into(&self, dir: &PathBuf) {
        let file_name = dir.join(self.filename());
        println!("Writing task to {}", file_name.display());
        std::fs::write(file_name, self.file_content()).unwrap();
    }

    pub fn filename(&self) -> String {
        let sanitized: String = self
            .title
            .chars()
            .map(|c| {
                // Basic cross-platform invalid characters
                match c {
                    '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
                    _ => c,
                }
            })
            .collect();

        // If sanitizing produced something useless (all underscores or empty),
        // fall back to just the id.
        if sanitized.trim_matches('_').is_empty() {
            format!("{}.md", self.id)
        } else {
            format!("{} {}.md", self.id, sanitized)
        }
    }

    pub fn file_content(&self) -> String {
        format!(
            "---\n{}---\n# {} {}\n\n{}\n",
            serde_yml::to_string(&self.cleaned_meta).unwrap(),
            self.id,
            self.title,
            htmd::convert(&self.description).unwrap()
        )
    }
}

impl From<&ModelsTask> for TaskNote {
    fn from(value: &ModelsTask) -> Self {
        let mut cleaned_meta = serde_json::to_value(value).unwrap();

        let id = cleaned_meta
            .as_object_mut()
            .unwrap()
            .remove("id")
            .unwrap()
            .as_i64()
            .unwrap() as i32;
        let title = cleaned_meta
            .as_object_mut()
            .unwrap()
            .remove("title")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        let description = cleaned_meta
            .as_object_mut()
            .unwrap()
            .remove("description")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        Self {
            id,
            title,
            description,
            cleaned_meta,
        }
    }
}

pub fn write_prj_note(prj: &ModelsProject, file: PathBuf) {
    let info = serde_yml::to_string(&prj).unwrap();
    let name = prj.title.clone().unwrap();
    let task_view = format!(
        "```dataview\nTABLE FROM \"./{}\" WHERE project_id = {}\n```\n",
        prj.title.as_ref().unwrap(),
        prj.id.unwrap()
    );
    let contents = format!("---\n{info}---\n# {name}\n\n{task_view}\n");
    std::fs::write(file, contents).unwrap();
}

pub async fn fetch_local(dir: PathBuf, api: &VikunjaAPI) {
    let projects = api.get_all_projects().await.unwrap();

    for project in projects {
        let prj_title = project.title.as_ref().unwrap();

        if prj_title.as_str() == "Favorites" {
            continue;
        }

        println!("Syncing project '{prj_title}'");

        let prj_dir = dir.join(&project.title.as_ref().unwrap());
        std::fs::create_dir_all(&prj_dir).unwrap();

        let tasks = api
            .get_all_tasks_from_project(project.id.unwrap())
            .await
            .unwrap();

        for task in &tasks {
            let task: TaskNote = task.into();
            task.write_into(&prj_dir);
        }

        write_prj_note(
            &project,
            dir.join(format!("{}.md", project.title.as_ref().unwrap())),
        );
    }
}
