use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    style::{Color, Modifier, Style},
    {TerminalOptions, Viewport},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Terminal,
};

/// Open an inline TUI for toggling subtask items. Mutates `items` in place.
/// Returns `true` if any item was toggled (and enter was pressed to save).
pub fn run_check_tui(items: &mut Vec<(bool, String)>) -> bool {
    let mut selected = 0usize;
    let mut changed = false;

    enable_raw_mode().unwrap();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(items.len() as u16),
        },
    )
    .unwrap();

    loop {
        terminal
            .draw(|f| {
                let area = f.area();

                let list_items: Vec<ListItem> = items
                    .iter()
                    .map(|(checked, text)| {
                        let (icon, icon_style, text_style) = if *checked {
                            (
                                "●",
                                Style::default().fg(Color::Green),
                                Style::default().fg(Color::DarkGray),
                            )
                        } else {
                            (
                                "○",
                                Style::default().fg(Color::DarkGray),
                                Style::default(),
                            )
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("  {icon} "), icon_style),
                            Span::styled(text.clone(), text_style),
                        ]))
                    })
                    .collect();

                let mut state = ListState::default();
                state.select(Some(selected));

                let list = List::new(list_items)
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD));
                f.render_stateful_widget(list, area, &mut state);
            })
            .unwrap();

        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Enter => break,
                    KeyCode::Esc | KeyCode::Char('q') => {
                        changed = false;
                        break;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected + 1 < items.len() {
                            selected += 1;
                        }
                    }
                    KeyCode::Char(' ') => {
                        if let Some(item) = items.get_mut(selected) {
                            item.0 = !item.0;
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode().unwrap();
    println!();

    changed
}
