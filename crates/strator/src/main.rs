mod catalog;
mod dashboard;
mod progress;
mod themes;
mod usage;
mod credentials;
mod go;
mod handoff;
mod models;
mod platform;
mod profiles;
mod ui;

use catalog::{HARNESSES, PROVIDERS};
use serde_json::Value;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const HELP:&str="Strator — default-first terminal launcher: your harness, its own login and model picker. BYOK is secondary.\n\n  strator                          Dashboard: Launch / Usage / Progress / Themes / Help\n  strator home                     Same dashboard, explicitly\n  strator usage                    Launch history and counts\n  strator progress                 Live agent states and done bar\n  strator themes [set T [BG]]      List or apply theme + background\n  strator doctor                   Engine, config and profile check\n  strator open [--harness ID]      DEFAULT: open an installed harness with CLI default\n                                   model, its own login + native model picker (no forced --model)\n  strator byok [--harness ID] [--provider ID]\n                                   SECONDARY, BYOK: launch codex / claude / opencode / etc\n                                   with your API key. Key is saved in the OS credential\n                                   store (env, saved, or masked entry) and reused next time.\n                                   Examples: strator byok --harness codex\n                                             strator byok --harness claude --provider anthropic\n  strator open --byok              Same BYOK path, long form\n  strator launch [OPTIONS]         Advanced provider-first launch (catalog model picker)\n  open/launch flags: --model MODEL overrides CLI default; --cwd PATH; --no-attach\n  strator providers               List model providers and API formats\n  strator harnesses                List harnesses, installation, and BYOK support\n  strator list                     List live panes and agent state\n  strator attach                   Open the full Strator TUI\n  strator stop PANE_ID             Close that pane and stop its process\n  strator profiles                 List auto-saved launch profiles\n  strator profiles forget ID       Remove a profile (keeps saved keys)\n  strator keys save|status|forget --provider ID [--base-url URL]\n\nLaunch options:\n  --profile ID    --provider ID    --harness ID    --model MODEL    --cwd PATH\n  --base-url URL   --key-env NAME  --session NAME   --no-attach\n\nBYOK keys come from environment, then the OS credential store, then masked entry.\nEvery resolved BYOK key is saved securely for the next launch. Use keys forget to remove.\nDefault (native) launches use no Strator key at all: the harness login stays in charge.\nModels are fetched from the provider; type in the picker to filter.\nThere is no --api-key option or plaintext credential file. API formats must\nmatch: Codex uses Responses; Claude uses Messages; chat endpoints use OpenCode\nor Aider. STRATOR_ENGINE selects a Strator runtime outside PATH.\n";

