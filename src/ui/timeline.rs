//! Full-screen timeline of tasks, laid out by their date fields.
//!
//! Each task with at least one date gets a row. `start_date`..`end_date` is
//! drawn as a bar, `due_date` and `done_at` as markers on top of it, and a
//! vertical rule marks the current time. Layout is kept in pure functions
//! (`Viewport`, `row_cells`, `axis_labels`) so it can be tested without a
//! terminal; `run_timeline` only turns their output into widgets.

use std::io;

use chrono::{
    DateTime, Datelike, Duration, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, Timelike,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use vikunjars::models::{ModelsProject, ModelsTask};

use crate::ui::parse_datetime;

// ----------------------------------------------------------------- entry ---

/// A task reduced to what the timeline draws, with dates in local time.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: i32,
    pub title: String,
    pub project: String,
    pub color: Option<Color>,
    pub done: bool,
    pub start: Option<NaiveDateTime>,
    pub end: Option<NaiveDateTime>,
    pub due: Option<NaiveDateTime>,
    pub done_at: Option<NaiveDateTime>,
}

fn to_local(s: Option<&str>) -> Option<NaiveDateTime> {
    s.and_then(parse_datetime)
        .map(|dt| DateTime::<Local>::from(dt).naive_local())
}

/// Parse a Vikunja hex colour (with or without `#`); empty means none.
fn parse_color(hex: Option<&str>) -> Option<Color> {
    let hex = hex?.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    format!("#{hex}").parse().ok()
}

impl Entry {
    /// Build an entry, or `None` when the task has no date to place it by.
    pub fn from_task(task: &ModelsTask, projects: &[ModelsProject]) -> Option<Self> {
        let done = task.done.unwrap_or_default();
        let project = projects.iter().find(|p| p.id == task.project_id);

        let entry = Entry {
            id: task.id.unwrap_or_default(),
            title: task.title.clone().unwrap_or_default(),
            project: project.and_then(|p| p.title.clone()).unwrap_or_default(),
            color: parse_color(task.hex_color.as_deref())
                .or_else(|| parse_color(project.and_then(|p| p.hex_color.as_deref()))),
            done,
            start: to_local(task.start_date.as_deref()),
            end: to_local(task.end_date.as_deref()),
            due: to_local(task.due_date.as_deref()),
            // Undone tasks can keep a stale done_at from a previous completion.
            done_at: if done {
                to_local(task.done_at.as_deref())
            } else {
                None
            },
        };

        entry.dates().next().is_some().then_some(entry)
    }

    fn dates(&self) -> impl Iterator<Item = NaiveDateTime> {
        [self.start, self.end, self.due, self.done_at]
            .into_iter()
            .flatten()
    }

    /// The earliest date on the task; entries always have at least one.
    pub fn earliest(&self) -> NaiveDateTime {
        self.dates().min().unwrap_or_default()
    }

    fn is_overdue(&self, now: NaiveDateTime) -> bool {
        !self.done && self.due.is_some_and(|d| d < now)
    }
}

// ------------------------------------------------------------- viewport ---

/// How much time one terminal column covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Quarter,
    Day,
    Week,
    Month,
}

impl Scale {
    fn minutes(self) -> i64 {
        match self {
            Scale::Quarter => 6 * 60,
            Scale::Day => 24 * 60,
            Scale::Week => 7 * 24 * 60,
            Scale::Month => 30 * 24 * 60,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Scale::Quarter => "6 hours",
            Scale::Day => "day",
            Scale::Week => "week",
            Scale::Month => "month",
        }
    }

    fn zoom_in(self) -> Self {
        match self {
            Scale::Month => Scale::Week,
            Scale::Week => Scale::Day,
            _ => Scale::Quarter,
        }
    }

    fn zoom_out(self) -> Self {
        match self {
            Scale::Quarter => Scale::Day,
            Scale::Day => Scale::Week,
            _ => Scale::Month,
        }
    }
}

