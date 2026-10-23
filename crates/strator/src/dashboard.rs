//! `strator` home: one screen made of several small windows, all visible
//! at once — Launch, Usage, Progress, Themes — like a union of terminal
//! windows and widgets. Mouse-first: click a window to focus it, click a
//! row to select, double-click (or Enter) to run. Wheel scrolls.
use crate::themes::{self, Prefs, BACKGROUNDS, THEMES};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind,
};
use crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Launch,
    OpenDefault,
    OpenByok,
    Attach,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Win {
    Launch,
    Themes,
    Usage,
    Progress,
}

const LAUNCH_ITEMS: [(&str, Option<Action>); 5] = [
    ("Open harness — CLI default, native picker", Some(Action::OpenDefault)),
    ("Open harness — BYOK model provider key", Some(Action::OpenByok)),
    ("Launch — provider-first catalog, advanced", Some(Action::Launch)),
    ("Attach to running session", Some(Action::Attach)),
    ("Quit", Some(Action::Quit)),
];

struct Areas {
    header: Rect,
    launch: Rect,
    themes: Rect,
    theme_list: Rect,
    bg_list: Rect,
    usage: Rect,
    progress: Rect,
    footer: Rect,
}

fn areas(area: Rect) -> Areas {
    let outer = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(2),
    ])
    .margin(1)
    .split(area);
    let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[1]);
    let left = Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[0]);
    let right = Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[1]);
    let theme_inner = Rect {
        x: right[0].x + 1,
        y: right[0].y + 1,
        width: right[0].width.saturating_sub(2),
        height: right[0].height.saturating_sub(2),
    };
    let theme_cols =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(theme_inner);
    Areas {
        header: outer[0],
        launch: left[0],
        usage: left[1],
        themes: right[0],
        theme_list: theme_cols[0],
        bg_list: theme_cols[1],
        progress: right[1],
        footer: outer[2],
    }
}

fn inner_row(area: Rect, column: u16, row: u16) -> Option<usize> {
    if column <= area.x || column >= area.right().saturating_sub(1) {
        return None;
    }
    if row <= area.y || row >= area.bottom().saturating_sub(1) {
        return None;
    }
    Some((row - area.y - 1) as usize)
}

struct State {
    focus: Win,
    launch_sel: usize,
    theme_sel: usize,
    bg_sel: usize,
    bg_column: bool,
    usage_scroll: u16,
    progress_scroll: u16,
    note: String,
    last_click: Option<(Win, usize, Instant)>,
}

impl State {
    fn window_block<'a>(&self, win: Win, title: &'a str, accent: Color) -> Block<'a> {
        let focused = self.focus == win;
        Block::default()
            .borders(Borders::ALL)
            .title(if focused {
                format!(" > {title} ")
            } else {
                format!(" {title} ")
            })
            .border_style(if focused {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            })
    }
    fn apply_theme(&mut self) {
        match themes::apply(THEMES[self.theme_sel], BACKGROUNDS[self.bg_sel].name) {
            Ok(path) => {
                self.note = format!(
                    "Applied {} / {} -> {}",
                    THEMES[self.theme_sel],
                    BACKGROUNDS[self.bg_sel].name,
                    path.display()
                )
            }
            Err(e) => self.note = format!("Theme failed: {e}"),
        }
    }
    /// Returns true when this click is a double-click on the same row.
    fn clicked(&mut self, win: Win, row: usize) -> bool {
        let now = Instant::now();
        let double = matches!(self.last_click, Some((w, r, at)) if w == win && r == row && now.duration_since(at) < Duration::from_millis(450));
        self.last_click = Some((win, row, now));
        double
    }
}

