//! Strator's own multi-window terminal dashboard. No engine, no tabs:
//! 3–4 real terminal panes (PTYs, Strator's own) tiled on one screen,
//! with a Usage card and a Progress card below them. Mouse-first:
//! click a pane to focus and type into it, click header buttons to
//! add panes, wheel scrolls a pane's scrollback.
use crate::term::{Pane, PaneSpawn};
use crate::themes::{self, BACKGROUNDS, THEMES};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io;
use std::time::Duration;


const BUTTONS: [&str; 5] = ["[ Shell ]", "[ Agent ]", "[ BYOK ]", "[ Theme ]", "[ Quit ]"];

/// The dashboard always shows exactly this many terminal panes.
/// Adding an agent replaces the focused pane — nothing ever opens
/// outside the dashboard.
pub const PANE_COUNT: usize = 3;

struct Areas {
    header: Rect,
    buttons: Rect,
    panes: Vec<Rect>,
    usage: Rect,
    progress: Rect,
}

fn pane_rects(area: Rect, count: usize) -> Vec<Rect> {
    match count {
        0 => vec![],
        1 => vec![area],
        2 => Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .split(area)
            .to_vec(),
        _ => {
            let rows = Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                .spacing(1)
                .split(area);
            let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .spacing(1)
                .split(rows[0]);
            if count == 3 {
                vec![top[0], top[1], rows[1]]
            } else {
                let bottom = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .spacing(1)
                    .split(rows[1]);
                vec![top[0], top[1], bottom[0], bottom[1]]
            }
        }
    }
}

fn areas(area: Rect, pane_count: usize) -> Areas {
    let outer = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(6),
    ])
    .margin(1)
    .split(area);
    let cards = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(outer[3]);
    Areas {
        header: outer[0],
        buttons: outer[1],
        panes: pane_rects(outer[2], pane_count),
        usage: cards[0],
        progress: cards[1],
    }
}

fn button_rects(area: Rect) -> Vec<Rect> {
    let mut out = vec![];
    let mut x = area.x;
    for label in BUTTONS {
        let w = label.len() as u16;
        out.push(Rect::new(x, area.y, w, 1));
        x += w + 1;
    }
    out
}

fn key_bytes(key: event::KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let b = (c.to_ascii_lowercase() as u32).wrapping_sub('a' as u32) + 1;
                if (1..=26).contains(&b) {
                    return Some(vec![b as u8]);
                }
                return None;
            }
            let mut buf = [0u8; 4];
            Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
        }
        KeyCode::Enter => Some(b"\r".to_vec()),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(b"\t".to_vec()),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::F(n) => Some(format!("\x1b[{n}~").into_bytes()),
        _ => None,
    }
}

/// Exactly PANE_COUNT spawns: any requested agent first, the rest
/// plain shells. Nothing is ever opened outside the dashboard.
pub fn initial_spawns(initial: Vec<PaneSpawn>, cwd: &str) -> Vec<PaneSpawn> {
    let mut pending = initial;
    while pending.len() < PANE_COUNT {
        pending.push(PaneSpawn::shell(cwd));
    }
    pending.truncate(PANE_COUNT);
    pending
}

