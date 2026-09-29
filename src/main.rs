mod api;
mod args;
mod config;
mod description;
mod error;
mod sync;
mod ui;

use std::io::IsTerminal;
use std::path::PathBuf;

use api::{ProjectID, Relation, VikunjaAPI};
use error::{Result, VkError};
use once_cell::sync::Lazy;
use serde::Serialize;
use ui::hex_to_color;

use crate::args::{LabelCmds, LoginCmd, ProjectCmds, VkCLI, VkCommands};

static CONFIG_PATH: Lazy<PathBuf> =
    Lazy::new(|| dirs::home_dir().unwrap().join(".config").join("vk.toml"));

/// Print a value as one line of JSON on stdout.
fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    let line = serde_json::to_string(value)
        .map_err(|e| VkError::other(format!("Could not encode output: {e}")))?;
    println!("{line}");
    Ok(())
}

/// Output for a command that removed something.
#[derive(Serialize)]
struct Deleted<T: Serialize> {
    deleted: T,
}

/// Refuse to start a full-screen or editor flow when there is no one at the
/// terminal, or when the caller asked for JSON.
fn require_interactive(what: &str, json: bool) -> Result<()> {
    if json {
        return Err(VkError::usage(format!(
            "{what} is interactive and has no JSON output"
        )));
    }
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return Err(VkError::usage(format!(
            "{what} needs an interactive terminal"
        )));
    }
    Ok(())
}

fn parse_due(input: &str) -> Result<String> {
    parse_datetime(input)
        .map(|d| d.to_rfc3339())
        .ok_or_else(|| VkError::usage(format!("Could not parse date '{input}'")))
}

fn parse_priority(input: &str) -> Result<i32> {
    input
        .trim()
        .parse()
        .map_err(|_| VkError::usage(format!("Priority must be a number, got '{input}'")))
}

/// Open `$EDITOR` on `initial` and return what was saved.
fn edit_in_editor(name: &str, initial: &str) -> Result<String> {
    let tmp_path = std::env::temp_dir().join(name);
    std::fs::write(&tmp_path, initial)?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor).arg(&tmp_path).status();
    let text = std::fs::read_to_string(&tmp_path);
    std::fs::remove_file(&tmp_path).ok();

    match status {
        Ok(s) if s.success() => Ok(text?),
        Ok(s) => Err(VkError::other(format!("{editor} exited with {s}"))),
        Err(e) => Err(VkError::other(format!("Could not start {editor}: {e}"))),
    }
}

async fn login_cmd(arg: &LoginCmd, json: bool) -> Result<()> {
    let host = if arg.host.starts_with("http") {
        arg.host.to_string()
    } else {
        format!("https://{}", arg.host)
    };

    let api = VikunjaAPI::new(&host, "");

    let token = api
        .login(&arg.username, &arg.password, arg.totp.clone())
        .await?;
    let config = format!("host = \"{host}\"\ntoken = \"{token}\"");

    std::fs::write(CONFIG_PATH.clone(), config)?;
    if json {
        print_json(&serde_json::json!({ "host": host }))?;
    }
    Ok(())
}

async fn project_commands(arg: ProjectCmds, api: &VikunjaAPI, json: bool) -> Result<()> {
    match arg.cmd {
        args::ProjectCommands::List(_) => {
            if json {
                print_json(&api.get_all_projects().await?)
            } else {
                ui::project::list_projects(api).await
            }
        }
        args::ProjectCommands::Add(project_add_cmd) => {
            let parent = match project_add_cmd.parent {
                Some(parent) => Some(ProjectID::parse(api, &parent).await?),
                None => None,
            };

            let project = api
                .new_project(
                    &project_add_cmd.title,
                    project_add_cmd.description,
                    project_add_cmd.color,
                    parent,
                )
                .await?;
            if json {
                print_json(&project)?;
            }
            Ok(())
        }
        args::ProjectCommands::Remove(project_remove_cmd) => {
            let project = ProjectID::parse(api, &project_remove_cmd.project).await?;
            api.delete_project(&project).await?;
            if json {
                print_json(&Deleted {
                    deleted: serde_json::json!({ "project": project.0 }),
                })?;
            }
            Ok(())
        }
    }
}

