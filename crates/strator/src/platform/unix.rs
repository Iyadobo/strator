use crate::catalog::LaunchSpec;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn executable(command: &str) -> Option<PathBuf> {
    let paths = if command.contains('/') {
        vec![PathBuf::new()]
    } else {
        std::env::split_paths(&std::env::var_os("PATH")?).collect()
    };
    paths.into_iter().map(|p| p.join(command)).find(|p| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}
pub fn shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
}
pub fn config_dir() -> std::io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path).join("strator"));
    }
    std::env::var_os("HOME")
        .map(|p| PathBuf::from(p).join(".config/strator"))
        .ok_or_else(|| std::io::Error::other("Config directory is unavailable."))
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub fn runner_command(executable: &str, endpoint: &str, token: &str) -> String {
    format!(
        "{} __run {} {}",
        quote(executable),
        quote(endpoint),
        quote(token)
    )
}
pub fn output_spec(spec: &LaunchSpec) -> std::io::Result<std::process::Output> {
    Command::new(&spec.command)
        .args(&spec.args)
        .envs(&spec.env)
        .current_dir(&spec.cwd)
        .output()
}
pub fn spawn_spec(spec: &LaunchSpec) -> std::io::Result<i32> {
    Ok(Command::new(&spec.command)
        .args(&spec.args)
        .envs(&spec.env)
        .current_dir(&spec.cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?
        .code()
        .unwrap_or(1))
}
pub fn background(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_shell_quoted() {
        assert_eq!(quote("it's a path"), "'it'\\''s a path'");
    }
}
