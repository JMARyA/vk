use crate::{
    api::{ProjectID, Task, VikunjaAPI},
    ui::{parse_datetime, print_color, time_since},
};

fn print_task_oneline(task: &Task, api: &VikunjaAPI) {
    print_color(crossterm::style::Color::Yellow, &format!("({}) ", task.id));

    if task.is_favorite {
        print_color(crossterm::style::Color::Yellow, "⭐ ");
    }

    print_color(crossterm::style::Color::Blue, &task.title);

    // todo : colors based on project colors
    print_color(
        crossterm::style::Color::DarkRed,
        &format!(" [{}]", api.get_project_name_from_id(task.project_id)),
    );

    if task.done {
        print_color(crossterm::style::Color::Green, " [✓]");
    }

    print!("\n");
}

pub fn print_current_tasks(api: &VikunjaAPI, done: bool, fav: bool, project: Option<&String>) {
    // todo : improve performance by using filters -> https://vikunja.io/docs/filters/
    let current_tasks = api.get_all_tasks();

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

        selection = selection
            .into_iter()
            .filter(|x| x.project_id == p_id.0)
            .collect();
    }

    for task in selection {
        print_task_oneline(&task, api);
    }
}

pub fn print_task_info(task_id: isize, api: &VikunjaAPI) {
    let task = api.get_task(task_id);

    if task.done {
        print_color(
            crossterm::style::Color::Green,
            &format!("{} ✓ ", time_since(parse_datetime(&task.done_at).unwrap())),
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

    println!("Created by {}", task.created_by.username);

    if let Some(due_date) = parse_datetime(&task.due_date) {
        // todo : color if overdue
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

    if let Some(labels) = task.labels {
        // todo : labels and color
        println!("Labels: {}", labels.first().unwrap().title);
    }

    println!(
        "Created: {} | Updated: {}",
        time_since(parse_datetime(&task.created).unwrap()),
        time_since(parse_datetime(&task.updated).unwrap())
    );

    if task.description != "<p></p>" && !task.description.is_empty() {
        println!("---\n{}", task.description);
    }

    //pub assignees: Option<Vec<String>>,
    //pub labels: Option<Vec<Label>>,
    // pub percent_done: f64,
}
