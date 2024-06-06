use crate::{
    api::{Project, ProjectID, Task, VikunjaAPI},
    ui::{
        format_html_to_terminal, hex_to_color, is_in_past, parse_datetime, print_color,
        print_label, time_relative,
    },
};

// todo : move to grid view
fn print_task_oneline(task: &Task, projects: &[Project]) {
    print_color(crossterm::style::Color::Yellow, &format!("({}) ", task.id));

    if task.is_favorite {
        print_color(crossterm::style::Color::Yellow, "⭐ ");
    }

    print_color(crossterm::style::Color::Blue, &task.title);

    let project = projects.iter().find(|x| x.id == task.project_id).unwrap();
    print_color(
        hex_to_color(&project.hex_color).unwrap_or(crossterm::style::Color::Reset),
        &format!(" [{}]", project.title),
    );

    if task.done {
        print_color(crossterm::style::Color::Green, " [✓]");
    }

    if let Some(labels) = &task.labels {
        print!(" ");
        for label in labels {
            print_label(label);
            print!(" ");
        }
    }

    println!();
}

pub fn print_current_tasks(
    api: &VikunjaAPI,
    done: bool,
    fav: bool,
    project: Option<&String>,
    label: Option<&String>,
) {
    let current_tasks = if project.is_some() || label.is_some() {
        api.get_all_tasks()
    } else {
        api.get_latest_tasks()
    };

    let mut selection: Vec<_> = if done {
        current_tasks
    } else {
        current_tasks.into_iter().filter(|x| !x.done).collect()
    };

    selection = if fav {
        selection.into_iter().filter(|x| x.is_favorite).collect()
    } else {
        selection
    };

    if let Some(project) = project {
        let p_id = ProjectID::parse(api, project).unwrap();
        selection.retain(|x| x.project_id == p_id.0);
    }

    if let Some(label_match) = label {
        selection.retain(|x| {
            if let Some(labels) = &x.labels {
                for label in labels {
                    if label.title.trim() == *label_match {
                        return true;
                    }
                }
            }
            false
        });
    }

    let projects = api.get_all_projects();

    for task in selection {
        print_task_oneline(&task, &projects);
    }
}

pub fn print_task_info(task_id: isize, api: &VikunjaAPI) {
    let task = api.get_task(task_id);

    if task.done {
        print_color(
            crossterm::style::Color::Green,
            &format!(
                "{} ✓ ",
                if let Some(dt) = parse_datetime(&task.done_at) {
                    time_relative(dt)
                } else {
                    String::new()
                }
            ),
        );
    }

    if task.is_favorite {
        print!("⭐ ");
    }

    print_color(crossterm::style::Color::Blue, &task.title);
    print_color(crossterm::style::Color::Yellow, &format!(" ({})", task.id));
    print_color(
        crossterm::style::Color::DarkRed,
        &format!(" [{}]\n", api.get_project_name_from_id(task.project_id)),
    );

    if let Some(user) = task.created_by {
        println!("Created by {}", user.username);
    }

    println!(
        "Created: {} | Updated: {}",
        time_relative(parse_datetime(&task.created).unwrap()),
        time_relative(parse_datetime(&task.updated).unwrap())
    );

    if let Some(due_date) = parse_datetime(&task.due_date) {
        print_color(
            if is_in_past(due_date) {
                crossterm::style::Color::Red
            } else {
                crossterm::style::Color::Reset
            },
            &format!("Due {}", time_relative(due_date)),
        );
        println!();
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

    if let Some(labels) = task.labels {
        print!("Labels: ");
        for label in labels {
            print_label(&label);
            print!(" ");
        }
        println!();
    }

    if task.description != "<p></p>" && !task.description.is_empty() {
        println!("---\n{}", format_html_to_terminal(&task.description));
    }

    if let Some(assigned) = task.assignees {
        print!("Assigned to: ");
        for assignee in assigned {
            print!("{} ", assignee.username);
        }
        println!();
    }

    // pub percent_done: f64,
}
