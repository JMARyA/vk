mod api;
mod args;
mod config;
mod ui;

use std::path::PathBuf;

use api::{ProjectID, Relation, VikunjaAPI};
use once_cell::sync::Lazy;
use ui::{hex_to_color, print_color};

use crate::args::{LabelCmds, LoginCmd, ProjectCmds, VkCommands};

static CONFIG_PATH: Lazy<PathBuf> =
    Lazy::new(|| dirs::home_dir().unwrap().join(".config").join("vk.toml"));

async fn login_cmd(arg: &LoginCmd) {
    let host = if arg.host.starts_with("http") {
        arg.host.to_string()
    } else {
        format!("https://{}", arg.host)
    };

    let api = VikunjaAPI::new(&host, "");

    let token = api
        .login(&arg.username, &arg.password, arg.totp.clone())
        .await;
    let config = format!("host = \"{host}\"\ntoken = \"{token}\"");

    std::fs::write(CONFIG_PATH.clone(), config).unwrap();
    std::process::exit(0);
}

async fn project_commands(arg: ProjectCmds, api: &VikunjaAPI) {
    match arg.cmd {
        args::ProjectCommands::List(_) => {
            ui::project::list_projects(api).await;
        }
        args::ProjectCommands::Add(project_add_cmd) => {
            let parent = if let Some(parent) = project_add_cmd.parent {
                Some(ProjectID::parse(api, &parent).await.unwrap())
            } else {
                None
            };

            api.new_project(
                &project_add_cmd.title,
                project_add_cmd.description,
                project_add_cmd.color,
                parent,
            )
            .await
            .unwrap();
        }
        args::ProjectCommands::Remove(project_remove_cmd) => {
            api.delete_project(
                &ProjectID::parse(api, &project_remove_cmd.project)
                    .await
                    .unwrap(),
            )
            .await;
        }
    }
}

