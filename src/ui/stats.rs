use std::io::{stdout, Write};

use crossterm::{
    style::{Color as CColor, ResetColor, SetForegroundColor},
    QueueableCommand,
};
use ratatui::{
    backend::TestBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Paragraph},
    Terminal,
};

use crate::{api::VikunjaAPI, config::Config, ui::parse_datetime};

fn to_ct(color: Color) -> CColor {
    match color {
        Color::Reset => CColor::Reset,
        Color::Black => CColor::Black,
        Color::Red => CColor::DarkRed,
        Color::Green => CColor::DarkGreen,
        Color::Yellow => CColor::DarkYellow,
        Color::Blue => CColor::DarkBlue,
        Color::Magenta => CColor::DarkMagenta,
        Color::Cyan => CColor::DarkCyan,
        Color::Gray => CColor::Grey,
        Color::DarkGray => CColor::DarkGrey,
        Color::LightRed => CColor::Red,
        Color::LightGreen => CColor::Green,
        Color::LightYellow => CColor::Yellow,
        Color::LightBlue => CColor::Blue,
        Color::LightMagenta => CColor::Magenta,
        Color::LightCyan => CColor::Cyan,
        Color::White => CColor::White,
        Color::Rgb(r, g, b) => CColor::Rgb { r, g, b },
        Color::Indexed(n) => CColor::AnsiValue(n),
    }
}

fn print_buffer(buffer: &ratatui::buffer::Buffer) {
    let mut out = stdout();
    let width = buffer.area.width as usize;
    for (i, cell) in buffer.content.iter().enumerate() {
        if i > 0 && i % width == 0 {
            writeln!(out).unwrap();
        }
        out.queue(SetForegroundColor(to_ct(cell.fg))).unwrap();
        write!(out, "{}", cell.symbol()).unwrap();
    }
    writeln!(out).unwrap();
    out.queue(ResetColor).unwrap();
    out.flush().unwrap();
}

pub async fn print_stats(api: &VikunjaAPI, config: &Config) {
    let tasks = api.get_all_tasks().await;
    let projects = api.get_all_projects().await.unwrap_or_default();
    let labels = api.get_all_labels().await;

    let today = chrono::Utc::now().date_naive();
    let week = today + chrono::Duration::days(7);

    let total = tasks.len();
    let done = tasks.iter().filter(|t| t.done.unwrap_or(false)).count();
    let overdue = tasks
        .iter()
        .filter(|t| {
            !t.done.unwrap_or(false)
                && t.due_date
                    .as_ref()
                    .and_then(|d| parse_datetime(d))
                    .is_some_and(|dt| dt.date_naive() < today)
        })
        .count();
    let due_today = tasks
        .iter()
        .filter(|t| {
            !t.done.unwrap_or(false)
                && t.due_date
                    .as_ref()
                    .and_then(|d| parse_datetime(d))
                    .is_some_and(|dt| dt.date_naive() == today)
        })
        .count();
    let due_week = tasks
        .iter()
        .filter(|t| {
            !t.done.unwrap_or(false)
                && t.due_date
                    .as_ref()
                    .and_then(|d| parse_datetime(d))
                    .is_some_and(|dt| {
                        let d = dt.date_naive();
                        d > today && d <= week
                    })
        })
        .count();

    let project_count = projects.iter().filter(|p| p.id.unwrap_or(0) > 0).count();
    let label_count = labels.len();
    let host = config
        .host
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let secondary =
        format!("{done} done  ·  {project_count} projects  ·  {label_count} labels  ·  {host}");

    // Outer box: 62 wide × 10 tall → inner 60 wide × 8 tall
    let backend = TestBackend::new(62, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    let border_color = Color::Rgb(99, 99, 99);
    let dim_color = Color::Rgb(80, 80, 80);
    let bright_color = Color::Rgb(230, 230, 230);
    let accent_color = Color::Rgb(0, 200, 255);
    let warn_color = Color::Rgb(255, 80, 80);

    terminal
        .draw(|frame| {
            let area = frame.area();

            let block = Block::bordered()
                .title(" vk ")
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            // 8 inner rows
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // empty
                    Constraint::Length(1), // stat numbers
                    Constraint::Length(1), // stat labels
                    Constraint::Length(1), // empty
                    Constraint::Length(1), // divider
                    Constraint::Length(1), // empty
                    Constraint::Length(1), // info line
                    Constraint::Length(1), // empty
                ])
                .split(inner);

            // 4-column split for numbers and labels
            let quad = [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ];
            let num_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(quad)
                .split(rows[1]);
            let lbl_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(quad)
                .split(rows[2]);

            let stats = [
                (total, false),
                (overdue, overdue > 0),
                (due_today, false),
                (due_week, false),
            ];
            let stat_labels = ["tasks", "overdue", "today", "this week"];

            for (i, (val, warn)) in stats.iter().enumerate() {
                let color = if *warn { warn_color } else { bright_color };
                frame.render_widget(
                    Paragraph::new(format!("{val}"))
                        .style(Style::default().fg(color))
                        .alignment(Alignment::Center),
                    num_cols[i],
                );
                frame.render_widget(
                    Paragraph::new(stat_labels[i])
                        .style(Style::default().fg(dim_color))
                        .alignment(Alignment::Center),
                    lbl_cols[i],
                );
            }

            // Divider: fill inner width minus 2 spaces of padding
            let div_width = inner.width.saturating_sub(2) as usize;
            frame.render_widget(
                Paragraph::new(format!(" {} ", "─".repeat(div_width)))
                    .style(Style::default().fg(dim_color)),
                rows[4],
            );

            // Info line — ratatui clips automatically if too wide
            frame.render_widget(
                Paragraph::new(format!("  {secondary}")).style(Style::default().fg(accent_color)),
                rows[6],
            );
        })
        .unwrap();

    println!();
    print_buffer(terminal.backend().buffer());
}
