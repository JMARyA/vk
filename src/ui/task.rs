use crate::{
    api::{Task, VikunjaAPI},
    ui::{parse_datetime, time_since},
};

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

pub fn print_current_tasks(api: &VikunjaAPI, done: bool, fav: bool) {
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

pub fn print_task_info(task_id: isize, api: &VikunjaAPI) {
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

    if let Some(labels) = task.labels {
        println!("Labels: {}", labels.first().unwrap().title);
    }

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