/// Round `t` down to a boundary of `scale`, so columns start on midnight,
/// Mondays, or the first of the month rather than wherever the view began.
fn snap(t: NaiveDateTime, scale: Scale) -> NaiveDateTime {
    let midnight = t.date().and_time(NaiveTime::MIN);
    match scale {
        Scale::Quarter => midnight + Duration::hours((t.hour() / 6 * 6) as i64),
        Scale::Day => midnight,
        Scale::Week => midnight - Duration::days(t.weekday().num_days_from_monday() as i64),
        Scale::Month => t
            .date()
            .with_day(1)
            .unwrap_or(t.date())
            .and_time(NaiveTime::MIN),
    }
}

/// Maps time to terminal columns: column 0 starts at `origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub origin: NaiveDateTime,
    pub scale: Scale,
}

impl Viewport {
    /// A viewport that shows `t` in column `col`.
    pub fn anchored(t: NaiveDateTime, scale: Scale, col: i64) -> Self {
        Viewport {
            origin: snap(t - Duration::minutes(scale.minutes() * col), scale),
            scale,
        }
    }

    /// Column containing `t`; negative or past the width when off-screen.
    pub fn col(&self, t: NaiveDateTime) -> i64 {
        (t - self.origin)
            .num_minutes()
            .div_euclid(self.scale.minutes())
    }

    /// Start of column `col`.
    pub fn time_at(&self, col: i64) -> NaiveDateTime {
        self.origin + Duration::minutes(col * self.scale.minutes())
    }

    fn pan(&mut self, cols: i64) {
        self.origin += Duration::minutes(cols * self.scale.minutes());
    }

    /// Change scale, keeping the time under column `pivot` in place.
    fn zoom(&mut self, scale: Scale, pivot: i64) {
        *self = Viewport::anchored(self.time_at(pivot), scale, pivot);
    }
}

// ----------------------------------------------------------------- axis ---

fn advance(d: NaiveDate, scale: Scale) -> NaiveDate {
    match scale {
        Scale::Quarter => d + Duration::days(1),
        Scale::Day => d + Duration::days(7),
        Scale::Week => d + Months::new(1),
        Scale::Month => d + Months::new(12),
    }
}

/// First axis tick at or after `t`: midnights when zoomed to 6 hours, Mondays
/// for days, month starts for weeks, and year starts for months.
fn next_tick(t: NaiveDateTime, scale: Scale) -> NaiveDateTime {
    let day = t.date();
    let first = match scale {
        Scale::Quarter => day,
        Scale::Day => day - Duration::days(day.weekday().num_days_from_monday() as i64),
        Scale::Week => day.with_day(1).unwrap_or(day),
        Scale::Month => NaiveDate::from_ymd_opt(day.year(), 1, 1).unwrap_or(day),
    };

    let mut tick = first.and_time(NaiveTime::MIN);
    while tick < t {
        tick = advance(tick.date(), scale).and_time(NaiveTime::MIN);
    }
    tick
}

fn tick_label(t: NaiveDateTime, scale: Scale) -> String {
    let fmt = match scale {
        Scale::Quarter if t.day() == 1 => "%b %-d",
        Scale::Quarter => "%a %-d",
        Scale::Day => "%b %-d",
        Scale::Week if t.month() == 1 => "%Y",
        Scale::Week => "%b",
        Scale::Month => "%Y",
    };
    t.format(fmt).to_string()
}

/// Columns in `0..width` that contain an axis tick.
pub fn axis_ticks(vp: &Viewport, width: usize) -> Vec<(usize, NaiveDateTime)> {
    (0..width)
        .filter_map(|c| {
            let tick = next_tick(vp.time_at(c as i64), vp.scale);
            (tick < vp.time_at(c as i64 + 1)).then_some((c, tick))
        })
        .collect()
}

/// Tick labels that fit, left to right, without touching each other.
pub fn axis_labels(vp: &Viewport, width: usize) -> Vec<(usize, String)> {
    let mut labels = Vec::new();
    let mut free_from = 0;

    for (col, tick) in axis_ticks(vp, width) {
        let label = tick_label(tick, vp.scale);
        if col < free_from || col + label.width() > width {
            continue;
        }
        free_from = col + label.width() + 1;
        labels.push((col, label));
    }

    labels
}

