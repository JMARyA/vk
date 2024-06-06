mod api;
mod args;
mod config;

use std::{collections::HashMap, io::stdout};

use api::{Project, Task, VikunjaAPI};
use chrono::{DateTime, Utc};
use crossterm::{
    style::{Color, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};

pub fn print_color(color: Color, txt: &str) {
    stdout().execute(SetForegroundColor(color)).unwrap();
    print!("{txt}");
    stdout().execute(SetForegroundColor(Color::Reset)).unwrap();
}

fn print_task_oneline(task: &Task, api: &VikunjaAPI) {
    let done_indicator = if task.done { "✓" } else { " " };

    println!(
        "[{}] ({}) '{}' [{}]",
        done_indicator,
        task.id,
        task.title,
        api.get_project_name_from_id(task.project_id),
    );
}

fn print_current_tasks(api: &VikunjaAPI, done: bool, fav: bool) {
    let current_tasks = api.get_all_tasks();

    let selection: Vec<_> = if done {
        current_tasks
    } else {
        current_tasks.into_iter().filter(|x| !x.done).collect()
    };

    let selection = if fav {
        selection.into_iter().filter(|x| x.is_favorite).collect()
    } else {
        selection
    };

    for task in selection {
        print_task_oneline(&task, api);
    }
}

fn parse_datetime(datetime_str: &str) -> Option<DateTime<Utc>> {
    if datetime_str == "0001-01-01T00:00:00Z" {
        return None;
    }

    match DateTime::parse_from_rfc3339(datetime_str) {
        Ok(dt) => Some(dt.with_timezone(&Utc)),
        Err(_) => None, // Return None if parsing fails
    }
}

pub fn time_since(event: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(event);

    if duration.num_days() > 0 {
        return format!("{}d ago", duration.num_days());
    } else if duration.num_hours() > 0 {
        return format!("{}h ago", duration.num_hours());
    } else if duration.num_minutes() > 0 {
        return format!("{}m ago", duration.num_minutes());
    } else {
        return "Just now".to_string();
    }
}

fn print_task_info(task_id: isize, api: &VikunjaAPI) {
    let task = api.get_task(task_id);
    let done_indicator = if task.done {
        format!("{} ✓ ", parse_datetime(&task.done_at).unwrap())
    } else {
        String::new()
    };
    let fav_indicator = if task.is_favorite { " ★ " } else { "" };

    println!(
        "{}{}'{}' [{}]  [{}]",
        done_indicator,
        fav_indicator,
        task.title,
        task.id,
        api.get_project_name_from_id(task.project_id)
    );
    println!("Created by {}", task.created_by.username);

    if let Some(due_date) = parse_datetime(&task.due_date) {
        println!("Due at {due_date}");
    }

    if task.priority != 0 {
        println!("Priority: {}", task.priority);
    }

    if let (Some(start_date), Some(end_date)) = (
        parse_datetime(&task.start_date),
        parse_datetime(&task.end_date),
    ) {
        println!("{start_date} -> {end_date}");
    }

    println!("Labels: {}", task.labels.unwrap().first().unwrap().title);

    println!(
        "Created: {} | Updated: {}",
        time_since(parse_datetime(&task.created).unwrap()),
        time_since(parse_datetime(&task.updated).unwrap())
    );

    if task.description != "<p></p>" {
        println!("---\n{}", task.description);
    }

    //pub assignees: Option<Vec<String>>,
    //pub labels: Option<Vec<Label>>,
    // pub percent_done: f64,
}

fn hex_to_color(hex: &str) -> Result<Color, String> {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 6 {
        return Err("Invalid hex color length".to_string());
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid blue component")?;

    Ok(Color::Rgb { r, g, b })
}

fn list_projects(api: &VikunjaAPI) {
    let projects = api.get_all_projects();

    let mut project_map: HashMap<usize, Vec<Project>> = HashMap::new();

    for prj in projects {
        project_map
            .entry(prj.parent_project_id)
            .or_insert_with(Vec::new)
            .push(prj);
    }

    for prj in project_map.get(&0).unwrap() {
        let color = if prj.hex_color.is_empty() {
            Color::Reset
        } else {
            hex_to_color(&prj.hex_color).unwrap()
        };
        print_color(color, &prj.title);
        print!(" [{}]\n", prj.id);

        if let Some(sub_projects) = project_map.get(&(prj.id as usize)) {
            for sub_prj in sub_projects {
                let color = if sub_prj.hex_color.is_empty() {
                    Color::Reset
                } else {
                    hex_to_color(&sub_prj.hex_color).unwrap()
                };
                print_color(color, &format!("  - {}", sub_prj.title));
                print!(" [{}]\n", sub_prj.id);
            }
        }
    }
}

fn main() {
    let config: config::Config =
        toml::from_str(&std::fs::read_to_string("config.toml").unwrap()).unwrap();
    let api = VikunjaAPI::new(&config.host, &config.token);
    let arg = args::get_args();

    match arg.subcommand() {
        Some(("info", task_info_arg)) => {
            let task_id: &String = task_info_arg.get_one("task_id").unwrap();
            print_task_info(task_id.parse().unwrap(), &api);
        }
        Some(("prj", prj_arg)) => match prj_arg.subcommand() {
            Some(("ls", _)) => {
                list_projects(&api);
            }
            _ => {}
        },
        _ => {
            let done = arg.get_flag("done");
            let fav = arg.get_flag("favorite");
            print_current_tasks(&api, done, fav);
        }
    }
}