async fn label_commands(arg: LabelCmds, api: &VikunjaAPI, json: bool) -> Result<()> {
    match arg.cmd {
        args::LabelCommands::List(_) => {
            if json {
                print_json(&api.get_all_labels().await?)
            } else {
                ui::print_all_labels(api).await
            }
        }
        args::LabelCommands::New(label_new_cmd) => {
            if let Some(color) = &label_new_cmd.color {
                if hex_to_color(color).is_err() {
                    return Err(VkError::usage(format!("'{color}' is no hex color")));
                }
            }

            let label = api
                .new_label(
                    &label_new_cmd.title,
                    label_new_cmd.description,
                    label_new_cmd.color,
                )
                .await?;
            if json {
                print_json(&label)?;
            }
            Ok(())
        }
        args::LabelCommands::Remove(label_remove_cmd) => {
            let label = api.remove_label(&label_remove_cmd.title).await?;
            if json {
                print_json(&Deleted {
                    deleted: serde_json::json!({ "label": label.id }),
                })?;
            }
            Ok(())
        }
    }
}

/// Keep tasks carrying a label with exactly this (trimmed) title.
fn retain_label(tasks: &mut Vec<vikunjars::models::ModelsTask>, label: &str) {
    tasks.retain(|x| {
        x.labels.as_ref().is_some_and(|labels| {
            labels
                .iter()
                .any(|l| l.title.as_deref().unwrap_or("").trim() == label)
        })
    });
}

async fn timeline(cmd: args::TimelineCmd, api: &VikunjaAPI, json: bool) -> Result<()> {
    require_interactive("vk timeline", json)?;

    let mut tasks = api.get_all_tasks().await?;
    let projects = api.get_all_projects().await?;

    if !cmd.done {
        tasks.retain(|x| !x.done.unwrap_or_default());
    }
    if let Some(project) = &cmd.project {
        let p_id = ProjectID::parse(api, project).await?;
        tasks.retain(|x| x.project_id.unwrap_or_default() == p_id.0 as i32);
    }
    if let Some(label) = &cmd.label {
        retain_label(&mut tasks, label);
    }

    let entries: Vec<ui::timeline::Entry> = tasks
        .iter()
        .filter_map(|t| ui::timeline::Entry::from_task(t, &projects))
        .filter(|e| !cmd.scheduled || e.is_scheduled())
        .collect();
    let hidden = tasks.len() - entries.len();

    if entries.is_empty() {
        println!("No tasks to show.");
        return Ok(());
    }

    match ui::timeline::run_timeline(entries, cmd.sort, hidden) {
        Ok(Some(id)) => ui::task::print_task_info(id, api).await,
        Ok(None) => Ok(()),
        Err(e) => Err(VkError::other(format!("Terminal error: {e}"))),
    }
}

fn load_config() -> Result<config::Config> {
    let content = std::fs::read_to_string(CONFIG_PATH.clone()).map_err(|e| {
        VkError::new(
            error::Kind::Auth,
            format!("Could not read config file: {e}\nTo setup vk run `vk login --help`"),
        )
    })?;

    toml::from_str(&content).map_err(|e| {
        VkError::usage(format!(
            "Invalid config file {}: {e}",
            CONFIG_PATH.display()
        ))
    })
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
    let (arg, json) = args::get_args();

    if let Err(e) = run(arg, json).await {
        e.report(json);
        std::process::exit(e.kind.exit_code());
    }
}

/// Print a task: as JSON, or re-fetched and shown in full.
async fn show_task(
    task: &vikunjars::models::ModelsTask,
    api: &VikunjaAPI,
    json: bool,
) -> Result<()> {
    if json {
        print_json(task)
    } else {
        ui::task::print_task_info(task.id.unwrap_or_default(), api).await
    }
}

/// Like [`show_task`] for commands whose API call does not return the task.
async fn show_task_id(task_id: i32, api: &VikunjaAPI, json: bool) -> Result<()> {
    if json {
        print_json(&api.get_task(task_id).await?)
    } else {
        ui::task::print_task_info(task_id, api).await
    }
}