// ------------------------------------------------------------------ rows ---

/// What a single column of a task row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Today,
    Bar,
    Start,
    End,
    Due,
    Overdue,
    Done,
    /// Everything on the row lies left of the view.
    Before,
    /// Everything on the row lies right of the view.
    After,
}

impl Cell {
    fn symbol(self) -> char {
        match self {
            Cell::Empty => ' ',
            Cell::Today => '┊',
            Cell::Bar => '━',
            Cell::Start => '▶',
            Cell::End => '■',
            Cell::Due | Cell::Overdue => '◆',
            Cell::Done => '✓',
            Cell::Before => '◂',
            Cell::After => '▸',
        }
    }

    fn style(self, entry: &Entry) -> Style {
        let base = if entry.done {
            Color::DarkGray
        } else {
            entry.color.unwrap_or(Color::Blue)
        };
        match self {
            Cell::Empty => Style::default(),
            Cell::Today => Style::default().fg(Color::Red),
            Cell::Bar | Cell::Start | Cell::End => Style::default().fg(base),
            Cell::Due => Style::default().fg(Color::Yellow),
            Cell::Overdue => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Cell::Done => Style::default().fg(Color::Green),
            Cell::Before | Cell::After => Style::default().fg(Color::DarkGray),
        }
    }
}

/// Lay out one task row across `width` columns.
pub fn row_cells(entry: &Entry, vp: &Viewport, width: usize, now: NaiveDateTime) -> Vec<Cell> {
    let w = width as i64;
    let mut cells = vec![Cell::Empty; width];
    let put = |cells: &mut [Cell], t: NaiveDateTime, cell: Cell| {
        let c = vp.col(t);
        if (0..w).contains(&c) {
            cells[c as usize] = cell;
        }
    };

    put(&mut cells, now, Cell::Today);

    match (entry.start, entry.end) {
        (Some(a), Some(b)) => {
            let (from, to) = (vp.col(a.min(b)).max(0), vp.col(a.max(b)).min(w - 1));
            for c in from..=to {
                cells[c as usize] = Cell::Bar;
            }
        }
        (Some(a), None) => put(&mut cells, a, Cell::Start),
        (None, Some(b)) => put(&mut cells, b, Cell::End),
        (None, None) => {}
    }

    if let Some(due) = entry.due {
        let cell = if entry.is_overdue(now) {
            Cell::Overdue
        } else {
            Cell::Due
        };
        put(&mut cells, due, cell);
    }
    if let Some(done_at) = entry.done_at {
        put(&mut cells, done_at, Cell::Done);
    }

    // Nothing landed on screen: point to where the task is instead.
    if width > 0 && cells.iter().all(|c| matches!(c, Cell::Empty | Cell::Today)) {
        let cols: Vec<i64> = entry.dates().map(|t| vp.col(t)).collect();
        if cols.iter().any(|&c| c < 0) {
            cells[0] = Cell::Before;
        }
        if cols.iter().any(|&c| c >= w) {
            cells[width - 1] = Cell::After;
        }
    }

    cells
}

// ------------------------------------------------------------------- tui ---

/// Truncate to `width` display columns with an ellipsis, padded to `width`.
fn fit(s: &str, width: usize) -> String {
    if s.width() <= width {
        return format!("{s}{}", " ".repeat(width - s.width()));
    }

    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw + 1 > width {
            break;
        }
        out.push(ch);
        used += cw;
    }
    if width > 0 {
        out.push('…');
        used += 1;
    }
    format!("{out}{}", " ".repeat(width.saturating_sub(used)))
}

fn fmt_date(t: NaiveDateTime, now: NaiveDateTime) -> String {
    let mut fmt = String::from("%b %-d");
    if t.year() != now.year() {
        fmt.push_str(" %Y");
    }
    if t.time() != NaiveTime::MIN {
        fmt.push_str(" %H:%M");
    }
    t.format(&fmt).to_string()
}