async fn label_commands(arg: LabelCmds, api: &VikunjaAPI) {
    match arg.cmd {
        args::LabelCommands::List(_) => {
            ui::print_all_labels(api).await;
        }
        args::LabelCommands::New(label_new_cmd) => {
            if let Some(color) = &label_new_cmd.color {
                if hex_to_color(&color).is_err() {
                    print_color(
                        crossterm::style::Color::Red,
                        &format!("'{color}' is no hex color"),
                    );
                    println!();
                    std::process::exit(1);
                }
            }

            api.new_label(
                &label_new_cmd.title,
                label_new_cmd.description,
                label_new_cmd.color,
            )
            .await
            .unwrap();
        }
        args::LabelCommands::Remove(label_remove_cmd) => {
            api.remove_label(&label_remove_cmd.title).await;
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

fn parse_datetime(input: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.fZ",
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%S%z",
        "%+%",
    ];

    let input = input.trim();

    for format in &formats {
        if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(input, format) {
            let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
            return Some(chrono::TimeZone::from_utc_datetime(
                &chrono::Utc,
                &naive_datetime,
            ));
        }
        if let Ok(naive_datetime) = chrono::NaiveDateTime::parse_from_str(input, format) {
            return Some(chrono::TimeZone::from_utc_datetime(
                &chrono::Utc,
                &naive_datetime,
            ));
        }
        if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(input) {
            return Some(datetime.with_timezone(&chrono::Utc));
        }
    }

    None
}

#[tokio::main]
async fn main() {
    let arg = args::get_args();

    if let Some(VkCommands::Login(arg)) = &arg.cmd {
        login_cmd(&arg).await;
    }

    let config = load_config();
    let api = VikunjaAPI::new(&config.host, &config.token);

    if let Some(subcommand) = arg.cmd {
        match subcommand {
            VkCommands::TaskInfo(task_info_cmd) => {
                if !task_info_cmd.json {
                    ui::task::print_task_info(task_info_cmd.task_id, &api).await;
                    return;
                }

                // json
                let task = api
                    .get_task(task_info_cmd.task_id)
                    .await
                    .unwrap_or_else(|_| {
                        print_color(
                            crossterm::style::Color::Red,
                            &format!("Could not get task #{}", task_info_cmd.task_id),
                        );
                        println!();
                        std::process::exit(1);
                    });

                println!("{}", serde_json::to_string(&task).unwrap());
            }
            VkCommands::TaskEdit(task_edit_cmd) => {
                let due_date = task_edit_cmd.due.map(|x| {
                    if let Some(parsed) = parse_datetime(&x) {
                        parsed.to_rfc3339()
                    } else {
                        print_color(crossterm::style::Color::Red, "Failed to parse due date");
                        println!();
                        std::process::exit(1);
                    }
                });
                api.edit_task(
                    task_edit_cmd.task_id,
                    task_edit_cmd.title,
                    task_edit_cmd.description,
                    due_date,
                    task_edit_cmd.priority.map(|x| x.parse().unwrap()),
                )
                .await
                .unwrap();
                ui::task::print_task_info(task_edit_cmd.task_id, &api).await;
            }
            VkCommands::TaskRemove(task_remove_cmd) => {
                api.delete_task(task_remove_cmd.task_id).await;
            }
            VkCommands::TaskDone(task_done_cmd) => {
                let task_id = task_done_cmd.task_id;
                let done = !task_done_cmd.undo;
                api.done_task(task_id, done).await.unwrap();
                ui::task::print_task_info(task_id, &api).await;
            }
            VkCommands::TaskNew(task_new_cmd) => {
                let title = task_new_cmd.title;
                let project = &task_new_cmd.project;
                let project = ProjectID::parse(&api, project).await.unwrap();
                let description: Option<String> = task_new_cmd.description;
                let due_date: Option<String> = task_new_cmd.due;
                let due_date = due_date.map(|x| {
                    if let Some(parsed) = parse_datetime(&x) {
                        parsed.to_rfc3339()
                    } else {
                        print_color(crossterm::style::Color::Red, "Failed to parse due date");
                        println!();
                        std::process::exit(1);
                    }
                });

                let label: Option<String> = task_new_cmd.label;
                let priority: Option<String> = task_new_cmd.priority;
                let fav = task_new_cmd.favorite;
                // todo : add args

                let task = api
                    .new_task(
                        title.as_str(),
                        &project,
                        description,
                        due_date,
                        fav,
                        label,
                        priority.map(|x| x.parse().unwrap()),
                    )
                    .await;
                if let Err(msg) = task {
                    print_color(crossterm::style::Color::Red, &msg);
                    println!();
                    std::process::exit(1);
                } else {
                    ui::task::print_task_info(task.unwrap().id.unwrap() as i32, &api).await;
                }
            }
            VkCommands::TaskAssign(task_assign_cmd) => {
                if task_assign_cmd.undo {
                    api.remove_assign_to_task(&task_assign_cmd.user, task_assign_cmd.task_id)
                        .await;
                } else if let Err(msg) = api
                    .assign_to_task(&task_assign_cmd.user, task_assign_cmd.task_id)
                    .await
                {
                    print_color(crossterm::style::Color::Red, &msg);
                    println!();
                }
            }
            VkCommands::TaskComments(task_comments_cmd) => {
                let comments = api
                    .get_task_comments(task_comments_cmd.task_id)
                    .await
                    .unwrap();

                for comment in comments {
                    ui::task::print_comment(&comment);
                }
            }
            VkCommands::TaskComment(task_comment_cmd) => {
                api.new_comment(task_comment_cmd.task_id, task_comment_cmd.comment)
                    .await
                    .unwrap();
            }
            VkCommands::TaskRelation(task_relation_cmd) => {
                let task_id = task_relation_cmd.task_id;
                let relation = &task_relation_cmd.relation;
                let sec_task_id = task_relation_cmd.second_task_id;
                let delete = task_relation_cmd.delete;

                let relation = Relation::try_parse(relation).unwrap();

                if delete {
                    api.remove_relation(task_id, &relation, sec_task_id).await;
                } else {
                    api.add_relation(task_id, &relation, sec_task_id)
                        .await
                        .unwrap();
                }

                ui::task::print_task_info(task_id, &api).await;
            }
            VkCommands::TaskLabel(task_label_cmd) => {
                if task_label_cmd.undo {
                    api.label_task_remove(&task_label_cmd.label, task_label_cmd.task_id)
                        .await;
                } else if let Err(msg) = api
                    .label_task(&task_label_cmd.label, task_label_cmd.task_id)
                    .await
                {
                    let msg = format!("API Error: {msg}");
                    print_color(crossterm::style::Color::Red, &msg);
                    println!();
                    std::process::exit(1);
                }
                ui::task::print_task_info(task_label_cmd.task_id, &api).await;
            }
            VkCommands::TaskFav(task_fav_cmd) => {
                api.fav_task(task_fav_cmd.task_id, !task_fav_cmd.undo)
                    .await
                    .unwrap();
                ui::task::print_task_info(task_fav_cmd.task_id, &api).await;
            }

            VkCommands::Stats(_) => {
                ui::stats::print_stats(&api, &config).await;
            }
            VkCommands::Login(_) => unreachable!(),
            VkCommands::ProjectCmds(project_cmds) => project_commands(project_cmds, &api).await,
            VkCommands::Labels(label_cmds) => label_commands(label_cmds, &api).await,
        }
    } else {
        ui::task::print_current_tasks(&api, arg.done, arg.favorite, arg.from, arg.label).await;
    }
}
