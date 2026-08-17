use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;
use vikunjars::models::{ModelsProject, ModelsTask, ModelsTaskComment};

use crate::{
    api::{ProjectID, Relation, VikunjaAPI},
    ui::{
        hex_to_color, is_in_past, parse_datetime, print_color, print_description, print_label,
        progress_color, task_item_counts, time_relative,
    },
};

fn print_task_list(tasks: &[ModelsTask], projects: &[ModelsProject], show_project: bool) {
    if tasks.is_empty() {
        return;
    }

    let max_id_len = tasks
        .iter()
        .map(|t| t.id.unwrap_or(0).to_string().len())
        .max()
        .unwrap_or(1);

    let max_title_len = tasks
        .iter()
        .map(|t| t.title.as_deref().unwrap_or("").width())
        .max()
        .unwrap_or(10)
        .min(45);

    let max_proj_len = if show_project {
        tasks
            .iter()
            .filter_map(|t| {
                projects
                    .iter()
                    .find(|p| p.id == t.project_id)
                    .and_then(|p| p.title.as_ref())
                    .map(|name| name.width() + 2)
            })
            .max()
            .unwrap_or(0)
    } else {
        0
    };

    // Pre-compute date strings so we can measure max width for alignment.
    let date_strings: Vec<String> = tasks
        .iter()
        .map(|t| {
            let due = t.due_date.as_deref().and_then(parse_datetime);
            let is_done = t.done.unwrap_or_default();
            let is_overdue = due.map(is_in_past).unwrap_or(false) && !is_done;
            if let Some(due_dt) = due {
                if is_overdue {
                    format!("overdue {}", time_relative(due_dt).replace(" ago", ""))
                } else {
                    format!("due {}", time_relative(due_dt))
                }
            } else if let Some(start_dt) = t.start_date.as_deref().and_then(parse_datetime) {
                if !is_in_past(start_dt) {
                    format!("starts {}", time_relative(start_dt))
                } else {
                    t.created
                        .as_deref()
                        .and_then(parse_datetime)
                        .map(time_relative)
                        .unwrap_or_default()
                }
            } else {
                t.created
                    .as_deref()
                    .and_then(parse_datetime)
                    .map(time_relative)
                    .unwrap_or_default()
            }
        })
        .collect();

    let max_date_len = date_strings.iter().map(|s| s.len()).max().unwrap_or(0);

    for (task, date_str) in tasks.iter().zip(date_strings.iter()) {
        let due = task.due_date.as_deref().and_then(parse_datetime);
        let is_done = task.done.unwrap_or_default();
        let is_overdue = due.map(is_in_past).unwrap_or(false) && !is_done;
        let base_color = if is_overdue {
            crossterm::style::Color::Red
        } else {
            crossterm::style::Color::Reset
        };

        // Status: ✓ done, ★ favorite, space otherwise
        if is_done {
            print_color(crossterm::style::Color::Green, "✓");
        } else if task.is_favorite.unwrap_or_default() {
            print_color(crossterm::style::Color::Yellow, "★");
        } else {
            print!(" ");
        }

        // Bullet
        print_color(base_color, " ◆  ");

        // ID right-aligned
        print_color(
            base_color,
            &format!("{:>width$}", task.id.unwrap_or(0), width = max_id_len),
        );
        print!("  ");

        // Title, truncated with ellipsis if needed
        let title = task.title.as_deref().unwrap_or("");
        let (display_title, visual_len) = if title.width() > max_title_len {
            let mut w = 0;
            let mut truncated = String::new();
            for ch in title.chars() {
                let cw = ch.width().unwrap_or(0);
                if w + cw > max_title_len - 1 {
                    break;
                }
                truncated.push(ch);
                w += cw;
            }
            truncated.push('…');
            (truncated, max_title_len)
        } else {
            let w = title.width();
            (title.to_string(), w)
        };
        print_color(
            if is_overdue {
                crossterm::style::Color::Red
            } else {
                crossterm::style::Color::Blue
            },
            &display_title,
        );
        print!("{}", " ".repeat(max_title_len - visual_len + 2));

        // Labels inline after title
        if let Some(labels) = &task.labels {
            if !labels.is_empty() {
                for label in labels {
                    print_label(label);
                    print!(" ");
                }
                print!(" ");
            }
        }

        // Project column (hidden when filtering by project)
        if show_project {
            if let Some(proj) = projects.iter().find(|p| p.id == task.project_id) {
                let name = proj.title.as_deref().unwrap_or("");
                let proj_display = format!("[{name}]");
                let proj_pad = max_proj_len.saturating_sub(proj_display.width());
                let color = proj
                    .hex_color
                    .as_deref()
                    .and_then(|h| hex_to_color(h).ok())
                    .unwrap_or(crossterm::style::Color::Reset);
                print_color(color, &proj_display);
                print!("{}", " ".repeat(proj_pad + 2));
            }
        }

        // Date — padded to max width so the subtask badge column aligns
        print_color(
            if is_overdue {
                crossterm::style::Color::Red
            } else {
                crossterm::style::Color::DarkGrey
            },
            date_str,
        );

        // Subtask progress badge at end of row
        if let Some((done, total)) = task.description.as_deref().and_then(task_item_counts) {
            let pad = max_date_len - date_str.len();
            print!("{}", " ".repeat(pad));
            print_color(progress_color(done, total), &format!("  [{done}/{total}]"));
        }

        println!();
    }
}

