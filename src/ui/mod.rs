use std::io::stdout;

use chrono::{DateTime, Utc};
use crossterm::{
    style::{Color, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};

use crate::api::{Label, VikunjaAPI};

pub mod project;
pub mod task;

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
fn hex_to_color(hex: &str) -> Result<Color, String> {
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
pub fn time_since(event: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(event);

    if duration.num_days() > 0 {
        format!("{}d ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m ago", duration.num_minutes())
    } else {
        "Just now".to_string()
    }
}

fn print_label(label: &Label) {
    let color = hex_to_color(&label.hex_color).unwrap();
    print_color_bg(color, label.title.trim());
}

pub fn print_all_labels(api: &VikunjaAPI) {
    let labels = api.get_all_labels();

    for label in labels {
        print_label(&label);
        print!("\n\n");
    }
}
