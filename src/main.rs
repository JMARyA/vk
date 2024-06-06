mod api;
mod args;
mod config;
mod ui;

use api::{ProjectID, VikunjaAPI};

fn main() {
    let config: config::Config =
        toml::from_str(&std::fs::read_to_string("config.toml").unwrap()).unwrap();
    let api = VikunjaAPI::new(&config.host, &config.token);
    let arg = args::get_args();

    match arg.subcommand() {
        Some(("info", task_info_arg)) => {
            let task_id: &String = task_info_arg.get_one("task_id").unwrap();
            ui::task::print_task_info(task_id.parse().unwrap(), &api);
        }
        Some(("prj", prj_arg)) => match prj_arg.subcommand() {
            Some(("ls", _)) => {
                ui::project::list_projects(&api);
            }
            Some(("add", add_prj_arg)) => {}
            _ => {
                ui::project::list_projects(&api);
            }
        },
        Some(("label", label_args)) => match label_args.subcommand() {
            Some(("ls", _)) => {
                ui::print_all_labels(&api);
            }
            Some(("new", new_label_arg)) => {}
            _ => {
                // todo : label tasks
            }
        },
        Some(("new", new_task_arg)) => {
            let title: &String = new_task_arg.get_one("title").unwrap();
            let project: &String = new_task_arg.get_one("project").unwrap();
            let project = ProjectID::parse(&api, project).unwrap();
            let task = api.new_task(title.as_str(), &project);
            ui::task::print_task_info(task.id, &api);
        }
        Some(("done", done_args)) => {
            let task_id: &String = done_args.get_one("task_id").unwrap();
            api.done_task(task_id.parse().unwrap());
        }
        _ => {
            let done = arg.get_flag("done");
            let fav = arg.get_flag("favorite");
            let project: Option<&String> = arg.get_one("from");
            let label: Option<&String> = arg.get_one("label");
            ui::task::print_current_tasks(&api, done, fav, project, label);
        }
    }
}