pub async fn print_current_tasks(
    api: &VikunjaAPI,
    done: bool,
    fav: bool,
    project: Option<String>,
    label: Option<String>,
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

    let show_project = project.is_none();

    if let Some(project) = project {
        let p_id = ProjectID::parse(api, &project).await.unwrap();
        selection.retain(|x| x.project_id.unwrap_or_default() == p_id.0 as i32);
    }

    if let Some(label_match) = label {
        selection.retain(|x| {
            if let Some(labels) = &x.labels {
                for label in labels {
                    if label.title.as_deref().unwrap_or("").trim() == label_match {
                        return true;
                    }
                }
            }
            false
        });
    }

    let projects = api.get_all_projects().await.unwrap();

    print_task_list(&selection, &projects, show_project);
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

    let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);

    let is_done = task.done.unwrap_or_default();
    let is_fav = task.is_favorite.unwrap_or_default();
    let priority = task.priority.unwrap_or_default();

    let project = api
        .get_all_projects()
        .await
        .unwrap()
        .into_iter()
        .find(|p| p.id == task.project_id);

    // ── Header ───────────────────────────────────────────────────────
    if is_done {
        print_color(crossterm::style::Color::Green, "✓ ");
    } else if is_fav {
        print_color(crossterm::style::Color::Yellow, "★ ");
    }
    print_color(
        crossterm::style::Color::Blue,
        task.title.as_deref().unwrap_or(""),
    );
    print_color(
        crossterm::style::Color::DarkGrey,
        &format!("  #{}", task.id.unwrap_or(0)),
    );
    if let Some(proj) = &project {
        let name = proj.title.as_deref().unwrap_or("");
        let color = proj
            .hex_color
            .as_deref()
            .and_then(|h| hex_to_color(h).ok())
            .unwrap_or(crossterm::style::Color::Reset);
        print_color(color, &format!("  [{name}]"));
    }
    println!();

    // ── Meta: author · created · updated ─────────────────────────────
    {
        let mut parts: Vec<String> = Vec::new();
        if let Some(user) = task.created_by {
            if let Some(name) = user.username {
                parts.push(name);
            }
        }
        if let Some(dt) = task.created.as_deref().and_then(parse_datetime) {
            parts.push(time_relative(dt));
        }
        if let Some(dt) = task.updated.as_deref().and_then(parse_datetime) {
            parts.push(format!("updated {}", time_relative(dt)));
        }
        if !parts.is_empty() {
            print_color(crossterm::style::Color::DarkGrey, &parts.join("  ·  "));
            println!();
        }
    }

    // ── Done timestamp ───────────────────────────────────────────────
    if is_done {
        if let Some(dt) = task.done_at.as_deref().and_then(parse_datetime) {
            print_color(
                crossterm::style::Color::Green,
                &format!("done {}", time_relative(dt)),
            );
            println!();
        }
    }

    // ── Due date ─────────────────────────────────────────────────────
    if let Some(due_dt) = task.due_date.as_deref().and_then(parse_datetime) {
        let overdue = is_in_past(due_dt) && !is_done;
        let label = if overdue {
            format!("overdue {}", time_relative(due_dt).replace(" ago", ""))
        } else {
            format!("due {}", time_relative(due_dt))
        };
        print_color(
            if overdue {
                crossterm::style::Color::Red
            } else {
                crossterm::style::Color::Reset
            },
            &label,
        );
        println!();
    }

    // ── Priority ─────────────────────────────────────────────────────
    if priority != 0 {
        let (label, color) = match priority {
            1 => ("↑ low", crossterm::style::Color::DarkGrey),
            2 => ("↑↑ medium", crossterm::style::Color::Yellow),
            3 => ("↑↑↑ high", crossterm::style::Color::DarkYellow),
            4 => ("!! urgent", crossterm::style::Color::Red),
            _ => ("!!! do now", crossterm::style::Color::DarkRed),
        };
        print_color(color, label);
        println!();
    }

    // ── Labels ───────────────────────────────────────────────────────
    if let Some(labels) = task.labels {
        if !labels.is_empty() {
            for label in &labels {
                print_label(label);
                print!(" ");
            }
            println!();
        }
    }

    // ── Assignees ────────────────────────────────────────────────────
    if let Some(assigned) = task.assignees {
        if !assigned.is_empty() {
            print_color(crossterm::style::Color::DarkGrey, "assigned  ");
            for a in &assigned {
                print!("{} ", a.username.as_deref().unwrap_or(""));
            }
            println!();
        }
    }

    // ── Relations ────────────────────────────────────────────────────
    if let Some(related) = task.related_tasks {
        for (kind, tasks) in &related {
            if let Some(rel) = Relation::try_parse(kind) {
                print_color(
                    crossterm::style::Color::DarkGrey,
                    &format!("{}  ", rel.repr()),
                );
            }
            for t in tasks {
                let done = t.done.unwrap_or_default();
                if done {
                    print_color(crossterm::style::Color::Green, "✓ ");
                }
                print_color(
                    if done {
                        crossterm::style::Color::DarkGrey
                    } else {
                        crossterm::style::Color::Blue
                    },
                    t.title.as_deref().unwrap_or(""),
                );
                print_color(
                    crossterm::style::Color::DarkGrey,
                    &format!(" #{}", t.id.unwrap_or(0)),
                );
                print!("  ");
            }
            println!();
        }
    }

    // ── Description ──────────────────────────────────────────────────
    let desc = task.description.unwrap_or_default();
    if desc != "<p></p>" && !desc.is_empty() {
        // Subtask summary line above the divider
        if let Some((done, total)) = task_item_counts(&desc) {
            let color = progress_color(done, total);
            let icon = if done == total {
                "●"
            } else if done > 0 {
                "◐"
            } else {
                "○"
            };
            print_color(color, &format!("{icon}  {done}/{total}"));
            print_color(crossterm::style::Color::DarkGrey, " subtasks");
            println!();
        }
        println!("{}", "─".repeat(term_width));
        print_description(&desc);
    }
}

pub fn print_comment(comment: &ModelsTaskComment) {
    print_color(
        crossterm::style::Color::Blue,
        comment
            .author
            .as_ref()
            .and_then(|a| a.username.as_deref())
            .unwrap_or(""),
    );
    if let Some(dt) = comment.created.as_deref().and_then(parse_datetime) {
        print_color(
            crossterm::style::Color::DarkGrey,
            &format!("  ·  {}", time_relative(dt)),
        );
    }
    println!();
    print_description(comment.comment.as_deref().unwrap_or(""));
    println!();
}
