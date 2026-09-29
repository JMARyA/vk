use std::collections::HashMap;

use crossterm::style::Color;
use vikunjars::models::ModelsProject;

use crate::{
    api::VikunjaAPI,
    ui::{hex_to_color, print_color},
};

/// Colour of a project, falling back to the terminal default.
///
/// Since Vikunja 2.4.0 zero-valued fields are omitted from the JSON entirely,
/// so an uncoloured project has no `hex_color` at all rather than an empty one.
fn project_color(prj: &ModelsProject) -> Color {
    match prj.hex_color.as_deref() {
        None | Some("") => Color::Reset,
        Some(hex) => hex_to_color(hex).unwrap_or(Color::Reset),
    }
}

pub async fn list_projects(api: &VikunjaAPI) {
    let projects = api.get_all_projects().await.unwrap();

    let mut project_map: HashMap<isize, Vec<ModelsProject>> = HashMap::new();

    for prj in projects {
        // Top-level projects have no parent, which Vikunja omits rather than
        // sending as 0 — both spellings belong under the 0 bucket.
        project_map
            .entry(prj.parent_project_id.unwrap_or(0) as isize)
            .or_default()
            .push(prj);
    }

    let Some(top_level) = project_map.get(&0) else {
        return;
    };

    for prj in top_level {
        print_color(project_color(prj), prj.title.as_deref().unwrap_or(""));
        print_color(Color::DarkGrey, &format!("  #{}\n", prj.id.unwrap_or(0)));

        if let Some(sub_projects) = project_map.get(&(prj.id.unwrap_or(0) as isize)) {
            let last = sub_projects.len().saturating_sub(1);
            for (i, sub_prj) in sub_projects.iter().enumerate() {
                let connector = if i == last { "  └ " } else { "  ├ " };
                print_color(Color::DarkGrey, connector);
                print_color(
                    project_color(sub_prj),
                    sub_prj.title.as_deref().unwrap_or(""),
                );
                print_color(
                    Color::DarkGrey,
                    &format!("  #{}\n", sub_prj.id.unwrap_or(0)),
                );
            }
        }
    }
}
