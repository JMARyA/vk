use std::io::stdout;

use chrono::{DateTime, Utc};
use crossterm::{
    style::{Color, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};
use vikunjars::models::ModelsLabel;

use crate::api::VikunjaAPI;

pub mod check;
pub mod project;
pub mod stats;
pub mod task;

/// Interpolate between RGB colour stops at evenly-assumed positions.
fn lerp_color(t: f32, stops: &[(f32, (u8, u8, u8))]) -> Color {
    let clamp = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if clamp <= t1 {
            let s = (clamp - t0) / (t1 - t0);
            return Color::Rgb {
                r: (c0.0 as f32 + s * (c1.0 as f32 - c0.0 as f32)) as u8,
                g: (c0.1 as f32 + s * (c1.1 as f32 - c0.1 as f32)) as u8,
                b: (c0.2 as f32 + s * (c1.2 as f32 - c0.2 as f32)) as u8,
            };
        }
    }
    let last = stops.last().unwrap().1;
    Color::Rgb {
        r: last.0,
        g: last.1,
        b: last.2,
    }
}

/// Return a gradient colour for a completion percentage:
/// grey (0 %) → amber → yellow (50 %) → green (100 %).
pub fn progress_color(done: usize, total: usize) -> Color {
    if total == 0 || done == 0 {
        return Color::DarkGrey;
    }
    let pct = done as f32 / total as f32;
    lerp_color(
        pct,
        &[
            (0.0, (190, 100, 30)), // amber  (first item ticked)
            (0.5, (200, 190, 40)), // yellow
            (1.0, (70, 200, 90)),  // green
        ],
    )
}

/// Count `(checked, total)` task-list items in TipTap HTML.
/// Returns `None` when the description has no checklist items at all.
pub fn task_item_counts(html: &str) -> Option<(usize, usize)> {
    let total = html.matches(r#"data-type="taskItem""#).count();
    if total == 0 {
        return None;
    }
    let checked = html.matches(r#"data-checked="true""#).count();
    Some((checked, total))
}

fn html2text_render(html: &str, width: usize) -> String {
    html2text::from_read(std::io::Cursor::new(html), width).expect("unable to render HTML")
}

/// Render a TipTap task list (the HTML *after* the opening `<ul data-type="taskList">` tag),
/// printing each item with ○/● indicators. Returns the HTML remaining after `</ul>`.
fn print_task_list(html: &str, width: usize) -> &str {
    let mut rest = html;
    loop {
        // Done with this task list
        if let Some(end) = rest.strip_prefix("</ul>") {
            return end;
        }

        // Find the next list item
        let Some(li_pos) = rest.find("<li data-checked=") else {
            // No more items — skip to end of </ul>
            return rest.find("</ul>").map(|i| &rest[i + 5..]).unwrap_or("");
        };
        rest = &rest[li_pos..];

        let checked = rest.starts_with(r#"<li data-checked="true""#);

        // Extract inline content from <div><p>…</p></div>
        let content = if let (Some(cs), Some(ce)) = (
            rest.find("<div><p>").map(|i| i + "<div><p>".len()),
            rest.find("</p></div>"),
        ) {
            // Pass the inner HTML through html2text to render inline formatting
            html2text_render(&rest[cs..ce], width).trim().to_string()
        } else {
            String::new()
        };

        print!("  ");
        if checked {
            print_color(Color::Green, "●");
            print!(" ");
            print_color(Color::DarkGrey, &content);
        } else {
            print_color(Color::DarkGrey, "○");
            print!(" {content}");
        }
        println!();

        // Advance past this </li>
        rest = rest.find("</li>").map(|i| &rest[i + 5..]).unwrap_or("");
    }
}

/// Print an HTML description to the terminal, rendering TipTap task-list items
/// as ○ / ● indicators with color.
pub fn print_description(html: &str) {
    if html.is_empty() || html == "<p></p>" {
        return;
    }
    let width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);

    const TASK_LIST_OPEN: &str = r#"<ul data-type="taskList">"#;
    let mut rest = html;

    loop {
        match rest.find(TASK_LIST_OPEN) {
            Some(pos) => {
                // Render everything before the task list normally
                if pos > 0 {
                    print!("{}", html2text_render(&rest[..pos], width));
                }
                rest = print_task_list(&rest[pos + TASK_LIST_OPEN.len()..], width);
                println!(); // blank line after task list
            }
            None => {
                // No more task lists
                print!("{}", html2text_render(rest, width));
                break;
            }
        }
    }
}

/// Print `txt` with a custom `color`
pub fn print_color(color: Color, txt: &str) {
    stdout().execute(SetForegroundColor(color)).unwrap();
    print!("{txt}");
    stdout().execute(SetForegroundColor(Color::Reset)).unwrap();
}

/// Print `txt` with a custom `color` as background
pub fn print_color_bg(color: Color, txt: &str) {
    stdout().execute(SetBackgroundColor(color)).unwrap();
    print!("{txt}");
    stdout().execute(SetBackgroundColor(Color::Reset)).unwrap();
}

/// Convert a HEX Color String into a `Color` struct
pub fn hex_to_color(hex: &str) -> Result<Color, String> {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 6 {
        return Err("Invalid hex color length".to_string());
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid blue component")?;

    Ok(Color::Rgb { r, g, b })
}

/// Parse datetime string
pub fn parse_datetime(datetime_str: &str) -> Option<DateTime<Utc>> {
    if datetime_str == "0001-01-01T00:00:00Z" {
        return None;
    }

    DateTime::parse_from_rfc3339(datetime_str).map_or(None, |dt| Some(dt.with_timezone(&Utc)))
}

/// Return a formatted time duration
pub fn time_relative(event: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(event);

    if duration.num_seconds() == 0 {
        return "Just now".to_string();
    }

    let is_past = duration.num_seconds() > 0;
    let abs_duration = if is_past { duration } else { -duration };

    let time_string = if abs_duration.num_days() > 0 {
        format!("{}d", abs_duration.num_days())
    } else if abs_duration.num_hours() > 0 {
        format!("{}h", abs_duration.num_hours())
    } else if abs_duration.num_minutes() > 0 {
        format!("{}m", abs_duration.num_minutes())
    } else {
        format!("{}s", abs_duration.num_seconds())
    };

    if is_past {
        format!("{time_string} ago")
    } else {
        format!("in {time_string}")
    }
}

fn is_in_past(dt: DateTime<Utc>) -> bool {
    dt < Utc::now()
}

fn print_label(label: &ModelsLabel) {
    let color = hex_to_color(label.hex_color.as_deref().unwrap_or("")).unwrap_or(Color::Reset);
    print_color_bg(color, label.title.as_deref().unwrap_or("").trim());
}

pub async fn print_all_labels(api: &VikunjaAPI) {
    let labels = api.get_all_labels().await;

    for label in labels {
        print_label(&label);
        print!("  ");
    }
    println!();
}