pub fn run(usage_text: String, progress_text: String) -> io::Result<Action> {
    if !io::stdin().is_terminal() {
        return Ok(Action::OpenDefault);
    }
    let prefs = Prefs::load();
    let mut st = State {
        focus: Win::Launch,
        launch_sel: 0,
        theme_sel: THEMES
            .iter()
            .position(|t| *t == prefs.theme)
            .unwrap_or(0),
        bg_sel: BACKGROUNDS
            .iter()
            .position(|b| b.name == prefs.background)
            .unwrap_or(0),
        bg_column: false,
        usage_scroll: 0,
        progress_scroll: 0,
        note: String::new(),
        last_click: None,
    };
    let mut usage_text = usage_text;
    let mut progress_text = progress_text;
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), event::EnableMouseCapture);
    let result = (|| {
        loop {
            let frame_area = terminal.size().map(|s| Rect::new(0, 0, s.width, s.height))?;
            let a = areas(frame_area);
            let bg = BACKGROUNDS[st.bg_sel];
            let bg_color = themes::parse_hex(bg.panel_bg);
            let accent = themes::parse_hex(bg.accent);
            terminal.draw(|frame| {
                frame.render_widget(Block::default().style(Style::default().bg(bg_color)), frame.area());
                let a = areas(frame.area());
                frame.render_widget(
                    Paragraph::new("STRATOR  —  all windows, one screen   ·   click a window, double-click to run")
                        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                    a.header,
                );
                // Launch window.
                let mut ls = ListState::default().with_selected(Some(st.launch_sel));
                let launch = List::new(LAUNCH_ITEMS.iter().map(|(label, _)| ListItem::new(*label)))
                    .block(st.window_block(Win::Launch, "Launch", accent))
                    .highlight_symbol(" > ")
                    .highlight_style(Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD));
                frame.render_stateful_widget(launch, a.launch, &mut ls);
                // Usage window (scrollable).
                frame.render_widget(
                    Paragraph::new(usage_text.clone())
                        .block(st.window_block(Win::Usage, "Usage", accent))
                        .wrap(Wrap { trim: false })
                        .scroll((st.usage_scroll, 0)),
                    a.usage,
                );
                // Progress window (scrollable).
                frame.render_widget(
                    Paragraph::new(progress_text.clone())
                        .block(st.window_block(Win::Progress, "Progress", accent))
                        .wrap(Wrap { trim: false })
                        .scroll((st.progress_scroll, 0)),
                    a.progress,
                );
                // Themes window: two small lists side by side.
                frame.render_widget(st.window_block(Win::Themes, "Themes + backgrounds", accent), a.themes);
                let mut ts = ListState::default().with_selected(Some(st.theme_sel));
                let themes_list = List::new(THEMES.iter().map(|t| ListItem::new(*t)))
                    .block(Block::default().borders(Borders::ALL).title(if st.bg_column { " Theme " } else { " Theme > " }))
                    .highlight_symbol(" > ")
                    .highlight_style(Style::default().fg(Color::Black).bg(accent));
                frame.render_stateful_widget(themes_list, a.theme_list, &mut ts);
                let mut bs = ListState::default().with_selected(Some(st.bg_sel));
                let bg_list = List::new(BACKGROUNDS.iter().map(|b| ListItem::new(b.name)))
                    .block(Block::default().borders(Borders::ALL).title(if st.bg_column { " Background > " } else { " Background " }))
                    .highlight_symbol(" > ")
                    .highlight_style(Style::default().fg(Color::Black).bg(accent));
                frame.render_stateful_widget(bg_list, a.bg_list, &mut bs);
                let hint = if st.note.is_empty() {
                    "Mouse: click focus/select · double-click run/apply · wheel scroll    Keys: Tab window · ↑↓ move · Enter run · r refresh · q quit"
                        .to_string()
                } else {
                    st.note.clone()
                };
                frame.render_widget(Paragraph::new(hint), a.footer);
            })?;
            match event::read()? {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let (col, row) = (mouse.column, mouse.row);
                        if let Some(r) = inner_row(a.launch, col, row) {
                            if r < LAUNCH_ITEMS.len() {
                                st.focus = Win::Launch;
                                if st.launch_sel == r && st.clicked(Win::Launch, r) {
                                    if let Some(action) = LAUNCH_ITEMS[r].1 {
                                        return Ok(action);
                                    }
                                }
                                st.launch_sel = r;
                            }
                        } else if let Some(r) = inner_row(a.theme_list, col, row) {
                            if r < THEMES.len() {
                                st.focus = Win::Themes;
                                st.bg_column = false;
                                st.theme_sel = r;
                                st.clicked(Win::Themes, r);
                                st.apply_theme();
                            }
                        } else if let Some(r) = inner_row(a.bg_list, col, row) {
                            if r < BACKGROUNDS.len() {
                                st.focus = Win::Themes;
                                st.bg_column = true;
                                st.bg_sel = r;
                                st.clicked(Win::Themes, 100 + r);
                                st.apply_theme();
                            }
                        } else if inner_row(a.usage, col, row).is_some() {
                            st.focus = Win::Usage;
                        } else if inner_row(a.progress, col, row).is_some() {
                            st.focus = Win::Progress;
                        } else if inner_row(a.themes, col, row).is_some() {
                            st.focus = Win::Themes;
                        }
                    }
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                        let down = mouse.kind == MouseEventKind::ScrollDown;
                        let (col, row) = (mouse.column, mouse.row);
                        if inner_row(a.usage, col, row).is_some() || (st.focus == Win::Usage && inner_row(a.usage, col, row).is_none() && col >= a.usage.x && col < a.usage.right()) {
                            st.focus = Win::Usage;
                            st.usage_scroll = if down { st.usage_scroll.saturating_add(3) } else { st.usage_scroll.saturating_sub(3) };
                        } else if col >= a.progress.x && col < a.progress.right() && row >= a.progress.y && row < a.progress.bottom() {
                            st.focus = Win::Progress;
                            st.progress_scroll = if down { st.progress_scroll.saturating_add(3) } else { st.progress_scroll.saturating_sub(3) };
                        } else if col >= a.theme_list.x && col < a.theme_list.right() && row >= a.theme_list.y && row < a.theme_list.bottom() {
                            st.focus = Win::Themes;
                            st.bg_column = false;
                            st.theme_sel = if down { (st.theme_sel + 1).min(THEMES.len() - 1) } else { st.theme_sel.saturating_sub(1) };
                        } else if col >= a.bg_list.x && col < a.bg_list.right() && row >= a.bg_list.y && row < a.bg_list.bottom() {
                            st.focus = Win::Themes;
                            st.bg_column = true;
                            st.bg_sel = if down { (st.bg_sel + 1).min(BACKGROUNDS.len() - 1) } else { st.bg_sel.saturating_sub(1) };
                        } else if col >= a.launch.x && col < a.launch.right() && row >= a.launch.y && row < a.launch.bottom() {
                            st.focus = Win::Launch;
                            st.launch_sel = if down { (st.launch_sel + 1).min(LAUNCH_ITEMS.len() - 1) } else { st.launch_sel.saturating_sub(1) };
                        }
                    }
                    _ => {}
                },
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('q') => return Ok(Action::Quit),
                        KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => return Ok(Action::Quit),
                        KeyCode::Tab => {
                            st.focus = match st.focus {
                                Win::Launch => Win::Themes,
                                Win::Themes => Win::Progress,
                                Win::Progress => Win::Usage,
                                Win::Usage => Win::Launch,
                            }
                        }
                        KeyCode::BackTab => {
                            st.focus = match st.focus {
                                Win::Launch => Win::Usage,
                                Win::Usage => Win::Progress,
                                Win::Progress => Win::Themes,
                                Win::Themes => Win::Launch,
                            }
                        }
                        KeyCode::Char('r') => {
                            if let Some((u, p)) = crate::dashboard_data() {
                                usage_text = u;
                                progress_text = p;
                                st.note = "Refreshed.".into();
                            }
                        }
                        KeyCode::Char('b') if st.focus == Win::Launch => return Ok(Action::OpenByok),
                        KeyCode::Char('l') if st.focus == Win::Launch => return Ok(Action::Launch),
                        KeyCode::Char('a') if st.focus == Win::Launch => return Ok(Action::Attach),
                        KeyCode::Left if st.focus == Win::Themes => st.bg_column = false,
                        KeyCode::Right if st.focus == Win::Themes => st.bg_column = true,
                        KeyCode::Up => match st.focus {
                            Win::Launch => st.launch_sel = st.launch_sel.saturating_sub(1),
                            Win::Themes => {
                                if st.bg_column { st.bg_sel = st.bg_sel.saturating_sub(1) } else { st.theme_sel = st.theme_sel.saturating_sub(1) }
                            }
                            Win::Usage => st.usage_scroll = st.usage_scroll.saturating_sub(1),
                            Win::Progress => st.progress_scroll = st.progress_scroll.saturating_sub(1),
                        },
                        KeyCode::Down => match st.focus {
                            Win::Launch => st.launch_sel = (st.launch_sel + 1).min(LAUNCH_ITEMS.len() - 1),
                            Win::Themes => {
                                if st.bg_column { st.bg_sel = (st.bg_sel + 1).min(BACKGROUNDS.len() - 1) } else { st.theme_sel = (st.theme_sel + 1).min(THEMES.len() - 1) }
                            }
                            Win::Usage => st.usage_scroll = st.usage_scroll.saturating_add(1),
                            Win::Progress => st.progress_scroll = st.progress_scroll.saturating_add(1),
                        },
                        KeyCode::Enter => match st.focus {
                            Win::Launch => {
                                if let Some(action) = LAUNCH_ITEMS[st.launch_sel].1 {
                                    return Ok(action);
                                }
                            }
                            Win::Themes => st.apply_theme(),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    })();
    let _ = execute!(io::stdout(), event::DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_tile_without_overlap() {
        let a = areas(Rect::new(0, 0, 160, 24));
        // Two columns, two rows: every window beside another, none stacked.
        assert!(a.launch.right() <= a.themes.x);
        assert!(a.launch.bottom() <= a.usage.y);
        assert!(a.themes.bottom() <= a.progress.y);
        // Theme lists sit inside the outer Themes window.
        assert!(a.theme_list.x > a.themes.x && a.bg_list.right() < a.themes.right());
        assert!(a.theme_list.y > a.themes.y);
    }

    #[test]
    fn mouse_hit_rows_map_to_items() {
        let a = areas(Rect::new(0, 0, 160, 24));
        // First row inside Launch is item 0; borders hit nothing.
        assert_eq!(inner_row(a.launch, a.launch.x + 3, a.launch.y + 1), Some(0));
        assert_eq!(inner_row(a.launch, a.launch.x + 3, a.launch.y + 2), Some(1));
        assert_eq!(inner_row(a.launch, a.launch.x, a.launch.y + 1), None);
        // Background list: row 0 is the first background, next row the second.
        assert_eq!(inner_row(a.bg_list, a.bg_list.x + 3, a.bg_list.y + 1), Some(0));
        assert_eq!(inner_row(a.bg_list, a.bg_list.x + 3, a.bg_list.y + 2), Some(1));
        assert_eq!(inner_row(a.theme_list, a.theme_list.x + 3, a.theme_list.y + 1), Some(0));
        // Usage and Progress are separate windows left/right, same rows.
        assert!(a.usage.y == a.progress.y && a.usage.right() <= a.progress.x);
    }

    #[test]
    fn launch_items_cover_default_byok_and_quit() {
        assert_eq!(LAUNCH_ITEMS[0].1, Some(Action::OpenDefault));
        assert_eq!(LAUNCH_ITEMS[1].1, Some(Action::OpenByok));
        assert_eq!(LAUNCH_ITEMS[4].1, Some(Action::Quit));
    }
}