#[derive(Default)]
struct Options {
    profile: Option<String>,
    provider: Option<String>,
    harness: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    endpoint: Option<String>,
    key_env: Option<String>,
    session: String,
    no_attach: bool,
    byok: bool,
    cli_default: bool,
}
fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        session: "strator".into(),
        ..Options::default()
    };
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        if flag == "--no-attach" {
            o.no_attach = true;
            i += 1;
            continue;
        }
        if flag == "--byok" {
            o.byok = true;
            i += 1;
            continue;
        }
        if flag == "--cli-default" {
            o.cli_default = true;
            i += 1;
            continue;
        }
        let target = match flag {
            "--profile" => &mut o.profile,
            "--provider" => &mut o.provider,
            "--harness" => &mut o.harness,
            "--model" => &mut o.model,
            "--cwd" => &mut o.cwd,
            "--base-url" => &mut o.endpoint,
            "--key-env" => &mut o.key_env,
            "--session" => {
                i += 1;
                o.session = args.get(i).ok_or("Missing session name.")?.clone();
                i += 1;
                continue;
            }
            _ => return Err(format!("Unknown option: {flag}")),
        };
        i += 1;
        *target = Some(
            args.get(i)
                .ok_or_else(|| format!("Missing value for {flag}"))?
                .clone(),
        );
        i += 1;
    }
    if o.session.is_empty()
        || o.session.len() > 64
        || !o
            .session
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Session name must be 1–64 letters, digits, underscores, or hyphens.".into());
    }
    Ok(o)
}
fn runtime() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("STRATOR_ENGINE") {
        return platform::executable(&path.to_string_lossy())
            .ok_or_else(|| io::Error::other("STRATOR_ENGINE is not an executable file."));
    }
    if let Some(path) = platform::executable("strator-engine") {
        return Ok(path);
    }
    let current = std::env::current_exe()?;
    for parent in current.ancestors().skip(1) {
        if let Some(path) = platform::executable(&parent.join("strator-engine").to_string_lossy()) {
            return Ok(path);
        }
        if let Some(path) =
            platform::executable(&parent.join(".local/strator/strator-engine").to_string_lossy())
        {
            return Ok(path);
        }
    }
    Err(io::Error::other(
        "Install Strator, or set STRATOR_ENGINE to its executable path.",
    ))
}
fn command(runtime: &PathBuf, session: &str) -> Command {
    let mut c = Command::new(runtime);
    c.args(["--session", session]);
    c.env_remove("STRATOR_SOCKET_PATH")
        .env_remove("STRATOR_CLIENT_SOCKET_PATH");
    c
}
fn api(runtime: &PathBuf, session: &str, args: &[&str]) -> io::Result<Value> {
    let mut c = command(runtime, session);
    c.args(args);
    platform::background(&mut c);
    let result = c.output()?;
    if !result.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&result.stderr).trim().to_string(),
        ));
    }
    // Raw pane input commands acknowledge success with an empty stdout.
    if result.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Null);
    }
    let response: Value = serde_json::from_slice(&result.stdout)
        .map_err(|_| io::Error::other("Strator returned invalid API output."))?;
    if let Some(error) = response.get("error") {
        return Err(io::Error::other(
            error["message"]
                .as_str()
                .unwrap_or("Strator rejected the action."),
        ));
    }
    Ok(response["result"].clone())
}
fn start(runtime: &PathBuf, session: &str) -> io::Result<()> {
    if api(runtime, session, &["api", "snapshot"]).is_ok() {
        return Ok(());
    }
    let mut c = command(runtime, session);
    c.arg("server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    platform::background(&mut c);
    let mut child = c.spawn()?;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(200));
        if api(runtime, session, &["api", "snapshot"]).is_ok() {
            return Ok(());
        }
        if child.try_wait()?.is_some() {
            return Err(io::Error::other(
                "Strator server exited before becoming ready. Check its named-session log.",
            ));
        }
    }
    Err(io::Error::other(
        "Strator did not become ready. Check its named-session log.",
    ))
}
fn attach(runtime: &PathBuf, session: &str) -> io::Result<i32> {
    Ok(command(runtime, session).status()?.code().unwrap_or(1))
}
fn apply_profile(o: &mut Options, profile: &profiles::Profile) {
    let s = &profile.setup;
    o.provider.get_or_insert_with(|| s.provider.clone());
    o.harness.get_or_insert_with(|| s.harness.clone());
    o.model.get_or_insert_with(|| s.model.clone());
    o.cwd.get_or_insert_with(|| s.cwd.clone());
    o.endpoint.get_or_insert_with(|| s.endpoint.clone());
    o.key_env.get_or_insert_with(|| s.key_env.clone());
}
fn launch(mut o: Options) -> io::Result<i32> {
    let store = profiles::Store::user()?;
    let saved = store.load()?;
    if let Some(id) = &o.profile {
        if o.provider.is_some()
            || o.harness.is_some()
            || o.endpoint.is_some()
            || o.key_env.is_some()
        {
            return Err(io::Error::other("A saved profile can override --model and --cwd. Choose a new launch to change its provider, harness, endpoint, or key variable."));
        }
        let id: u64 = id
            .parse()
            .map_err(|_| io::Error::other("Profile ID must be a number."))?;
        let profile = saved
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| io::Error::other("Unknown saved profile. Run strator profiles."))?;
        apply_profile(&mut o, profile);
    }
    if o.provider.is_none() {
        let mut rows: Vec<_> = saved.iter().map(profiles::Profile::label).collect();
        rows.extend(PROVIDERS.iter().map(|p| {
            format!(
                "{}{}",
                p.name,
                if p.id == "opencode-go" {
                    "  [provider preset]".into()
                } else if p.key_env.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", p.key_env)
                }
            )
        }));
        let Some(index) = ui::choose(
            " Profiles & providers ",
            "Reuse a saved profile, or choose a provider. New launches save automatically.",
            &rows,
        )?
        else {
            return Ok(0);
        };
        if index < saved.len() {
            apply_profile(&mut o, &saved[index]);
        } else {
            o.provider = Some(PROVIDERS[index - saved.len()].id.into());
        }
    }
    let provider = PROVIDERS
        .iter()
        .find(|p| Some(p.id) == o.provider.as_deref())
        .ok_or_else(|| io::Error::other("Unknown provider. Run strator providers."))?;
    let available: Vec<_> = HARNESSES
        .iter()
        .filter(|h| h.providers.contains(&provider.id))
        .collect();
    let harness = if let Some(id) = &o.harness {
        HARNESSES
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| io::Error::other("Unknown harness. Run strator harnesses."))?
    } else {
        let rows: Vec<_> = available
            .iter()
            .map(|h| {
                format!(
                    "{}  {}",
                    h.name,
                    if h.id == "terminal" || platform::executable(h.command).is_some() {
                        "installed"
                    } else {
                        "not installed"
                    }
                )
            })
            .collect();
        let Some(index) = ui::choose(
            " Harness ",
            &format!(
                "Provider: {}. Only matching API adapters appear.",
                provider.name
            ),
            &rows,
        )?
        else {
            return Ok(0);
        };
        available[index]
    };
    if !harness.providers.contains(&provider.id) {
        return Err(io::Error::other(
            "This harness does not support the chosen API format.",
        ));
    }
    let harness_command = if harness.id == "terminal" {
        platform::shell()
    } else {
        harness.command.into()
    };
    if platform::executable(&harness_command).is_none() {
        return Err(io::Error::other(format!(
            "{} is not installed. Setup: {}",
            harness.name, harness.install
        )));
    }
    let interactive = io::stdin().is_terminal();
    let current = std::env::current_dir()?.to_string_lossy().into_owned();
    let cwd = match o.cwd {
        Some(p) => p,
        None if interactive => ui::text("Project folder", &current)?,
        None => current,
    };
    if !PathBuf::from(&cwd).is_dir() {
        return Err(io::Error::other("Project folder does not exist."));
    }
    let endpoint = match o.endpoint {
        Some(url) => url,
        None if interactive && ["compatible", "responses", "gateway"].contains(&provider.id) => {
            ui::text("API endpoint URL", "")?
        }
        None => provider.endpoint.into(),
    };
    let key_env = o.key_env.as_deref().unwrap_or(provider.key_env);
    if provider.id == "opencode-go" && endpoint.trim_end_matches('/') != provider.endpoint {
        return Err(io::Error::other("The OpenCode Go preset uses its official endpoint. Choose a custom provider for another URL."));
    }
    if !endpoint.is_empty() {
        catalog::validate_endpoint(&endpoint).map_err(io::Error::other)?;
    }
    let key = if provider.id == "native" {
        None
    } else {
        let env_key = std::env::var(key_env).ok().filter(|k| !k.is_empty());
        let saved_key = if env_key.is_none() {
            credentials::load(provider.id, &endpoint, key_env)?
        } else {
            None
        };
        let (key, source) = match key_source(env_key.is_some(), saved_key.is_some()) {
            KeySource::Env => (env_key.unwrap(), KeySource::Env),
            KeySource::Saved => (saved_key.unwrap(), KeySource::Saved),
            KeySource::Prompt => (ui::secret(key_env)?, KeySource::Prompt),
        };
        // Make it save: whichever way the key arrived, persist it in the OS
        // credential store so the next BYOK launch for this provider +
        // endpoint needs no key at all. Failure only warns; launch continues.
        match credentials::save(provider.id, &endpoint, key_env, &key) {
            Ok(()) => match source {
                KeySource::Saved => println!("Using saved {} key.", provider.name),
                _ => println!("{} key saved in your OS credential store.", provider.name),
            },
            Err(error) => eprintln!("Key could not be saved ({error}); continuing without saving."),
        }
        Some(key)
    };
    let model = match o.model {
        Some(m) => m,
        None if o.cli_default => String::new(),
        None if interactive && harness.model_flag => {
            let mut ids = Vec::new();
            if let Some(ref key) = key {
                println!("Fetching available models…");
                match models::fetch(provider.id, &endpoint, key) {
                    Ok(found) => ids = found,
                    Err(error) => println!("{error} You can enter a model ID manually."),
                }
            } else if harness.id == "claude" {
                ids = ["sonnet", "opus", "haiku", "opusplan"]
                    .map(String::from)
                    .to_vec();
            }
            if provider.id == "opencode-go" {
                ids.retain(|model| go::supports(harness.id, model));
            }
            let requires_model = provider.id != "native"
                && (provider.id == "opencode-go"
                    || ["aider", "pi"].contains(&harness.id)
                    || (harness.id == "opencode"
                        && ["compatible", "responses"].contains(&provider.id)));
            let mut rows = Vec::new();
            if !requires_model {
                rows.push("Use harness default".into());
            }
            let manual_index = rows.len();
            rows.push("Enter model ID manually…".into());
            rows.extend(ids.iter().cloned());
            let Some(index) = ui::choose(
                " Model ",
                "Provider catalog. Model support also depends on the harness.",
                &rows,
            )?
            else {
                return Ok(0);
            };
            if index == manual_index {
                ui::text("Model ID", "")?
            } else if !requires_model && index == 0 {
                String::new()
            } else {
                ids[index - manual_index - 1].clone()
            }
        }
        None => String::new(),
    };
    let setup = profiles::Setup {
        provider: provider.id.into(),
        harness: harness.id.into(),
        model: model.clone(),
        endpoint: endpoint.clone(),
        key_env: key_env.into(),
        cwd: std::fs::canonicalize(&cwd)?.to_string_lossy().into_owned(),
    };
    let mut spec = catalog::build(
        harness,
        provider,
        &model,
        &endpoint,
        key_env,
        key,
        cwd.clone(),
    )
    .map_err(io::Error::other)?;
    spec.command = harness_command;
    let runtime = runtime()?;
    start(&runtime, &o.session)?;
    let handoff = handoff::serve(spec)?;
    let label = format!("{} / {}", harness.name, provider.id);
    let created = api(
        &runtime,
        &o.session,
        &[
            "workspace",
            "create",
            "--cwd",
            &cwd,
            "--label",
            &label,
            "--focus",
        ],
    )?;
    let pane = created["root_pane"]["pane_id"]
        .as_str()
        .ok_or_else(|| io::Error::other("Strator did not return a pane ID."))?;
    let workspace = created["workspace"]["workspace_id"]
        .as_str()
        .ok_or_else(|| io::Error::other("Strator did not return a workspace ID."))?;
    let launched = (|| {
        let mut ready = false;
        for _ in 0..40 {
            let info = api(
                &runtime,
                &o.session,
                &["pane", "process-info", "--pane", pane],
            )?;
            let shell = info["process_info"]["shell_pid"].as_u64();
            if let Some(processes) = info["process_info"]["foreground_processes"].as_array() {
                if !processes.is_empty() && processes.iter().all(|p| p["pid"].as_u64() == shell) {
                    ready = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !ready {
            return Err(io::Error::other("New pane shell did not become ready."));
        }
        let executable = std::env::current_exe()?.to_string_lossy().into_owned();
        api(
            &runtime,
            &o.session,
            &[
                "pane",
                "run",
                pane,
                &platform::runner_command(&executable, &handoff.address, &handoff.token),
            ],
        )?;
        if !handoff
            .claimed
            .recv_timeout(Duration::from_secs(40))
            .unwrap_or(false)
        {
            return Err(io::Error::other(
                "Harness did not claim its launch ticket. Check the pane shell.",
            ));
        }
        println!(
            "Launched {} with {} in {pane}.",
            harness.name, provider.name
        );
        println!("Attach: strator attach --session {}", o.session);
        Ok(())
    })();
    if let Err(error) = launched {
        let _ = api(&runtime, &o.session, &["workspace", "close", workspace]);
        return Err(error);
    }
    let _ = usage::record(provider.id, harness.id, &model, &setup.cwd);
    match store.save(setup) {
        Ok(id) => println!("Profile #{id} saved automatically."),
        Err(error) => eprintln!("Harness launched, but profile could not be saved: {error}"),
    }
    if o.no_attach {
        Ok(0)
    } else {
        attach(&runtime, &o.session)
    }
}

pub fn dashboard_data() -> Option<(String, String)> {
    let usage_text = usage::render(&usage::load(), usage::now_secs());
    let progress_text = match runtime().ok().and_then(|r| api(&r, "strator", &["api", "snapshot"]).ok()) {
        Some(snap) => progress::render(&progress::summarize(&snap)),
        None => "No live Strator session.\nStart one with: strator launch\n".to_string(),
    };
    Some((usage_text, progress_text))
}
fn doctor() -> io::Result<i32> {
    match runtime() {
        Ok(path) => println!("engine: {}", path.display()),
        Err(e) => println!("engine: missing ({e})"),
    }
    println!("dashboard prefs: {}", themes::Prefs::load().theme);
    println!("engine config: {}", themes::engine_config_path().display());
    println!("profiles: {}", profiles::Store::user()?.load()?.len());
    println!("launches logged: {}", usage::load().len());
    Ok(0)
}

/// Open a harness you already have installed/logged in.
/// Two honest modes, nothing forced:
/// - CLI default: provider `native`, no `--model`. The harness uses its
///   own login, default model, and native in-CLI model picker.
/// - BYOK (`--byok`): pick a compatible provider for *this* harness,
///   key from env / OS credential store / masked entry (saved securely),
///   then Strator's provider model picker.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    CliDefault,
    Byok,
}

/// Default-first mode choice for `strator open`: no flag means CLI default
/// (own login, default model, native picker). BYOK is the secondary opt-in.
pub fn open_mode(byok: bool, cli_default: bool) -> Result<OpenMode, String> {
    match (byok, cli_default) {
        (true, true) => Err("Choose one: --byok or --cli-default, not both.".into()),
        (true, false) => Ok(OpenMode::Byok),
        _ => Ok(OpenMode::CliDefault),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Env,
    Saved,
    Prompt,
}

/// Where a BYOK key comes from, in priority order. Every resolved key is
/// then saved to the OS credential store (see launch), so a key supplied
/// once — typed or from the environment — is reused next time.
pub fn key_source(env_present: bool, saved_present: bool) -> KeySource {
    if env_present {
        KeySource::Env
    } else if saved_present {
        KeySource::Saved
    } else {
        KeySource::Prompt
    }
}
fn open_harness(mut o: Options) -> io::Result<i32> {
    let mode = open_mode(o.byok, o.cli_default).map_err(io::Error::other)?;
    // Pick from harnesses actually installed on this machine.
    let installed: Vec<_> = HARNESSES
        .iter()
        .filter(|h| h.id == "terminal" || platform::executable(h.command).is_some())
        .collect();
    let harness = if let Some(id) = &o.harness {
        HARNESSES
            .iter()
            .find(|h| h.id == id)
            .ok_or_else(|| io::Error::other("Unknown harness. Run strator harnesses."))?
    } else {
        if installed.is_empty() {
            return Err(io::Error::other("No supported harness found on PATH. Run strator harnesses."));
        }
        let rows: Vec<_> = installed.iter().map(|h| format!("{}  installed", h.name)).collect();
        let Some(index) = ui::choose(" Open harness ", "Harnesses found on this machine.", &rows)? else {
            return Ok(0);
        };
        installed[index]
    };
    o.harness = Some(harness.id.into());
    if mode == OpenMode::Byok {
        let compatible: Vec<_> = PROVIDERS
            .iter()
            .filter(|p| p.id != "native" && harness.providers.contains(&p.id))
            .collect();
        if compatible.is_empty() {
            return Err(io::Error::other(format!(
                "{} has no BYOK API provider in Strator. Use CLI default instead: strator open --harness {}",
                harness.name, harness.id
            )));
        }
        if o.provider.is_none() {
            let rows: Vec<_> = compatible
                .iter()
                .map(|p| format!("{}  [{}]", p.name, p.key_env))
                .collect();
            let Some(index) = ui::choose(
                " BYOK provider ",
                &format!("Your key, for {}. Saved in the OS credential store, never a plain file.", harness.name),
                &rows,
            )? else {
                return Ok(0);
            };
            o.provider = Some(compatible[index].id.into());
        }
    } else {
        // Default path (also bare `strator open`): CLI default model.
        o.provider = Some("native".into());
        o.cli_default = true;
    }
    launch(o)
}
fn run() -> io::Result<i32> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "__run") {
        if args.len() != 3 {
            return Err(io::Error::other("Invalid runner invocation."));
        }
        let spec = handoff::receive(&args[1], &args[2])?;
        return platform::spawn_spec(&spec);
    }
    if args.is_empty() && io::stdin().is_terminal() {
        let (u, p) = dashboard_data().unwrap_or_default();
        return match dashboard::run(u, p)? {
            dashboard::Action::Launch => launch(options(&[]).map_err(io::Error::other)?),
            dashboard::Action::OpenDefault => open_harness(Options { session: "strator".into(), ..Options::default() }),
            dashboard::Action::OpenByok => open_harness(Options { byok: true, session: "strator".into(), ..Options::default() }),
            dashboard::Action::Attach => {
                let o = options(&[]).map_err(io::Error::other)?;
                let runtime = runtime()?;
                start(&runtime, &o.session)?;
                attach(&runtime, &o.session)
            }
            dashboard::Action::Quit => Ok(0),
        };
    }
    let action = args.first().map(String::as_str).unwrap_or("launch");
    match action {
        "home" => {
            let (u, p) = dashboard_data().unwrap_or_default();
            match dashboard::run(u, p)? {
                dashboard::Action::Launch => launch(options(&[]).map_err(io::Error::other)?),
            dashboard::Action::OpenDefault => open_harness(Options { session: "strator".into(), ..Options::default() }),
            dashboard::Action::OpenByok => open_harness(Options { byok: true, session: "strator".into(), ..Options::default() }),
                dashboard::Action::Attach => {
                    let o = options(&[]).map_err(io::Error::other)?;
                    let runtime = runtime()?;
                    start(&runtime, &o.session)?;
                    attach(&runtime, &o.session)
                }
                dashboard::Action::Quit => Ok(0),
            }
        }
        "open" => open_harness(options(&args[1..]).map_err(io::Error::other)?),
        "byok" => {
            let mut o = options(&args[1..]).map_err(io::Error::other)?;
            o.byok = true;
            open_harness(o)
        }
        "usage" => {
            println!("{}", usage::render(&usage::load(), usage::now_secs()));
            Ok(0)
        }
        "progress" => {
            let o = options(&args[1..]).map_err(io::Error::other)?;
            match api(&runtime()?, &o.session, &["api", "snapshot"]) {
                Ok(snap) => println!("{}", progress::render(&progress::summarize(&snap))),
                Err(_) => println!("No live Strator session. Start one with: strator launch"),
            }
            Ok(0)
        }
        "doctor" => doctor(),
        "themes" => {
            if args.get(1).map(String::as_str) == Some("set") {
                let theme = args.get(2).ok_or_else(|| io::Error::other("Usage: strator themes set THEME [BACKGROUND]"))?;
                let bg = args.get(3).map(String::as_str).unwrap_or("midnight");
                let path = themes::apply(theme, bg)?;
                println!("Applied {theme} / {bg} -> {}", path.display());
            } else {
                println!("Themes: {}", themes::THEMES.join(", "));
                println!("Backgrounds: {}", themes::BACKGROUNDS.iter().map(|b| b.name).collect::<Vec<_>>().join(", "));
                println!("Set: strator themes set THEME [BACKGROUND]");
            }
            Ok(0)
        }
        "--help" | "-h" | "help" => {
            print!("{HELP}");
            Ok(0)
        }
        "--version" => {
            println!("Strator {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        "providers" => {
            for p in PROVIDERS {
                println!("{:<13} {:<43} {}", p.id, p.name, p.key_env);
            }
            Ok(0)
        }
        "harnesses" => {
            for h in HARNESSES {
                println!(
                    "{:<10} {:<20} {:<13} {}",
                    h.id,
                    h.name,
                    if h.id == "terminal" || platform::executable(h.command).is_some() {
                        "installed"
                    } else {
                        "not installed"
                    },
                    h.providers.join(", ")
                );
            }
            Ok(0)
        }
        "profiles" => {
            let store = profiles::Store::user()?;
            if args.get(1).is_some_and(|v| v == "forget") {
                let id = args
                    .get(2)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| io::Error::other("Usage: strator profiles forget ID"))?;
                store.forget(id)?;
                println!("Profile #{id} removed. Saved keys are unchanged.");
            } else if args.len() == 1 {
                for profile in store.load()? {
                    println!("{}", profile.label());
                }
            } else {
                return Err(io::Error::other("Usage: strator profiles [forget ID]"));
            }
            Ok(0)
        }
        "keys" => {
            let verb = args.get(1).map(String::as_str).unwrap_or("status");
            if !["save", "forget", "status"].contains(&verb) {
                return Err(io::Error::other("Usage: strator keys save|forget|status --provider ID [--base-url URL] [--key-env NAME]"));
            }
            let o = options(&args[2.min(args.len())..]).map_err(io::Error::other)?;
            let provider = PROVIDERS
                .iter()
                .find(|p| Some(p.id) == o.provider.as_deref())
                .filter(|p| p.id != "native")
                .ok_or_else(|| io::Error::other("Choose an API provider with --provider ID."))?;
            let endpoint = o.endpoint.as_deref().unwrap_or(provider.endpoint);
            if ["compatible", "responses", "gateway"].contains(&provider.id) && endpoint.is_empty()
            {
                return Err(io::Error::other("Custom providers require --base-url URL."));
            }
            if !endpoint.is_empty() {
                catalog::validate_endpoint(endpoint).map_err(io::Error::other)?;
            }
            let variable = o.key_env.as_deref().unwrap_or(provider.key_env);
            match verb {
                "save" => {
                    let key = match std::env::var(variable) {
                        Ok(key) if !key.is_empty() => key,
                        _ => ui::secret(variable)?,
                    };
                    credentials::save(provider.id, endpoint, variable, &key)?;
                    println!("Key saved in your OS credential store.");
                }
                "forget" => {
                    credentials::forget(provider.id, endpoint, variable)?;
                    println!("Saved key removed. Environment keys are unchanged.");
                }
                _ => println!(
                    "Saved key: {}",
                    if credentials::load(provider.id, endpoint, variable)?.is_some() {
                        "present"
                    } else {
                        "none"
                    }
                ),
            }
            Ok(0)
        }
        "launch" => launch(
            options(if args.is_empty() { &[] } else { &args[1..] }).map_err(io::Error::other)?,
        ),
        "attach" => {
            let o = options(&args[1..]).map_err(io::Error::other)?;
            let runtime = runtime()?;
            start(&runtime, &o.session)?;
            attach(&runtime, &o.session)
        }
        "list" => {
            let o = options(&args[1..]).map_err(io::Error::other)?;
            let snapshot = api(&runtime()?, &o.session, &["api", "snapshot"])?;
            println!("{:<12} {:<12} PROJECT / HARNESS", "PANE", "STATE");
            for pane in snapshot["snapshot"]["panes"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let workspace = snapshot["snapshot"]["workspaces"]
                    .as_array()
                    .and_then(|w| w.iter().find(|w| w["workspace_id"] == pane["workspace_id"]));
                println!(
                    "{:<12} {:<12} {}",
                    pane["pane_id"].as_str().unwrap_or("?"),
                    pane["agent_status"].as_str().unwrap_or("unknown"),
                    workspace
                        .and_then(|w| w["label"].as_str())
                        .unwrap_or("Terminal")
                );
            }
            Ok(0)
        }
        "stop" => {
            let pane = args.get(1).ok_or_else(|| {
                io::Error::other("Usage: strator stop PANE_ID [--session NAME]")
            })?;
            let o = options(&args[2..]).map_err(io::Error::other)?;
            api(&runtime()?, &o.session, &["pane", "close", pane])?;
            println!("Stopped {pane}.");
            Ok(0)
        }
        _ => Err(io::Error::other("Unknown command. Run strator --help.")),
    }
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("Strator: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_or_secret_options_are_rejected() {
        assert!(options(&["--api-key".into(), "fake".into()]).is_err());
        assert!(options(&["--provider".into()]).is_err());
    }
    #[test]
    fn open_mode_is_cli_default_unless_byok() {
        assert_eq!(open_mode(false, false), Ok(OpenMode::CliDefault));
        assert_eq!(open_mode(false, true), Ok(OpenMode::CliDefault));
        assert_eq!(open_mode(true, false), Ok(OpenMode::Byok));
        assert!(open_mode(true, true).is_err());
    }
    #[test]
    fn byok_key_source_priority() {
        assert_eq!(key_source(true, true), KeySource::Env);
        assert_eq!(key_source(false, true), KeySource::Saved);
        assert_eq!(key_source(false, false), KeySource::Prompt);
    }
    #[test]
    fn open_mode_flags_parse_and_reject_secrets() {
        let o = options(&["--byok".into(), "--harness".into(), "codex".into()]).unwrap();
        assert!(o.byok && !o.cli_default);
        let o = options(&["--cli-default".into()]).unwrap();
        assert!(o.cli_default && !o.byok);
        assert!(options(&["--api-key".into(), "fake".into()]).is_err());
    }
    #[test]
    fn session_names_are_bounded() {
        assert!(options(&["--session".into(), "../default".into()]).is_err());
        assert!(options(&["--session".into(), "test-session".into()]).is_ok());
    }
}
