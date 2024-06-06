mod api;
mod args;
mod config;
mod ui;

use api::VikunjaAPI;

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
            _ => {
                ui::project::list_projects(&api);
            }
        },
        _ => {
            let done = arg.get_flag("done");
            let fav = arg.get_flag("favorite");
            ui::task::print_current_tasks(&api, done, fav);
        }
    }
}
