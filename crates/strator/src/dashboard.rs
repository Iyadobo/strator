//! `strator` home: tabbed dashboard. Launch / Usage / Progress / Themes / Help.
use crate::themes::{self, Prefs, BACKGROUNDS, THEMES};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};
use std::io::{self, IsTerminal};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Launch,
    OpenDefault,
    OpenByok,
    Attach,
    Quit,
}

const TAB_NAMES: [&str; 5] = [" Launch ", " Usage ", " Progress ", " Themes ", " Help "];

pub fn run(usage_text: String, progress_text: String) -> io::Result<Action> {
    if !io::stdin().is_terminal() {
        return Ok(Action::OpenDefault);
    }
    let mut prefs = Prefs::load();
    if prefs.theme.is_empty() {
        prefs.theme = THEMES[0].into();
    }
    if prefs.background.is_empty() {
        prefs.background = BACKGROUNDS[0].name.into();
    }
    let mut tab = 0usize;
    let mut theme_sel = THEMES.iter().position(|t| *t == prefs.theme).unwrap_or(0);
    let mut bg_sel = BACKGROUNDS.iter().position(|b| b.name == prefs.background).unwrap_or(0);
    let mut focus_bg = false;
    let mut note = String::new();
    let mut usage_text = usage_text;
    let mut progress_text = progress_text;
    let mut terminal = ratatui::init();
    let result = (|| {
        loop {
            let (bg_color, accent) = {
                let bg = BACKGROUNDS[bg_sel];
                (themes::parse_hex(bg.panel_bg), themes::parse_hex(bg.accent))
            };
            terminal.draw(|frame| {
                let area = frame.area();
                frame.render_widget(Block::default().style(Style::default().bg(bg_color)), area);
                let rows = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(2),
                ])
                .margin(1)
                .split(area);
                let title = Paragraph::new("STRATOR  —  agents, at a glance")
                    .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
                frame.render_widget(title, rows[0]);
                let tabs = Tabs::new(TAB_NAMES.iter().copied())
                    .select(tab)
                    .highlight_style(Style::default().fg(Color::Black).bg(accent).add_modifier(Modifier::BOLD))
                    .divider(" | ");
                // Tabs sit inside the header row area below title text.
                let header = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(rows[0]);
                let _ = header;
                frame.render_widget(tabs, Layout::vertical([Constraint::Length(2), Constraint::Length(1)]).split(rows[0])[1]);
                let body = Block::default().borders(Borders::ALL).title(TAB_NAMES[tab]);
                match tab {
                    0 => {
                        let items = [
                            "Open harness — CLI default model, native picker  (Enter)",
                            "Open harness — BYOK, your model provider key  (b)",
                            "Launch — provider-first catalog picker, advanced  (l)",
                            "Attach to running session  (a)",
                            "Refresh usage + progress  (r)",
                            "Quit  (q)",
                        ];
                        frame.render_widget(
                            List::new(items.iter().map(|i| ListItem::new(*i))).block(body),
                            rows[1],
                        );
                    }
                    1 => frame.render_widget(Paragraph::new(usage_text.clone()).block(body), rows[1]),
                    2 => frame.render_widget(Paragraph::new(progress_text.clone()).block(body), rows[1]),
                    3 => {
                        let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(rows[1]);
                        let mut ts = ListState::default().with_selected(Some(theme_sel));
                        let themes_list = List::new(THEMES.iter().map(|t| ListItem::new(*t)))
                            .block(Block::default().borders(Borders::ALL).title(if focus_bg { " Theme " } else { " Theme > " }))
                            .highlight_symbol(" > ")
                            .highlight_style(Style::default().fg(Color::Black).bg(accent));
                        frame.render_stateful_widget(themes_list, cols[0], &mut ts);
                        let mut bs = ListState::default().with_selected(Some(bg_sel));
                        let bg_list = List::new(BACKGROUNDS.iter().map(|b| ListItem::new(b.name)))
                            .block(Block::default().borders(Borders::ALL).title(if focus_bg { " Background > " } else { " Background " }))
                            .highlight_symbol(" > ")
                            .highlight_style(Style::default().fg(Color::Black).bg(accent));
                        frame.render_stateful_widget(bg_list, cols[1], &mut bs);
                    }
                    _ => frame.render_widget(
                        Paragraph::new(
                            "Keys\n  Tab / 1-5   switch tab\n  Up/Down     move (Themes)\n  Left/Right  theme <-> background (Themes)\n  Enter       open harness, CLI default (Launch) / apply theme (Themes)\n  b           BYOK launch (secondary)    l  provider-first launch\n  a           attach    r  refresh    q  quit\n\nCLI\n  strator            this dashboard\n  strator open       DEFAULT: installed harness, CLI default + native picker\n  strator byok       SECONDARY: model provider key, saved once, shared by harnesses\n  strator launch     advanced provider-first catalog launch\n  strator usage      launch history + counts\n  strator progress   live agent states + done bar\n  strator themes     list / set theme + background\n  strator doctor     engine, config and PATH check\n  strator attach     open the full multiplexer\n  strator list       panes + agent states (plain text)\n",
                        )
                        .block(body),
                        rows[1],
                    ),
                }
                let hint = if note.is_empty() {
                    "Tab switch · Enter select · q quit".to_string()
                } else {
                    note.clone()
                };
                frame.render_widget(Paragraph::new(hint), rows[2]);
            })?;
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => return Ok(Action::Quit),
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        return Ok(Action::Quit)
                    }
                    KeyCode::Tab | KeyCode::Right if tab != 3 => tab = (tab + 1) % TAB_NAMES.len(),
                    KeyCode::BackTab => tab = (tab + TAB_NAMES.len() - 1) % TAB_NAMES.len(),
                    KeyCode::Char(c @ '1'..='5') => tab = (c as usize) - ('1' as usize),
                    KeyCode::Char('a') if tab == 0 => return Ok(Action::Attach),
                    KeyCode::Char('d') if tab == 0 => return Ok(Action::OpenDefault),
                    KeyCode::Char('l') if tab == 0 => return Ok(Action::Launch),
                    KeyCode::Char('b') if tab == 0 => return Ok(Action::OpenByok),
                    KeyCode::Char('r') => {
                        if let Some((u, p)) = crate::dashboard_data() {
                            usage_text = u;
                            progress_text = p;
                            note = "Refreshed.".into();
                        }
                    }
                    KeyCode::Left if tab == 3 => focus_bg = false,
                    KeyCode::Right if tab == 3 => focus_bg = true,
                    KeyCode::Up if tab == 3 => {
                        if focus_bg {
                            bg_sel = bg_sel.saturating_sub(1);
                        } else {
                            theme_sel = theme_sel.saturating_sub(1);
                        }
                    }
                    KeyCode::Down if tab == 3 => {
                        if focus_bg {
                            bg_sel = (bg_sel + 1).min(BACKGROUNDS.len() - 1);
                        } else {
                            theme_sel = (theme_sel + 1).min(THEMES.len() - 1);
                        }
                    }
                    KeyCode::Enter if tab == 0 => return Ok(Action::OpenDefault),
                    KeyCode::Enter if tab == 3 => {
                        match themes::apply(THEMES[theme_sel], BACKGROUNDS[bg_sel].name) {
                            Ok(path) => note = format!("Applied {} / {} -> {}", THEMES[theme_sel], BACKGROUNDS[bg_sel].name, path.display()),
                            Err(e) => note = format!("Theme failed: {e}"),
                        }
                    }
                    _ => {}
                }
            }
        }
    })();
    ratatui::restore();
    result
}
