//! Agent progress from a live engine snapshot (`strator-engine api snapshot`).
use serde_json::Value;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub total: usize,
    pub working: usize,
    pub blocked: usize,
    pub done: usize,
    pub idle: usize,
    pub unknown: usize,
    pub rows: Vec<(String, String, String)>, // pane, status, label
}

pub fn bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 || width == 0 {
        return "-".repeat(width);
    }
    let filled = (done * width) / total;
    format!("{}{}", "#".repeat(filled), "-".repeat(width - filled))
}

pub fn summarize(snapshot: &Value) -> Progress {
    let mut p = Progress::default();
    let snap = &snapshot["snapshot"];
    let workspaces = snap["workspaces"].as_array();
    for pane in snap["panes"].as_array().into_iter().flatten() {
        let status = pane["agent_status"].as_str().unwrap_or("unknown");
        p.total += 1;
        match status {
            "working" => p.working += 1,
            "blocked" => p.blocked += 1,
            "done" => p.done += 1,
            "idle" => p.idle += 1,
            _ => p.unknown += 1,
        }
        let label = workspaces
            .and_then(|list| list.iter().find(|w| w["workspace_id"] == pane["workspace_id"]))
            .and_then(|w| w["label"].as_str())
            .unwrap_or("Terminal")
            .to_string();
        p.rows.push((
            pane["pane_id"].as_str().unwrap_or("?").into(),
            status.into(),
            label,
        ));
    }
    p
}

pub fn render(p: &Progress) -> String {
    let mut out = format!(
        "Agents: {} · working {} · blocked {} · done {} · idle {} · unknown {}\nDone [{}] {}/{}\n",
        p.total,
        p.working,
        p.blocked,
        p.done,
        p.idle,
        p.unknown,
        bar(p.done, p.total, 24),
        p.done,
        p.total
    );
    if p.rows.is_empty() {
        out.push_str("\nNo live panes. Start one with: strator launch\n");
    }
    for (pane, status, label) in &p.rows {
        out.push_str(&format!("\n  {pane:<12} {status:<10} {label}"));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn summarize_snapshot_counts_statuses() {
        let snap = json!({"snapshot":{"workspaces":[{"workspace_id":"w1","label":"api"}],
            "panes":[{"pane_id":"p1","workspace_id":"w1","agent_status":"working"},
                     {"pane_id":"p2","workspace_id":"w1","agent_status":"done"},
                     {"pane_id":"p3","workspace_id":"w9","agent_status":"blocked"}]}});
        let p = summarize(&snap);
        assert_eq!((p.total, p.working, p.done, p.blocked), (3, 1, 1, 1));
        assert_eq!(p.rows[0].2, "api");
        assert_eq!(p.rows[2].2, "Terminal");
        assert!(render(&p).contains("Done [#"));
    }
    #[test]
    fn bar_handles_zero_total() {
        assert_eq!(bar(0, 0, 4), "----");
        assert_eq!(bar(1, 2, 4), "##--");
    }
}
