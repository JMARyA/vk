mod api;
mod args;
mod config;
mod ui;

use std::path::PathBuf;

use api::{ProjectID, Relation, VikunjaAPI};
use clap::ArgMatches;
use once_cell::sync::Lazy;
use ui::{hex_to_color, print_color};

static CONFIG_PATH: Lazy<PathBuf> =
    Lazy::new(|| dirs::home_dir().unwrap().join(".config").join("vk.toml"));

fn login_cmd(arg: &ArgMatches) {
    if let Some(("login", login_arg)) = arg.subcommand() {
        let username: &String = login_arg.get_one("username").unwrap();
        let password: &String = login_arg.get_one("password").unwrap();
        let totp: Option<&String> = login_arg.get_one("totp");
        let host: &String = login_arg.get_one("host").unwrap();

        let host = if host.starts_with("http") {
            host.to_string()
        } else {
            format!("https://{host}")
        };

        let api = VikunjaAPI::new(&host, "");

        let token = api.login(username, password, totp.map(std::string::String::as_str));
        let config = format!("host = \"{host}\"\ntoken = \"{token}\"");

        std::fs::write(CONFIG_PATH.clone(), config).unwrap();
        std::process::exit(0);
    }
}

fn project_commands(arg: &ArgMatches, api: &VikunjaAPI) {
    match arg.subcommand() {
        Some(("add", add_prj_arg)) => {
            let title: &String = add_prj_arg.get_one("title").unwrap();
            let description: Option<&String> = add_prj_arg.get_one("description");
            let color: Option<&String> = add_prj_arg.get_one("color");
            let parent: Option<&String> = add_prj_arg.get_one("parent");
            api.new_project(
                title,
                description.map(std::string::String::as_str),
                color.map(std::string::String::as_str),
                parent.map(|x| ProjectID::parse(api, x).unwrap()),
            );
        }
        Some(("rm", rm_prj_arg)) => {
            let prj: &String = rm_prj_arg.get_one("project").unwrap();
            api.delete_project(&ProjectID::parse(api, prj).unwrap());
        }
        _ => {
            ui::project::list_projects(api);
        }
    }
}

fn label_commands(arg: &ArgMatches, api: &VikunjaAPI) {
    match arg.subcommand() {
        Some(("rm", rm_label_arg)) => {
            let title: &String = rm_label_arg.get_one("title").unwrap();

            api.remove_label(title);
        }
        Some(("new", new_label_arg)) => {
            let description: Option<&String> = new_label_arg.get_one("description");
            let color: Option<&String> = new_label_arg.get_one("color");
            let title: &String = new_label_arg.get_one("title").unwrap();

            if let Some(color) = color {
                if hex_to_color(color).is_err() {
                    print_color(
                        crossterm::style::Color::Red,
                        &format!("'{color}' is no hex color"),
                    );
                    println!();
                    std::process::exit(1);
                }
            }

            api.new_label(
                title.as_str(),
                description.map(std::string::String::as_str),
                color.map(std::string::String::as_str),
            );
        }
        _ => {
            ui::print_all_labels(api);
        }
    }
}

fn load_config() -> config::Config {
    let content = &std::fs::read_to_string(CONFIG_PATH.clone()).unwrap_or_else(|e| {
        ui::print_color(
            crossterm::style::Color::Red,
            &format!("Could not read config file: {e}"),
        );
        println!("\nTo setup vk run `vk login --help`");
        std::process::exit(1);
    });

    toml::from_str(content).unwrap()
}

fn main() {
    let arg = args::get_args();

    login_cmd(&arg);

    let config = load_config();
    let api = VikunjaAPI::new(&config.host, &config.token);

    match arg.subcommand() {
        Some(("info", task_info_arg)) => {
            let task_id: &String = task_info_arg.get_one("task_id").unwrap();
            ui::task::print_task_info(task_id.parse().unwrap(), &api);
        }
        Some(("prj", prj_arg)) => project_commands(prj_arg, &api),
        Some(("rm", rm_args)) => {
            let task_id: &String = rm_args.get_one("task_id").unwrap();
            api.delete_task(task_id.parse().unwrap());
        }
        Some(("assign", assign_arg)) => {
            let user: &String = assign_arg.get_one("user").unwrap();
            let task_id: &String = assign_arg.get_one("task_id").unwrap();
            let undo = assign_arg.get_flag("undo");

            if undo {
                api.remove_assign_to_task(user, task_id.parse().unwrap());
            } else if let Err(msg) = api.assign_to_task(user, task_id.parse().unwrap()) {
                print_color(crossterm::style::Color::Red, &msg);
                println!();
            }
        }
        Some(("comments", c_arg)) => {
            let task_id: &String = c_arg.get_one("task_id").unwrap();
            let comments = api.get_task_comments(task_id.parse().unwrap());

            for comment in comments {
                ui::task::print_comment(&comment);
            }
        }
        Some(("comment", comment_arg)) => {
            let task_id: &String = comment_arg.get_one("task_id").unwrap();
            let comment: &String = comment_arg.get_one("comment").unwrap();

            api.new_comment(task_id.parse().unwrap(), comment);
        }
        Some(("labels", label_args)) => label_commands(label_args, &api),
        Some(("label", label_args)) => {
            let label: &String = label_args.get_one("label").unwrap();
            let task_id: &String = label_args.get_one("task_id").unwrap();
            let undo = label_args.get_flag("undo");

            if undo {
                api.label_task_remove(label, task_id.parse().unwrap());
            } else if let Err(msg) = api.label_task(label, task_id.parse().unwrap()) {
                print_color(crossterm::style::Color::Red, &msg);
                println!();
                std::process::exit(1);
            }
            ui::task::print_task_info(task_id.parse().unwrap(), &api);
        }
        Some(("new", new_task_arg)) => {
            let title: &String = new_task_arg.get_one("title").unwrap();
            let project: &String = new_task_arg.get_one("project").unwrap();
            let project = ProjectID::parse(&api, project).unwrap();
            let task = api.new_task(title.as_str(), &project);
            ui::task::print_task_info(task.id, &api);
        }
        Some(("done", done_args)) => {
            let task_id: &String = done_args.get_one("task_id").unwrap();
            let done = !done_args.get_flag("undo");
            api.done_task(task_id.parse().unwrap(), done);
            ui::task::print_task_info(task_id.parse().unwrap(), &api);
        }
        Some(("fav", fav_args)) => {
            let task_id: &String = fav_args.get_one("task_id").unwrap();
            let undo = fav_args.get_flag("undo");

            api.fav_task(task_id.parse().unwrap(), !undo);
            ui::task::print_task_info(task_id.parse().unwrap(), &api);
        }
        Some(("relation", rel_args)) => {
            let task_id: &String = rel_args.get_one("task_id").unwrap();
            let relation: &String = rel_args.get_one("relation").unwrap();
            let sec_task_id: &String = rel_args.get_one("second_task_id").unwrap();
            let delete = rel_args.get_flag("delete");

            let relation = Relation::try_parse(relation).unwrap();

            if delete {
                api.remove_relation(
                    task_id.parse().unwrap(),
                    &relation,
                    sec_task_id.parse().unwrap(),
                );
            } else {
                api.add_relation(
                    task_id.parse().unwrap(),
                    &relation,
                    sec_task_id.parse().unwrap(),
                );
            }

            ui::task::print_task_info(task_id.parse().unwrap(), &api);
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
