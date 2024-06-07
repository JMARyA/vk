use std::io::stdout;

use chrono::{DateTime, Utc};
use crossterm::{
    style::{Color, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};

use crate::api::{Label, VikunjaAPI};

pub mod project;
pub mod task;

fn format_html_to_terminal(html: &str) -> String {
    let width = crossterm::terminal::size().unwrap().0 as usize;
    let cursor = std::io::Cursor::new(html);
    html2text::from_read(cursor, width)
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
fn parse_datetime(datetime_str: &str) -> Option<DateTime<Utc>> {
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

fn print_label(label: &Label) {
    let color = hex_to_color(&label.hex_color).unwrap_or(Color::Reset);
    print_color_bg(color, label.title.trim());
}

pub fn print_all_labels(api: &VikunjaAPI) {
    let labels = api.get_all_labels();

    for label in labels {
        print_label(&label);
        print!("\n\n");
    }
}
