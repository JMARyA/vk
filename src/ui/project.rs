use std::collections::HashMap;

use crossterm::style::Color;
use vikunjars::models::ModelsProject;

use crate::{
    api::VikunjaAPI,
    ui::{hex_to_color, print_color},
};

pub async fn list_projects(api: &VikunjaAPI) {
    let projects = api.get_all_projects().await.unwrap();

    let mut project_map: HashMap<isize, Vec<ModelsProject>> = HashMap::new();

    for prj in projects {
        project_map
            .entry(prj.parent_project_id.unwrap() as isize)
            .or_default()
            .push(prj);
    }

    for prj in project_map.get(&0).unwrap() {
        let color = if prj.hex_color.as_ref().unwrap().is_empty() {
            Color::Reset
        } else {
            hex_to_color(&prj.hex_color.as_ref().unwrap()).unwrap()
        };
        print_color(color, &prj.title.as_ref().unwrap());
        println!(" [{}]", prj.id.unwrap());

        if let Some(sub_projects) = project_map.get(&(prj.id.unwrap() as isize)) {
            for sub_prj in sub_projects {
                let color = if sub_prj.hex_color.as_ref().unwrap().is_empty() {
                    Color::Reset
                } else {
                    hex_to_color(&sub_prj.hex_color.as_ref().unwrap()).unwrap()
                };
                print_color(color, &format!("  - {}", sub_prj.title.as_ref().unwrap()));
                println!(" [{}]", sub_prj.id.unwrap());
            }
        }
    }
}