fn details(entry: &Entry, now: NaiveDateTime) -> Vec<Span<'static>> {
    let dim = Style::default().fg(Color::DarkGray);
    let mut spans = vec![
        Span::styled(format!(" #{} ", entry.id), dim),
        Span::styled(
            entry.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ];
    if !entry.project.is_empty() {
        spans.push(Span::styled(format!("  {}", entry.project), dim));
    }

    let mut field = |label: &str, t: Option<NaiveDateTime>, style: Style| {
        if let Some(t) = t {
            spans.push(Span::styled(format!("  {label} "), dim));
            spans.push(Span::styled(fmt_date(t, now), style));
        }
    };
    field("start", entry.start, Style::default());
    field("end", entry.end, Style::default());
    let due_style = if entry.is_overdue(now) {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Yellow)
    };
    field("due", entry.due, due_style);
    field("done", entry.done_at, Style::default().fg(Color::Green));

    spans
}

struct State {
    selected: usize,
    scroll: usize,
    vp: Viewport,
}

/// Render one frame; returns the width of the timeline area in columns.
fn draw(
    f: &mut Frame,
    entries: &[Entry],
    hidden: usize,
    st: &mut State,
    now: NaiveDateTime,
) -> i64 {
    let [header, axis, body, info, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    let dim = Style::default().fg(Color::DarkGray);
    let gutter = entries
        .iter()
        .map(|e| format!("#{} {}", e.id, e.title).width() + 1)
        .max()
        .unwrap_or(0)
        .min(32)
        .min(body.width as usize / 3);
    let width = (body.width as usize).saturating_sub(gutter);

    // Axis labels omit the month or year at some scales, so the header
    // carries the full visible range.
    let last = st.vp.time_at(width as i64) - Duration::minutes(1);
    let mut title = format!(
        " vk timeline · {} – {} · 1 column = {} · {} tasks",
        st.vp.time_at(0).format("%b %-d %Y"),
        last.format("%b %-d %Y"),
        st.vp.scale.name(),
        entries.len()
    );
    if hidden > 0 {
        title.push_str(&format!(" ({hidden} without dates hidden)"));
    }
    f.render_widget(
        Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
        header,
    );

    // Axis: labels on the first line, tick marks and today on the second.
    let today = st.vp.col(now);
    let mut label_line = vec![' '; width];
    for (col, label) in axis_labels(&st.vp, width) {
        for (i, ch) in label.chars().enumerate() {
            if let Some(slot) = label_line.get_mut(col + i) {
                *slot = ch;
            }
        }
    }
    let mut ticks: Vec<Span> = vec![Span::raw(" ".repeat(gutter))];
    let tick_cols: Vec<usize> = axis_ticks(&st.vp, width).iter().map(|t| t.0).collect();
    for c in 0..width {
        ticks.push(if c as i64 == today {
            Span::styled("▼", Style::default().fg(Color::Red))
        } else if tick_cols.contains(&c) {
            Span::styled("╵", dim)
        } else {
            Span::raw(" ")
        });
    }
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" ".repeat(gutter)),
                Span::styled(label_line.into_iter().collect::<String>(), dim),
            ]),
            Line::from(ticks),
        ]),
        axis,
    );

    // Keep the selection on screen.
    let rows = body.height as usize;
    if st.selected < st.scroll {
        st.scroll = st.selected;
    } else if rows > 0 && st.selected >= st.scroll + rows {
        st.scroll = st.selected + 1 - rows;
    }

    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .skip(st.scroll)
        .take(rows)
        .map(|(i, e)| {
            let mut name_style = if e.done { dim } else { Style::default() };
            if i == st.selected {
                name_style = name_style.add_modifier(Modifier::REVERSED);
            }
            let mut spans = vec![Span::styled(
                fit(&format!("#{} {}", e.id, e.title), gutter.saturating_sub(1)),
                name_style,
            )];
            spans.push(Span::raw(if gutter > 0 { " " } else { "" }));
            spans.extend(
                row_cells(e, &st.vp, width, now)
                    .into_iter()
                    .map(|c| Span::styled(c.symbol().to_string(), c.style(e))),
            );
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), body);

    if let Some(e) = entries.get(st.selected) {
        f.render_widget(Paragraph::new(Line::from(details(e, now))), info);
    }
    f.render_widget(
        Paragraph::new(
            " j/k select · h/l scroll · H/L page · +/- zoom · t today · c center · ⏎ info · q quit",
        )
        .style(dim),
        help,
    );

    width as i64
}

