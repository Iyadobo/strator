//! One real terminal pane: a PTY child plus its screen, fed by a reader
//! thread. This is Strator's own pane — no engine, no Herdr runtime.
use crate::screen::{Screen, VtParser};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct PaneSpawn {
    pub title: String,
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
}

impl PaneSpawn {
    pub fn shell(cwd: &str) -> Self {
        Self {
            title: "Shell".into(),
            program: crate::platform::shell(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: cwd.into(),
        }
    }
    /// Spawn an agent LaunchSpec in a pane. On Windows the harness is
    /// usually a .cmd/.exe shim, so run it through PowerShell; the spec
    /// env (provider key etc.) is set on the PTY itself either way.
    pub fn from_spec(spec: &crate::catalog::LaunchSpec, title: String) -> Self {
        if cfg!(windows) {
            let mut line = spec.command.clone();
            for arg in &spec.args {
                line.push(' ');
                line.push_str(&format!("'{}'", arg.replace('\'', "''")));
            }
            Self {
                title,
                program: "powershell.exe".into(),
                args: vec!["-NoLogo".into(), "-Command".into(), line],
                env: spec.env.clone(),
                cwd: spec.cwd.clone(),
            }
        } else {
            Self {
                title,
                program: spec.command.clone(),
                args: spec.args.clone(),
                env: spec.env.clone(),
                cwd: spec.cwd.clone(),
            }
        }
    }
}


/// Answer terminal status queries a shell/agent sends on startup.
/// Without a reply to DSR (`ESC[6n`) PowerShell and many TUIs block
/// before printing anything. Scans the raw stream; queries can split
/// across reads, so the caller keeps a small tail buffer.
pub fn query_replies(bytes: &[u8], cursor_row: usize, cursor_col: usize) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = vec![];
    if text.contains("\x1b[6n") || text.contains("\x1b[?6n") {
        out.extend_from_slice(format!("\x1b[{};{}R", cursor_row + 1, cursor_col + 1).as_bytes());
    }
    if text.contains("\x1b[5n") {
        out.extend_from_slice(b"\x1b[0n");
    }
    if text.contains("\x1b[c") || text.contains("\x1b[0c") {
        out.extend_from_slice(b"\x1b[?62;22c");
    }
    if text.contains("\x1b[>c") || text.contains("\x1b[>0c") {
        out.extend_from_slice(b"\x1b[>0;10;1c");
    }
    if text.contains("\x1b]10;?") {
        out.extend_from_slice(b"\x1b]10;rgb:e5e5/e5e5/e5e5\x1b\\");
    }
    if text.contains("\x1b]11;?") {
        out.extend_from_slice(b"\x1b]11;rgb:0000/0000/0000\x1b\\");
    }
    out
}

pub struct Pane {
    pub title: String,
    pub screen: Arc<Mutex<Screen>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    pub exited: Arc<Mutex<Option<i32>>>,
    pub scroll: usize,
}

impl Pane {
    pub fn spawn(spawn: &PaneSpawn, cols: u16, rows: u16) -> std::io::Result<Self> {
        let pty = native_pty_system()
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(std::io::Error::other)?;
        let mut builder = CommandBuilder::new(&spawn.program);
        builder.args(&spawn.args);
        builder.cwd(&spawn.cwd);
        for (k, v) in &spawn.env {
            builder.env(k, v);
        }
        let child = pty.slave.spawn_command(builder).map_err(std::io::Error::other)?;
        drop(pty.slave);
        let mut reader = pty.master.try_clone_reader().map_err(std::io::Error::other)?;
        let writer = Arc::new(Mutex::new(pty.master.take_writer().map_err(std::io::Error::other)?));
        let screen = Arc::new(Mutex::new(Screen::new(cols as usize, rows as usize)));
        let exited = Arc::new(Mutex::new(None));
        {
            let screen = screen.clone();
            let thread_writer = writer.clone();
            std::thread::spawn(move || {
                let mut parser = VtParser::new();
                let mut buf = [0u8; 8192];
                let mut tail: Vec<u8> = vec![];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let chunk = &buf[..n];
                            let (row, col) = {
                                let Ok(mut s) = screen.lock() else { continue };
                                parser.feed(&mut s, chunk);
                                s.cursor()
                            };
                            // Keep a tail so a query split across reads
                            // is still answered on the next chunk.
                            tail.extend_from_slice(chunk);
                            if tail.len() > 64 {
                                tail.drain(..tail.len() - 64);
                            }
                            let replies = query_replies(&tail, row, col);
                            if !replies.is_empty() {
                                if let Ok(mut w) = thread_writer.lock() {
                                    let _ = w.write_all(&replies);
                                    let _ = w.flush();
                                }
                                tail.clear();
                            }
                        }
                    }
                }
            });
        }
        Ok(Self {
            title: spawn.title.clone(),
            screen,
            writer,
            master: pty.master,
            child,
            exited,
            scroll: 0,
        })
    }
    pub fn write(&self, bytes: &[u8]) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
    }
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        if let Ok(mut s) = self.screen.lock() {
            s.resize(cols as usize, rows as usize);
        }
    }
    pub fn status(&mut self) -> &'static str {
        if self.exited.lock().map(|e| e.is_some()).unwrap_or(false) {
            return "exited";
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                if let Ok(mut e) = self.exited.lock() {
                    *e = Some(status.exit_code() as i32);
                }
                "exited"
            }
            Ok(None) => "running",
            Err(_) => "exited",
        }
    }
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pane_captures_child_output() {
        let spawn = PaneSpawn {
            title: "test".into(),
            program: if cfg!(windows) { "powershell.exe".into() } else { "/bin/sh".into() },
            args: if cfg!(windows) {
                vec!["-NoLogo".into(), "-Command".into(), "echo hello-pane".into()]
            } else {
                vec!["-c".into(), "echo hello-pane".into()]
            },
            env: BTreeMap::new(),
            cwd: std::env::temp_dir().to_string_lossy().into_owned(),
        };
        let mut pane = Pane::spawn(&spawn, 40, 10).unwrap();
        let mut text = String::new();
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let screen = pane.screen.lock().unwrap();
            text = screen
                .display_line(0, 10)
                .iter()
                .flat_map(|row| row.iter().map(|c| c.ch))
                .collect();
            if text.contains("hello-pane") {
                break;
            }
        }
        assert!(text.contains("hello-pane"), "screen was: {text:?}");
        pane.kill();
    }
}

#[cfg(test)]
mod reply_tests {
    use super::query_replies;
    #[test]
    fn dsr_and_da_are_answered() {
        assert_eq!(query_replies(b"\x1b[6n", 2, 4), b"\x1b[3;5R".to_vec());
        assert_eq!(query_replies(b"\x1b[c", 0, 0), b"\x1b[?62;22c".to_vec());
        assert!(query_replies(b"plain output", 0, 0).is_empty());
    }
}
