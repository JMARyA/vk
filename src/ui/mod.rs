use std::io::stdout;

use chrono::{DateTime, Utc};
use crossterm::{
    style::{Color, SetForegroundColor},
    ExecutableCommand,
};

pub mod project;
pub mod task;

pub fn print_color(color: Color, txt: &str) {
    stdout().execute(SetForegroundColor(color)).unwrap();
    print!("{txt}");
    stdout().execute(SetForegroundColor(Color::Reset)).unwrap();
}

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

fn parse_datetime(datetime_str: &str) -> Option<DateTime<Utc>> {
    if datetime_str == "0001-01-01T00:00:00Z" {
        return None;
    }

    match DateTime::parse_from_rfc3339(datetime_str) {
        Ok(dt) => Some(dt.with_timezone(&Utc)),
        Err(_) => None, // Return None if parsing fails
    }
}

pub fn time_since(event: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(event);

    if duration.num_days() > 0 {
        return format!("{}d ago", duration.num_days());
    } else if duration.num_hours() > 0 {
        return format!("{}h ago", duration.num_hours());
    } else if duration.num_minutes() > 0 {
        return format!("{}m ago", duration.num_minutes());
    } else {
        return "Just now".to_string();
    }
}