/// Show the timeline until the user quits. Returns the id of the task picked
/// with enter, if any. `hidden` is the number of tasks left out for having no
/// dates, shown in the header.
pub fn run_timeline(entries: &[Entry], hidden: usize) -> io::Result<Option<i32>> {
    let mut terminal = ratatui::try_init()?;
    let now = Local::now().naive_local();

    let mut st = State {
        selected: entries
            .iter()
            .position(|e| e.dates().any(|t| t >= now))
            .unwrap_or(0),
        scroll: 0,
        vp: Viewport::anchored(now, Scale::Day, 0),
    };
    let mut placed = false;

    let picked = loop {
        let mut timeline_width = 0;
        terminal.draw(|f| timeline_width = draw(f, entries, hidden, &mut st, now))?;

        // The timeline width is only known once laid out: put today a quarter
        // of the way in on the first frame.
        if !placed {
            placed = true;
            st.vp = Viewport::anchored(now, Scale::Day, timeline_width / 4);
            continue;
        }
        let page = (timeline_width / 2).max(1);

        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break None,
            KeyCode::Enter => break entries.get(st.selected).map(|e| e.id),
            KeyCode::Down | KeyCode::Char('j') => {
                if st.selected + 1 < entries.len() {
                    st.selected += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => st.selected = st.selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => st.selected = 0,
            KeyCode::Char('G') | KeyCode::End => st.selected = entries.len().saturating_sub(1),
            KeyCode::Left | KeyCode::Char('h') => st.vp.pan(-1),
            KeyCode::Right | KeyCode::Char('l') => st.vp.pan(1),
            KeyCode::Char('H') | KeyCode::PageUp => st.vp.pan(-page),
            KeyCode::Char('L') | KeyCode::PageDown => st.vp.pan(page),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                st.vp.zoom(st.vp.scale.zoom_in(), timeline_width / 2)
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                st.vp.zoom(st.vp.scale.zoom_out(), timeline_width / 2)
            }
            KeyCode::Char('t') => st.vp = Viewport::anchored(now, st.vp.scale, timeline_width / 4),
            KeyCode::Char('c') => {
                if let Some(e) = entries.get(st.selected) {
                    st.vp = Viewport::anchored(e.earliest(), st.vp.scale, timeline_width / 4);
                }
            }
            _ => {}
        }
    };

    ratatui::restore();
    Ok(picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, 0, 0)
            .unwrap()
    }

    fn entry() -> Entry {
        Entry {
            id: 1,
            title: "t".into(),
            project: String::new(),
            color: None,
            done: false,
            start: None,
            end: None,
            due: None,
            done_at: None,
        }
    }

    fn day_view() -> Viewport {
        // Monday 2026-09-28, one column per day.
        Viewport::anchored(at(2026, 9, 28, 0), Scale::Day, 0)
    }

    #[test]
    fn test_snap_aligns_to_scale_boundaries() {
        let t = at(2026, 9, 30, 15); // Wednesday afternoon
        assert_eq!(snap(t, Scale::Quarter), at(2026, 9, 30, 12));
        assert_eq!(snap(t, Scale::Day), at(2026, 9, 30, 0));
        assert_eq!(snap(t, Scale::Week), at(2026, 9, 28, 0));
        assert_eq!(snap(t, Scale::Month), at(2026, 9, 1, 0));
    }

    #[test]
    fn test_viewport_maps_time_to_columns() {
        let vp = day_view();
        assert_eq!(vp.col(at(2026, 9, 28, 23)), 0);
        assert_eq!(vp.col(at(2026, 9, 30, 1)), 2);
        assert_eq!(vp.col(at(2026, 9, 27, 12)), -1);
        assert_eq!(vp.time_at(3), at(2026, 10, 1, 0));
    }

    #[test]
    fn test_anchored_places_time_in_requested_column() {
        let t = at(2026, 9, 30, 15);
        for scale in [Scale::Quarter, Scale::Day, Scale::Week] {
            assert_eq!(Viewport::anchored(t, scale, 10).col(t), 10, "{scale:?}");
        }
    }

    #[test]
    fn test_zoom_keeps_pivot_time() {
        let mut vp = day_view();
        let pivot_time = vp.time_at(20);
        vp.zoom(Scale::Quarter, 20);
        assert_eq!(vp.col(pivot_time), 20);
        vp.zoom(Scale::Week, 20);
        assert_eq!(vp.col(pivot_time), 20);
    }

    #[test]
    fn test_scale_zoom_saturates() {
        assert_eq!(Scale::Quarter.zoom_in(), Scale::Quarter);
        assert_eq!(Scale::Month.zoom_out(), Scale::Month);
        assert_eq!(Scale::Day.zoom_out().zoom_in(), Scale::Day);
    }

    #[test]
    fn test_axis_ticks_on_mondays_at_day_scale() {
        let ticks = axis_ticks(&day_view(), 15);
        let cols: Vec<usize> = ticks.iter().map(|t| t.0).collect();
        assert_eq!(cols, vec![0, 7, 14]);
        assert_eq!(ticks[1].1, at(2026, 10, 5, 0));
    }

    #[test]
    fn test_axis_ticks_on_month_starts_at_week_scale() {
        let vp = Viewport::anchored(at(2026, 9, 28, 0), Scale::Week, 0);
        let ticks = axis_ticks(&vp, 10);
        assert_eq!(ticks[0].1, at(2026, 10, 1, 0));
        assert_eq!(ticks[0].0, 0); // Oct 1 falls in the week of Sep 28
        assert_eq!(ticks[1].1, at(2026, 11, 1, 0));
    }

    #[test]
    fn test_axis_labels_do_not_overlap_or_overflow() {
        // Quarter scale ticks every 4 columns but "Mon 28" needs 6.
        let vp = Viewport::anchored(at(2026, 9, 28, 0), Scale::Quarter, 0);
        let labels = axis_labels(&vp, 30);
        assert_eq!(labels[0], (0, "Mon 28".to_string()));
        for pair in labels.windows(2) {
            assert!(pair[0].0 + pair[0].1.width() < pair[1].0);
        }
        assert!(labels.iter().all(|(c, l)| c + l.width() <= 30));
    }

    #[test]
    fn test_tick_labels_mark_new_year_and_month() {
        assert_eq!(tick_label(at(2027, 1, 1, 0), Scale::Week), "2027");
        assert_eq!(tick_label(at(2026, 10, 1, 0), Scale::Week), "Oct");
        assert_eq!(tick_label(at(2026, 10, 1, 0), Scale::Quarter), "Oct 1");
    }

    #[test]
    fn test_row_draws_bar_with_markers_on_top() {
        let mut e = entry();
        e.start = Some(at(2026, 9, 29, 0));
        e.end = Some(at(2026, 10, 2, 0));
        e.due = Some(at(2026, 10, 1, 0));
        let cells = row_cells(&e, &day_view(), 7, at(2026, 9, 28, 12));
        assert_eq!(
            cells,
            vec![
                Cell::Today,
                Cell::Bar,
                Cell::Bar,
                Cell::Due,
                Cell::Bar,
                Cell::Empty,
                Cell::Empty
            ]
        );
    }

    #[test]
    fn test_row_clips_bar_to_view() {
        let mut e = entry();
        e.start = Some(at(2026, 9, 1, 0));
        e.end = Some(at(2026, 12, 1, 0));
        let cells = row_cells(&e, &day_view(), 4, at(2020, 1, 1, 0));
        assert_eq!(cells, vec![Cell::Bar; 4]);
    }

    #[test]
    fn test_row_marks_overdue_and_done() {
        let now = at(2026, 10, 3, 0);

        let mut late = entry();
        late.due = Some(at(2026, 9, 29, 0));
        assert_eq!(row_cells(&late, &day_view(), 7, now)[1], Cell::Overdue);

        let mut finished = late.clone();
        finished.done = true;
        finished.done_at = Some(at(2026, 9, 30, 9));
        let cells = row_cells(&finished, &day_view(), 7, now);
        assert_eq!(cells[1], Cell::Due);
        assert_eq!(cells[2], Cell::Done);
    }

    #[test]
    fn test_row_points_offscreen() {
        let now = at(2020, 1, 1, 0);
        let mut e = entry();

        e.due = Some(at(2026, 9, 1, 0));
        let cells = row_cells(&e, &day_view(), 5, now);
        assert_eq!((cells[0], cells[4]), (Cell::Before, Cell::Empty));

        e.due = Some(at(2027, 1, 1, 0));
        let cells = row_cells(&e, &day_view(), 5, now);
        assert_eq!((cells[0], cells[4]), (Cell::Empty, Cell::After));

        // Start before the view, due after it, no bar between them.
        e.start = Some(at(2026, 9, 1, 0));
        let cells = row_cells(&e, &day_view(), 5, now);
        assert_eq!((cells[0], cells[4]), (Cell::Before, Cell::After));
    }

    #[test]
    fn test_row_handles_reversed_span_and_zero_width() {
        let mut e = entry();
        e.start = Some(at(2026, 9, 30, 0));
        e.end = Some(at(2026, 9, 29, 0));
        let cells = row_cells(&e, &day_view(), 4, at(2020, 1, 1, 0));
        assert_eq!(cells[1..3], [Cell::Bar, Cell::Bar]);
        assert!(row_cells(&e, &day_view(), 0, at(2020, 1, 1, 0)).is_empty());
    }

    #[test]
    fn test_entry_requires_a_date_and_ignores_stale_done_at() {
        let mut task = ModelsTask {
            id: Some(3),
            title: Some("x".into()),
            ..Default::default()
        };
        assert!(Entry::from_task(&task, &[]).is_none());

        // Reopened task: done_at left over from before must not count.
        task.done_at = Some("2026-09-01T10:00:00Z".into());
        assert!(Entry::from_task(&task, &[]).is_none());

        // Vikunja's zero time means "unset".
        task.due_date = Some("0001-01-01T00:00:00Z".into());
        assert!(Entry::from_task(&task, &[]).is_none());

        task.due_date = Some("2026-10-01T10:00:00Z".into());
        let e = Entry::from_task(&task, &[]).unwrap();
        assert!(e.due.is_some() && e.done_at.is_none());
    }

    #[test]
    fn test_entry_takes_project_colour_as_fallback() {
        let project = ModelsProject {
            id: Some(4),
            title: Some("Work".into()),
            hex_color: Some("ff0000".into()),
            ..Default::default()
        };
        let mut task = ModelsTask {
            project_id: Some(4),
            due_date: Some("2026-10-01T10:00:00Z".into()),
            ..Default::default()
        };
        let e = Entry::from_task(&task, std::slice::from_ref(&project)).unwrap();
        assert_eq!(e.color, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(e.project, "Work");

        task.hex_color = Some("00ff00".into());
        let e = Entry::from_task(&task, &[project]).unwrap();
        assert_eq!(e.color, Some(Color::Rgb(0, 255, 0)));
    }

    #[test]
    fn test_parse_color_rejects_empty_and_malformed() {
        assert_eq!(parse_color(Some("")), None);
        assert_eq!(parse_color(Some("xyz")), None);
        assert_eq!(parse_color(None), None);
        assert_eq!(parse_color(Some("#0000ff")), Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_fit_truncates_by_display_width() {
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("ヒドラ", 4), "ヒ… ");
        assert_eq!(fit("abc", 0), "");
    }
}
