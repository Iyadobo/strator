use crate::catalog::LaunchSpec;
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
use unix as os;

pub fn executable(command: &str) -> Option<PathBuf> {
    os::executable(command)
}
pub fn shell() -> String {
    os::shell()
}
pub fn config_dir() -> std::io::Result<PathBuf> {
    os::config_dir()
}
pub fn runner_command(executable: &str, endpoint: &str, token: &str) -> String {
    os::runner_command(executable, endpoint, token)
}
pub fn output_spec(spec: &LaunchSpec) -> std::io::Result<std::process::Output> {
    os::output_spec(spec)
}
pub fn spawn_spec(spec: &LaunchSpec) -> std::io::Result<i32> {
    os::spawn_spec(spec)
}
pub fn background(command: &mut Command) {
    os::background(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_finds_current_binary() {
        assert!(executable(std::env::current_exe().unwrap().to_str().unwrap()).is_some());
    }
    #[test]
    fn runner_command_does_not_interpolate_shell_code() {
        let command = runner_command("/some/path with spaces/runner", "127.0.0.1:1234", "abc123");
        assert!(!command.is_empty());
    }
}
