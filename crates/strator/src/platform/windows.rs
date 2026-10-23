use crate::catalog::LaunchSpec;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn executable(command: &str) -> Option<PathBuf> {
    let explicit = command.contains(['\\', '/']);
    let paths = if explicit {
        vec![PathBuf::new()]
    } else {
        std::env::split_paths(&std::env::var_os("PATH")?).collect()
    };
    let extensions = if PathBuf::from(command).extension().is_some() {
        vec![""]
    } else {
        vec![".exe", ".ps1", ".cmd", ".bat", ""]
    };
    paths
        .iter()
        .flat_map(|p| {
            extensions
                .iter()
                .map(move |e| p.join(format!("{command}{e}")))
        })
        .find(|p| p.is_file())
}
pub fn shell() -> String {
    "powershell.exe".into()
}
pub fn config_dir() -> std::io::Result<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("Strator"))
        .ok_or_else(|| std::io::Error::other("LOCALAPPDATA is unavailable."))
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
pub fn runner_command(executable: &str, endpoint: &str, token: &str) -> String {
    // Force a known shell even when the Strator pane uses cmd or another shell.
    let script = format!(
        "& {} __run {} {}",
        quote(executable),
        quote(endpoint),
        quote(token)
    );
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!(
        "powershell.exe -NoLogo -NoProfile -EncodedCommand {}",
        base64(&bytes)
    )
}
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as usize;
        let b = *chunk.get(1).unwrap_or(&0) as usize;
        let c = *chunk.get(2).unwrap_or(&0) as usize;
        result.push(TABLE[a >> 2] as char);
        result.push(TABLE[((a & 3) << 4) | (b >> 4)] as char);
        result.push(if chunk.len() > 1 {
            TABLE[((b & 15) << 2) | (c >> 6)] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            TABLE[c & 63] as char
        } else {
            '='
        });
    }
    result
}
fn spec_command(spec: &LaunchSpec) -> std::io::Result<Command> {
    let exe = executable(&spec.command).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Harness executable is not installed.",
        )
    })?;
    let mut command = if exe.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("ps1")
            || e.eq_ignore_ascii_case("cmd")
            || e.eq_ignore_ascii_case("bat")
    }) {
        let mut c = Command::new("powershell.exe");
        c.args(["-NoLogo","-NoProfile","-ExecutionPolicy","Bypass","-Command","$a = ConvertFrom-Json $env:STRATOR_ARGS; & $env:STRATOR_EXECUTABLE @a; exit $LASTEXITCODE"]);
        c.env("STRATOR_EXECUTABLE", exe);
        c.env("STRATOR_ARGS", serde_json::to_string(&spec.args)?);
        c
    } else {
        let mut c = Command::new(exe);
        c.args(&spec.args);
        c
    };
    command.current_dir(&spec.cwd).envs(&spec.env);
    Ok(command)
}
pub fn output_spec(spec: &LaunchSpec) -> std::io::Result<std::process::Output> {
    let mut command = spec_command(spec)?;
    background(&mut command);
    command.output()
}
pub fn spawn_spec(spec: &LaunchSpec) -> std::io::Result<i32> {
    Ok(spec_command(spec)?
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?
        .code()
        .unwrap_or(1))
}
pub fn background(command: &mut Command) {
    command.creation_flags(0x08000000);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn script_arguments_remain_separate_in_windows_powershell() {
        let path = std::env::temp_dir().join(format!("strator-args-{}.ps1", std::process::id()));
        std::fs::write(&path, "ConvertTo-Json -Compress -InputObject @($args)\n").unwrap();
        let spec = LaunchSpec {
            command: path.to_string_lossy().into_owned(),
            args: vec![
                "--model".into(),
                "opencode-go/kimi-k3".into(),
                "two words".into(),
            ],
            env: Default::default(),
            cwd: std::env::temp_dir().to_string_lossy().into_owned(),
        };
        let output = output_spec(&spec).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(output.status.success());
        let args: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(args, spec.args);
    }
    #[test]
    fn encoding_has_no_shell_metacharacters() {
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"a"), "YQ==");
        let c = runner_command("C:\\it's a path\\runner.exe", "127.0.0.1:1234", "abc123");
        assert!(!c.contains("it's"));
        assert!(c.starts_with("powershell.exe -NoLogo -NoProfile -EncodedCommand "));
    }
}