pub fn run(initial: Vec<PaneSpawn>) -> io::Result<()> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".into());
    let mut panes: Vec<Pane> = vec![];
    let mut pending = initial_spawns(initial, &cwd);
    let mut focus = 0usize;
    let mut usage_text = String::new();
    let refresh_usage = |usage_text: &mut String| {
        *usage_text = crate::usage::render(&crate::usage::load(), crate::usage::now_secs());
    };
    refresh_usage(&mut usage_text);
    let mut last_size = (0u16, 0u16);
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), event::EnableMouseCapture);
    let result = (|| -> io::Result<()> {
        // Spawn initial panes at a sane default; resized on first frame.
        for spawn in pending.drain(..) {
            panes.push(Pane::spawn(&spawn, 80, 24)?);
        }
        loop {
            let size = terminal.size()?;
            let frame_area = Rect::new(0, 0, size.width, size.height);
            let a = areas(frame_area, panes.len().max(1));
            // Resize panes whose window changed.
            if (size.width, size.height) != last_size {
                last_size = (size.width, size.height);
                for (pane, rect) in panes.iter_mut().zip(a.panes.iter()) {
                    pane.resize(rect.width.saturating_sub(2).max(2), rect.height.saturating_sub(2).max(2));
                }
            }
            let prefs = themes::Prefs::load();
            let bg = BACKGROUNDS
                .iter()
                .find(|b| b.name == prefs.background)
                .copied()
                .unwrap_or(BACKGROUNDS[0]);
            let bg_color = themes::parse_hex(bg.panel_bg);
            let accent = themes::parse_hex(bg.accent);
            let statuses: Vec<&str> = panes.iter_mut().map(|p| p.status()).collect();
            let running = statuses.iter().filter(|s| **s == "running").count();
            terminal.draw(|frame| {
                let a = areas(frame.area(), panes.len().max(1));
                frame.render_widget(Block::default().style(Style::default().bg(bg_color)), frame.area());
                frame.render_widget(
                    Paragraph::new(format!("STRATOR  ·  {running}/{} running", panes.len()))
                    .style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                    a.header,
                );
                // Buttons row.
                let btns = button_rects(a.buttons);
                for (i, (label, rect)) in BUTTONS.iter().zip(btns.iter()).enumerate() {
                    let _ = i;
                    frame.render_widget(
                        Paragraph::new(*label)
                            .style(Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD)),
                        *rect,
                    );
                }
                // Terminal panes.
                for (i, (pane, rect)) in panes.iter_mut().zip(a.panes.iter()).enumerate() {
                    let focused = i == focus;
                    let status = statuses[i];
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(format!(
                            " {} {} ",
                            pane.title,
                            if status == "running" { "●" } else { "○" }
                        ))
                        .border_style(if focused {
                            Style::default().fg(accent).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        });
                    let inner = block.inner(*rect);
                    frame.render_widget(block, *rect);
                    if let Ok(screen) = pane.screen.lock() {
                        let lines: Vec<Line> = screen
                            .display_line(pane.scroll, inner.height as usize)
                            .iter()
                            .map(|row| {
                                let mut spans = vec![];
                                let mut cur: Vec<char> = vec![];
                                let mut cur_style = Style::default();
                                for cell in row.iter() {
                                    if cell.style != cur_style && !cur.is_empty() {
                                        spans.push(Span::styled(cur.iter().collect::<String>(), cur_style));
                                        cur.clear();
                                    }
                                    cur_style = cell.style;
                                    cur.push(cell.ch);
                                }
                                if !cur.is_empty() {
                                    spans.push(Span::styled(cur.iter().collect::<String>(), cur_style));
                                }
                                Line::from(spans)
                            })
                            .collect();
                        frame.render_widget(Paragraph::new(lines), inner);
                        if focused && pane.scroll == 0 {
                            let (cx, cy) = screen.cursor();
                            let x = inner.x + (cx as u16).min(inner.width.saturating_sub(1));
                            let y = inner.y + (cy as u16).min(inner.height.saturating_sub(1));
                            frame.set_cursor_position((x, y));
                        }
                    }
                }
                // Usage card.
                frame.render_widget(
                    Paragraph::new(usage_text.clone())
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Usage ")
                                .border_style(Style::default().fg(accent)),
                        )
                        .wrap(Wrap { trim: false }),
                    a.usage,
                );
                // Progress card.
                let total = panes.len();
                let exited = total - running;
                let bar = crate::progress::bar(running, total.max(1), 20);
                let mut progress = format!(
                    "{running}/{total} running · {exited} exited  [{bar}]"
                );
                for (i, pane) in panes.iter().enumerate() {
                    progress.push_str(&format!(
                        "\n{} {} · {}",
                        if i == focus { ">" } else { " " },
                        pane.title, statuses[i]
                    ));
                }
                frame.render_widget(
                    Paragraph::new(progress)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Progress ")
                                .border_style(Style::default().fg(accent)),
                        )
                        .wrap(Wrap { trim: false }),
                    a.progress,
                );
            })?;
            if !event::poll(Duration::from_millis(50))? {
                continue;
            }
            match event::read()? {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let (col, row) = (mouse.column, mouse.row);
                        let btns = button_rects(a.buttons);
                        let mut handled = false;
                        for (i, rect) in btns.iter().enumerate() {
                            if col >= rect.x && col < rect.right() && row == rect.y {
                                handled = true;
                                match i {
                                    0 => {
                                        // Fresh shell in the focused pane.
                                        if !panes.is_empty() {
                                            let slot = focus.min(panes.len() - 1);
                                            panes[slot].kill();
                                            panes[slot] = Pane::spawn(&PaneSpawn::shell(&cwd), 80, 24)?;
                                            last_size = (0, 0);
                                        }
                                    }
                                    1 | 2 => {
                                        // Suspend dashboard, run pickers, spawn pane.
                                        let _ = execute!(io::stdout(), event::DisableMouseCapture);
                                        ratatui::restore();
                                        let byok = i == 2;
                                        let made = crate::pane_spawn_for(
                                            &crate::Options::default_session(),
                                            if byok { crate::OpenMode::Byok } else { crate::OpenMode::CliDefault },
                                        );
                                        terminal = ratatui::init();
                                        let _ = execute!(io::stdout(), event::EnableMouseCapture);
                                        last_size = (0, 0);
                                        if let Ok(Some(spawn)) = made {
                                            // Agent takes over the focused pane.
                                            if panes.is_empty() {
                                                panes.push(Pane::spawn(&spawn, 80, 24)?);
                                                focus = 0;
                                            } else {
                                                let slot = focus.min(panes.len() - 1);
                                                panes[slot].kill();
                                                panes[slot] = Pane::spawn(&spawn, 80, 24)?;
                                            }
                                            refresh_usage(&mut usage_text);
                                        }
                                    }
                                    3 => {
                                        // Cycle background, keep theme.
                                        let current = themes::Prefs::load();
                                        let idx = BACKGROUNDS
                                            .iter()
                                            .position(|b| b.name == current.background)
                                            .unwrap_or(0);
                                        let next = BACKGROUNDS[(idx + 1) % BACKGROUNDS.len()];
                                        let theme = if THEMES.contains(&current.theme.as_str()) {
                                            current.theme.clone()
                                        } else {
                                            THEMES[0].into()
                                        };
                                        let _ = themes::apply(&theme, next.name);
                                    }
                                    _ => return Ok(()),
                                }
                            }
                        }
                        if handled {
                            continue;
                        }
                        for (i, rect) in a.panes.iter().enumerate() {
                            if col >= rect.x && col < rect.right() && row >= rect.y && row < rect.bottom() {
                                focus = i;
                                panes[i].scroll = 0;
                            }
                        }
                    }
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                        let down = mouse.kind == MouseEventKind::ScrollDown;
                        for (i, rect) in a.panes.iter().enumerate() {
                            if mouse.column >= rect.x && mouse.column < rect.right() && mouse.row >= rect.y && mouse.row < rect.bottom() {
                                focus = i;
                                if down {
                                    panes[i].scroll = panes[i].scroll.saturating_sub(3);
                                } else {
                                    let max = panes[i].screen.lock().map(|s| s.scrollback.len()).unwrap_or(0);
                                    panes[i].scroll = (panes[i].scroll + 3).min(max);
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                            KeyCode::Char('w') | KeyCode::Char('W') => {
                                if !panes.is_empty() {
                                    let slot = focus.min(panes.len() - 1);
                                    panes[slot].kill();
                                    panes[slot] = Pane::spawn(&PaneSpawn::shell(&cwd), 80, 24)?;
                                    last_size = (0, 0);
                                }
                                continue;
                            }
                            _ => {}
                        }
                    }
                    if let Some(pane) = panes.get(focus) {
                        if let Some(bytes) = key_bytes(key) {
                            pane.write(&bytes);
                        }
                    }
                }
                Event::Paste(text) => {
                    if let Some(pane) = panes.get(focus) {
                        pane.write(text.as_bytes());
                    }
                }
                Event::Resize(_, _) => last_size = (0, 0),
                _ => {}
            }
        }
    })();
    for pane in panes.iter_mut() {
        pane.kill();
    }
    let _ = execute!(io::stdout(), event::DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_tile_one_to_four() {
        let area = Rect::new(0, 0, 160, 30);
        assert_eq!(pane_rects(area, 1).len(), 1);
        assert_eq!(pane_rects(area, 2).len(), 2);
        assert_eq!(pane_rects(area, 3).len(), 3);
        let four = pane_rects(area, 4);
        assert_eq!(four.len(), 4);
        assert!(four[0].right() <= four[1].x && four[0].bottom() <= four[2].y);
        assert!(four[3].right() <= area.right() && four[3].bottom() <= area.bottom());
    }

    #[test]
    fn cards_sit_below_panes() {
        let a = areas(Rect::new(0, 0, 160, 40), 2);
        assert!(a.usage.y >= a.panes[0].bottom());
        assert_eq!(a.usage.y, a.progress.y);
        assert!(a.usage.right() <= a.progress.x);
        let btns = button_rects(a.buttons);
        assert_eq!(btns.len(), 5);
        assert!(btns[0].x < btns[1].x && btns[1].x < btns[4].x);
    }

    #[test]
    fn always_three_panes() {
        let three = initial_spawns(vec![], "/tmp");
        assert_eq!(three.len(), PANE_COUNT);
        assert!(three.iter().all(|s| s.title == "Shell"));
        let agent = PaneSpawn { title: "Codex / openai".into(), ..PaneSpawn::shell("/tmp") };
        let mixed = initial_spawns(vec![agent], "/tmp");
        assert_eq!(mixed.len(), 3);
        assert_eq!(mixed[0].title, "Codex / openai");
    }
    #[test]
    fn key_encoding() {
        use crossterm::event::{KeyEvent, KeyEventState};
        let enter = KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE };
        assert_eq!(key_bytes(enter), Some(b"\r".to_vec()));
        let ctrl_c = KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, kind: KeyEventKind::Press, state: KeyEventState::NONE };
        assert_eq!(key_bytes(ctrl_c), Some(vec![3]));
    }
}
