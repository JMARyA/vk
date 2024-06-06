use std::collections::HashMap;

use crossterm::style::Color;

use crate::{
    api::{Project, VikunjaAPI},
    ui::{hex_to_color, print_color},
};

pub fn list_projects(api: &VikunjaAPI) {
    let projects = api.get_all_projects();

    let mut project_map: HashMap<usize, Vec<Project>> = HashMap::new();

    for prj in projects {
        project_map
            .entry(prj.parent_project_id)
            .or_default()
            .push(prj);
    }

    for prj in project_map.get(&0).unwrap() {
        let color = if prj.hex_color.is_empty() {
            Color::Reset
        } else {
            hex_to_color(&prj.hex_color).unwrap()
        };
        print_color(color, &prj.title);
        println!(" [{}]", prj.id);

        if let Some(sub_projects) = project_map.get(&(prj.id as usize)) {
            for sub_prj in sub_projects {
                let color = if sub_prj.hex_color.is_empty() {
                    Color::Reset
                } else {
                    hex_to_color(&sub_prj.hex_color).unwrap()
                };
                print_color(color, &format!("  - {}", sub_prj.title));
                println!(" [{}]", sub_prj.id);
            }
        }
    }
}