async fn run(arg: VkCLI, json: bool) -> Result<()> {
    if let Some(VkCommands::Login(login)) = &arg.cmd {
        return login_cmd(login, json).await;
    }

    let config = load_config()?;
    let api = VikunjaAPI::new(&config.host, &config.token);

    let Some(subcommand) = arg.cmd else {
        return list_tasks(&api, arg.done, arg.favorite, arg.from, arg.label, json).await;
    };

    match subcommand {
        VkCommands::Sync(sync_cmd) => {
            let opts = sync::SyncOptions {
                output: sync_cmd.output.into(),
                projects: sync_cmd.project,
                include_done: sync_cmd.done,
                include_archived: sync_cmd.archived,
                dry_run: sync_cmd.dry_run,
                quiet: json,
            };

            let stats = sync::fetch_local(&opts, &api).await;
            if json {
                print_json(&stats)?;
            }
            if stats.errors > 0 {
                return Err(VkError::other(format!(
                    "{} sync operation(s) failed",
                    stats.errors
                )));
            }
            Ok(())
        }
        VkCommands::TaskInfo(task_info_cmd) => {
            show_task_id(task_info_cmd.task_id, &api, json).await
        }
        VkCommands::TaskEdit(task_edit_cmd) => {
            let no_flags = task_edit_cmd.title.is_none()
                && task_edit_cmd.description.is_none()
                && task_edit_cmd.due.is_none()
                && task_edit_cmd.priority.is_none();

            let description = if no_flags {
                require_interactive(
                    "vk edit without --title/--description/--due/--priority",
                    json,
                )?;
                let existing = api.get_task(task_edit_cmd.task_id).await?;
                let current_html = existing.description.clone().unwrap_or_default();
                let current_md = description::html_to_markdown(&current_html);

                let new_md = edit_in_editor(
                    &format!("vk_edit_{}.md", task_edit_cmd.task_id),
                    &current_md,
                )?;
                if new_md == current_md {
                    return Ok(());
                }
                Some(description::markdown_to_html(&new_md))
            } else {
                task_edit_cmd
                    .description
                    .map(|d| description::markdown_to_html(&d))
            };

            let due_date = task_edit_cmd.due.as_deref().map(parse_due).transpose()?;
            let priority = task_edit_cmd
                .priority
                .as_deref()
                .map(parse_priority)
                .transpose()?;

            let task = api
                .edit_task(
                    task_edit_cmd.task_id,
                    task_edit_cmd.title,
                    description,
                    due_date,
                    priority,
                )
                .await?;
            show_task(&task, &api, json).await
        }
        VkCommands::TaskRemove(task_remove_cmd) => {
            api.delete_task(task_remove_cmd.task_id).await?;
            if json {
                print_json(&Deleted {
                    deleted: serde_json::json!({ "task": task_remove_cmd.task_id }),
                })?;
            }
            Ok(())
        }
        VkCommands::TaskDone(task_done_cmd) => {
            let task = api
                .done_task(task_done_cmd.task_id, !task_done_cmd.undo)
                .await?;
            show_task(&task, &api, json).await
        }
        VkCommands::TaskNew(task_new_cmd) => {
            let project = ProjectID::parse(&api, &task_new_cmd.project).await?;
            let description = task_new_cmd
                .description
                .map(|d| description::markdown_to_html(&d));
            let due_date = task_new_cmd.due.as_deref().map(parse_due).transpose()?;
            let priority = task_new_cmd
                .priority
                .as_deref()
                .map(parse_priority)
                .transpose()?;

            let task = api
                .new_task(
                    &task_new_cmd.title,
                    &project,
                    description,
                    due_date,
                    task_new_cmd.favorite,
                    task_new_cmd.label,
                    priority,
                )
                .await?;
            show_task(&task, &api, json).await
        }
        VkCommands::TaskAssign(task_assign_cmd) => {
            let task_id = task_assign_cmd.task_id;
            if task_assign_cmd.undo {
                api.remove_assign_to_task(&task_assign_cmd.user, task_id)
                    .await?;
            } else {
                api.assign_to_task(&task_assign_cmd.user, task_id).await?;
            }
            if json {
                print_json(&api.get_task(task_id).await?)?;
            }
            Ok(())
        }
        VkCommands::TaskComments(task_comments_cmd) => {
            let comments = api.get_task_comments(task_comments_cmd.task_id).await?;

            if json {
                print_json(&comments)?;
            } else {
                for comment in comments {
                    ui::task::print_comment(&comment);
                }
            }
            Ok(())
        }
        VkCommands::TaskComment(task_comment_cmd) => {
            let text = match task_comment_cmd.comment {
                Some(t) => t,
                None => {
                    require_interactive("vk comment without text", json)?;
                    edit_in_editor(&format!("vk_comment_{}.md", task_comment_cmd.task_id), "")?
                }
            };
            if text.trim().is_empty() {
                return Err(VkError::usage("Comment is empty"));
            }
            let comment = api
                .new_comment(
                    task_comment_cmd.task_id,
                    description::markdown_to_html(&text),
                )
                .await?;
            if json {
                print_json(&comment)?;
            }
            Ok(())
        }
        VkCommands::TaskRelation(task_relation_cmd) => {
            let task_id = task_relation_cmd.task_id;
            let sec_task_id = task_relation_cmd.second_task_id;
            let relation = Relation::try_parse(&task_relation_cmd.relation).ok_or_else(|| {
                VkError::usage(format!(
                    "Unknown relation '{}'. Use one of: subtask (sub), parenttask (parent), related, \
                     duplicateof, duplicates, blocking, blocked, precedes, follows, copiedfrom, copiedto",
                    task_relation_cmd.relation
                ))
            })?;

            if task_relation_cmd.delete {
                api.remove_relation(task_id, &relation, sec_task_id).await?;
            } else {
                api.add_relation(task_id, &relation, sec_task_id).await?;
            }

            show_task_id(task_id, &api, json).await
        }
        VkCommands::TaskLabel(task_label_cmd) => {
            if task_label_cmd.undo {
                api.label_task_remove(&task_label_cmd.label, task_label_cmd.task_id)
                    .await?;
            } else {
                api.label_task(&task_label_cmd.label, task_label_cmd.task_id)
                    .await?;
            }
            show_task_id(task_label_cmd.task_id, &api, json).await
        }
        VkCommands::TaskFav(task_fav_cmd) => {
            let task = api
                .fav_task(task_fav_cmd.task_id, !task_fav_cmd.undo)
                .await?;
            show_task(&task, &api, json).await
        }
        VkCommands::TaskCheck(cmd) => {
            require_interactive("vk check", json)?;
            let task_id = cmd.task_id;
            let task = api.get_task(task_id).await?;
            let html = task.description.clone().unwrap_or_default();
            let mut items = description::parse_task_items(&html);
            if items.is_empty() {
                return Err(VkError::not_found(format!(
                    "Task #{task_id} has no checklist items"
                )));
            }
            if ui::check::run_check_tui(&mut items) {
                let states: Vec<bool> = items.iter().map(|(c, _)| *c).collect();
                let new_html = description::apply_task_item_states(&html, &states);
                api.edit_task(task_id, None, Some(new_html), None, None)
                    .await?;
            }
            ui::task::print_task_info(task_id, &api).await
        }
        VkCommands::Timeline(cmd) => timeline(cmd, &api, json).await,
        VkCommands::Stats(_) => {
            if json {
                return Err(VkError::usage(
                    "vk stats has no JSON output; use `vk --json` or `vk prj ls --json`",
                ));
            }
            ui::stats::print_stats(&api, &config).await
        }
        VkCommands::Login(_) => unreachable!(),
        VkCommands::ProjectCmds(project_cmds) => project_commands(project_cmds, &api, json).await,
        VkCommands::Labels(label_cmds) => label_commands(label_cmds, &api, json).await,
    }
}

/// `vk` with no subcommand: the task list.
async fn list_tasks(
    api: &VikunjaAPI,
    done: bool,
    favorite: bool,
    from: Option<String>,
    label: Option<String>,
    json: bool,
) -> Result<()> {
    if !json {
        return ui::task::print_current_tasks(api, done, favorite, from, label).await;
    }

    let has_filters = from.is_some() || label.is_some();
    let mut tasks = if has_filters {
        api.get_all_tasks().await?
    } else {
        api.get_latest_tasks().await?
    };
    if !done {
        tasks.retain(|x| !x.done.unwrap_or_default());
    }
    if favorite {
        tasks.retain(|x| x.is_favorite.unwrap_or_default());
    }
    if let Some(from) = from {
        let p_id = ProjectID::parse(api, &from).await?;
        tasks.retain(|x| x.project_id.unwrap_or_default() == p_id.0 as i32);
    }
    if let Some(label) = label {
        retain_label(&mut tasks, &label);
    }
    print_json(&tasks)
}
