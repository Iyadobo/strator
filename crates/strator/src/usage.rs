//! Launch usage log: one JSON line per launch in the Strator config dir.
//! Powers the Usage tab and `strator usage`. No keys or endpoints are stored.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub ts: u64,
    pub provider: String,
    pub harness: String,
    pub model: String,
    pub cwd: String,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn log_path() -> io::Result<PathBuf> {
    Ok(crate::platform::config_dir()?.join("usage.jsonl"))
}

pub fn record(provider: &str, harness: &str, model: &str, cwd: &str) -> io::Result<()> {
    let path = log_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let event = Event {
        ts: now_secs(),
        provider: provider.into(),
        harness: harness.into(),
        model: if model.is_empty() { "default".into() } else { model.into() },
        cwd: cwd.into(),
    };
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(&event).unwrap_or_default())
}

pub fn load() -> Vec<Event> {
    let Ok(path) = log_path() else { return Vec::new() };
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub last_7_days: usize,
    pub by_provider: Vec<(String, usize)>,
    pub by_harness: Vec<(String, usize)>,
    pub by_model: Vec<(String, usize)>,
    pub projects: usize,
}

fn ranked(map: BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut rows: Vec<_> = map.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

pub fn summarize(events: &[Event], now: u64) -> Summary {
    let mut provider = BTreeMap::new();
    let mut harness = BTreeMap::new();
    let mut model = BTreeMap::new();
    let mut projects = std::collections::BTreeSet::new();
    let week = 7 * 24 * 3600;
    let mut last_7_days = 0;
    for e in events {
        *provider.entry(e.provider.clone()).or_insert(0) += 1;
        *harness.entry(e.harness.clone()).or_insert(0) += 1;
        *model.entry(e.model.clone()).or_insert(0) += 1;
        projects.insert(e.cwd.clone());
        if now.saturating_sub(e.ts) <= week {
            last_7_days += 1;
        }
    }
    Summary {
        total: events.len(),
        last_7_days,
        by_provider: ranked(provider),
        by_harness: ranked(harness),
        by_model: ranked(model),
        projects: projects.len(),
    }
}

pub fn render(events: &[Event], now: u64) -> String {
    let s = summarize(events, now);
    let mut out = format!(
        "Launches: {} total · {} in last 7 days · {} projects\n",
        s.total, s.last_7_days, s.projects
    );
    let section = |out: &mut String, title: &str, rows: &[(String, usize)]| {
        out.push_str(&format!("\n{title}\n"));
        if rows.is_empty() {
            out.push_str("  (none yet — launch an agent first)\n");
        }
        for (name, count) in rows.iter().take(8) {
            out.push_str(&format!("  {name:<28} {count}\n"));
        }
    };
    section(&mut out, "By provider", &s.by_provider);
    section(&mut out, "By harness", &s.by_harness);
    section(&mut out, "By model", &s.by_model);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ev(ts: u64, p: &str, h: &str, m: &str, cwd: &str) -> Event {
        Event { ts, provider: p.into(), harness: h.into(), model: m.into(), cwd: cwd.into() }
    }
    #[test]
    fn summarize_counts_and_ranks() {
        let now = 1_000_000;
        let events = vec![
            ev(now - 10, "openai", "codex", "gpt", "/a"),
            ev(now - 20, "openai", "codex", "gpt", "/a"),
            ev(now - 900_000, "native", "claude", "sonnet", "/b"),
        ];
        let s = summarize(&events, now);
        assert_eq!(s.total, 3);
        assert_eq!(s.last_7_days, 2);
        assert_eq!(s.projects, 2);
        assert_eq!(s.by_provider[0], ("openai".into(), 2));
        assert_eq!(s.by_harness[0].0, "codex");
    }
    #[test]
    fn empty_usage_renders_placeholder() {
        assert!(render(&[], 0).contains("none yet"));
    }
}
