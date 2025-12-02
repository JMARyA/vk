use vikunjars::models::{ModelsProject, ModelsTask, ModelsTaskComment};

use crate::{
    api::{ProjectID, Relation, VikunjaAPI},
    ui::{
        format_html_to_terminal, hex_to_color, is_in_past, parse_datetime, print_color,
        print_label, time_relative,
    },
};

// todo : move to grid view
fn print_task_oneline(task: &ModelsTask, projects: &[ModelsProject]) {
    print_color(
        crossterm::style::Color::Yellow,
        &format!("({}) ", task.id.unwrap()),
    );

    if task.is_favorite.unwrap_or_default() {
        print_color(crossterm::style::Color::Yellow, "⭐ ");
    }

    print_color(crossterm::style::Color::Blue, &task.title.as_ref().unwrap());

    let project = projects
        .iter()
        .find(|x| x.id.unwrap() == task.project_id.unwrap())
        .unwrap();
    print_color(
        hex_to_color(&project.hex_color.as_ref().unwrap())
            .unwrap_or(crossterm::style::Color::Reset),
        &format!(" [{}]", project.title.as_ref().unwrap()),
    );

    if task.done.unwrap_or_default() {
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

pub async fn print_current_tasks(
    api: &VikunjaAPI,
    done: bool,
    fav: bool,
    project: Option<&String>,
    label: Option<&String>,
) {
    let current_tasks = if project.is_some() || label.is_some() {
        api.get_all_tasks().await
    } else {
        api.get_latest_tasks().await.unwrap()
    };

    let mut selection: Vec<_> = if done {
        current_tasks
    } else {
        current_tasks
            .into_iter()
            .filter(|x| !x.done.unwrap_or_default())
            .collect()
    };

    selection = if fav {
        selection
            .into_iter()
            .filter(|x| x.is_favorite.unwrap_or_default())
            .collect()
    } else {
        selection
    };

    if let Some(project) = project {
        let p_id = ProjectID::parse(api, project).await.unwrap();
        selection.retain(|x| x.project_id.unwrap_or_default() == p_id.0 as i32);
    }

    if let Some(label_match) = label {
        selection.retain(|x| {
            if let Some(labels) = &x.labels {
                for label in labels {
                    if label.title.as_ref().unwrap().trim() == *label_match {
                        return true;
                    }
                }
            }
            false
        });
    }

    let projects = api.get_all_projects().await.unwrap();

    for task in selection {
        print_task_oneline(&task, &projects);
    }
}

pub async fn print_task_info(task_id: i32, api: &VikunjaAPI) {
    let task = api.get_task(task_id).await.unwrap_or_else(|_| {
        print_color(
            crossterm::style::Color::Red,
            &format!("Could not get task #{task_id}"),
        );
        println!();
        std::process::exit(1);
    });

    if task.done.unwrap_or_default() {
        print_color(
            crossterm::style::Color::Green,
            &format!(
                "{} ✓ ",
                parse_datetime(&task.done_at.unwrap()).map_or_else(String::new, time_relative)
            ),
        );
    }

    if task.is_favorite.unwrap_or_default() {
        print!("⭐ ");
    }

    print_color(crossterm::style::Color::Blue, &task.title.unwrap());
    print_color(
        crossterm::style::Color::Yellow,
        &format!(" ({})", task.id.unwrap()),
    );
    print_color(
        crossterm::style::Color::DarkRed,
        &format!(
            " [{}]\n",
            api.get_project_name_from_id(task.project_id.unwrap() as isize)
                .await
        ),
    );

    if let Some(user) = task.created_by {
        println!("Created by {}", user.username.unwrap());
    }

    println!(
        "Created: {} | Updated: {}",
        time_relative(parse_datetime(&task.created.unwrap()).unwrap()),
        time_relative(parse_datetime(&task.updated.unwrap()).unwrap())
    );

    if let Some(due_date) = parse_datetime(&task.due_date.unwrap()) {
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

    if task.priority.unwrap_or_default() != 0 {
        println!("Priority: {}", task.priority.unwrap_or_default());
    }

    if let (Some(start_date), Some(end_date)) = (
        parse_datetime(&task.start_date.unwrap()),
        parse_datetime(&task.end_date.unwrap()),
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

    if let Some(assigned) = task.assignees {
        print!("Assigned to: ");
        for assignee in assigned {
            print!("{} ", assignee.username.unwrap());
        }
        println!();
    }

    if let Some(related) = task.related_tasks {
        for relation in related {
            print_color(
                crossterm::style::Color::Magenta,
                &format!("{}: ", Relation::try_parse(&relation.0).unwrap().repr()),
            );
            for t in relation.1 {
                // todo : add done indication
                print_color(crossterm::style::Color::Blue, &t.title.unwrap());
                print_color(
                    crossterm::style::Color::Yellow,
                    &format!(" ({})", t.id.unwrap()),
                );
                print!(" ");
            }
            println!();
        }
    }

    if task.description.as_ref().unwrap() != "<p></p>"
        && !task.description.as_ref().unwrap().is_empty()
    {
        println!(
            "---\n{}",
            format_html_to_terminal(&task.description.unwrap())
        );
    }

    // pub percent_done: f64,
}

pub fn print_comment(comment: &ModelsTaskComment) {
    print_color(
        crossterm::style::Color::Blue,
        &comment.author.as_ref().unwrap().username.as_ref().unwrap(),
    );
    print!(
        " ({}): ",
        time_relative(parse_datetime(&comment.created.as_ref().unwrap()).unwrap())
    );
    println!();
    print!(
        "{}",
        format_html_to_terminal(&comment.comment.as_ref().unwrap())
    );
    println!();
}
