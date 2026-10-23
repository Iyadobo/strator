use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::io::{self, IsTerminal, Write};

pub fn choose(title: &str, subtitle: &str, items: &[String]) -> io::Result<Option<usize>> {
    if !io::stdin().is_terminal() {
        return Err(io::Error::other("Interactive picker needs a terminal. Use launch --provider ID --harness ID for scripting."));
    }
    let mut terminal = ratatui::init();
    let result = (|| {
        let mut selected = ListState::default().with_selected(Some(0));
        let mut query = String::new();
        loop {
            let needle = query.to_lowercase();
            let visible: Vec<_> = items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.to_lowercase().contains(&needle))
                .collect();
            terminal.draw(|frame| {
                let areas = Layout::vertical([
                    Constraint::Length(4),
                    Constraint::Min(3),
                    Constraint::Length(2),
                ])
                .margin(1)
                .split(frame.area());
                frame.render_widget(
                    Paragraph::new(format!("Strator\n{subtitle}\nFilter: {query}"))
                        .style(Style::default().fg(Color::Green)),
                    areas[0],
                );
                let list = List::new(visible.iter().map(|(_, item)| ListItem::new(item.as_str())))
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .highlight_symbol(" > ")
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    );
                frame.render_stateful_widget(list, areas[1], &mut selected);
                frame.render_widget(
                    Paragraph::new("Type to filter   ↑/↓ choose   Enter select   Esc cancel"),
                    areas[2],
                );
            })?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                let current = selected.selected().unwrap_or(0);
                match key.code {
                    KeyCode::Up => selected.select(Some(current.saturating_sub(1))),
                    KeyCode::Down => {
                        selected.select(Some((current + 1).min(visible.len().saturating_sub(1))))
                    }
                    KeyCode::Enter if !visible.is_empty() => return Ok(Some(visible[current].0)),
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        return Ok(None)
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        selected.select(Some(0));
                    }
                    KeyCode::Char(c) if !c.is_control() => {
                        query.push(c);
                        selected.select(Some(0));
                    }
                    _ => {}
                }
            }
        }
    })();
    ratatui::restore();
    result
}
pub fn text(label: &str, default: &str) -> io::Result<String> {
    print!(
        "{label}{}: ",
        if default.is_empty() {
            String::new()
        } else {
            format!(" [{default}]")
        }
    );
    io::stdout().flush()?;
    let mut value = String::new();
    if io::stdin().read_line(&mut value)? == 0 {
        return Err(io::Error::other("Input closed."));
    }
    Ok(if value.trim().is_empty() {
        default.into()
    } else {
        value.trim().into()
    })
}
pub fn secret(variable: &str) -> io::Result<String> {
    if !io::stdin().is_terminal() {
        return Err(io::Error::other(format!("Set {variable} before launch.")));
    }
    print!("Provider key ({variable}; saved in OS credential store): ");
    io::stdout().flush()?;
    crossterm::terminal::enable_raw_mode()?;
    let result = (|| {
        let mut value = String::new();
        loop {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Enter => return Ok(value),
                    KeyCode::Esc => return Err(io::Error::other("Key entry cancelled.")),
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        return Err(io::Error::other("Key entry cancelled."))
                    }
                    KeyCode::Backspace => {
                        if value.pop().is_some() {
                            print!("\u{8} \u{8}");
                        }
                    }
                    KeyCode::Char(c) if !c.is_control() && value.len() < 16384 => {
                        value.push(c);
                        print!("*");
                    }
                    _ => {}
                },
                Event::Paste(paste)
                    if value.len() + paste.len() <= 16384
                        && !paste.chars().any(char::is_control) =>
                {
                    for _ in paste.chars() {
                        print!("*");
                    }
                    value.push_str(&paste);
                }
                _ => {}
            }
            io::stdout().flush()?;
        }
    })();
    let restored = crossterm::terminal::disable_raw_mode();
    println!();
    restored?;
    result
}
