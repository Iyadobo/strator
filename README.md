# Strator

The home your coding agents live in. Type `strator`, get a dashboard:
launch an agent, watch usage and progress, change the theme — then drop
into a full terminal multiplexer where every agent runs in a real pane.

Strator is a fork of [Herdr](https://github.com/herdrdev/herdr) (Apache-2.0),
rebranded and extended with a tabbed home, usage/progress tracking,
themes + backgrounds, and harness-first launching. Engine credit stays
with the Herdr authors; see `NOTICE`.

## Two binaries, one name to remember

| Binary | What it is |
| --- | --- |
| `strator` | Dashboard + launcher (this is what you type) |
| `strator-engine` | The multiplexer runtime (panes, workspaces, agent detection) |

`strator` finds `strator-engine` beside itself, on `PATH`, or via
`STRATOR_ENGINE=/path/to/strator-engine`.

## Quick start

```sh
cargo build --release          # needs Rust + Zig 0.16 (engine terminal core)
# put both binaries on PATH, e.g.:
cp target/release/strator target/release/strator-engine ~/.cargo/bin/
strator
```

Windows: `Strator.cmd` does the same from a checkout.

## The dashboard

Tabs (keys `1`–`5` or `Tab`):

- **Launch** — provider-first launch (`Enter`), or open a harness you
  already have: **CLI default** (`d`) or **BYOK** (`b`).
- **Usage** — launch history: counts by provider, harness, model, project.
- **Progress** — live agents: working / blocked / done, with a done bar.
- **Themes** — pick a theme + background, `Enter` applies it live to the
  engine config.
- **Help** — every key and command, in one screen.

## Open a harness your way

```sh
strator open                     # installed harness, its own login,
                                 # default model, native model picker
strator open --byok              # your API key for that harness
strator open --harness codex --byok --provider openai
strator open --harness claude --model opus   # explicit override
```

CLI-default mode passes **no** `--model`: the harness keeps its own
login, default model, and in-CLI picker. BYOK keys come from the
environment, the OS credential store, or masked entry — saved securely,
never in a plain file. There is no `--api-key` flag.

## Commands

```sh
strator                 # dashboard
strator launch          # provider -> harness -> model -> attach
strator open [--byok]   # harness-first (see above)
strator usage           # launch history and counts
strator progress        # live agent states + done bar
strator themes          # list themes/backgrounds
strator themes set dracula ember
strator doctor          # engine, config, profiles check
strator attach          # straight into the multiplexer
strator list            # panes + agent states (plain text)
strator stop PANE_ID
strator profiles        # auto-saved launch profiles
strator keys save|status|forget --provider ID
strator providers       # model providers + API formats
strator harnesses       # harnesses, install + BYOK support
```

Detach with `Ctrl+B` then `q`; agents keep running. Reattach any time
with `strator attach`.

## Themes and backgrounds

18 built-in themes (catppuccin, tokyo-night, dracula, nord, gruvbox,
kanagawa, rose-pine, …) and 8 backgrounds (midnight, abyss, ember,
forest, plum, mono, paper, none). Apply from the Themes tab or:

```sh
strator themes set tokyo-night midnight
```

That writes `[theme]` into the engine config
(`%APPDATA%\strator\config.toml` on Windows,
`~/.config/strator/config.toml` elsewhere, or `$STRATOR_CONFIG_PATH`).
Hand-tune further with `[theme.custom]` colors in the same file.

## Layout of this repo

```
src/               strator-engine (multiplexer, agent detection, API)
crates/strator/    strator dashboard + launcher (usage, progress, themes)
crates/ghostty-vt/ terminal core binding
skills/strator/    agent skill for driving Strator from a pane
docs/              upstream docs (herdr-era, being rewritten)
```

## Config and data

- Engine config: `strator/config.toml` in your config dir (above).
- Dashboard prefs, profiles, usage log: `Strator/` in your local app
  data dir (`%LOCALAPPDATA%\Strator` / `$XDG_CONFIG_HOME/strator`).
- Env vars are `STRATOR_*` (`STRATOR_ENGINE`, `STRATOR_CONFIG_PATH`,
  `STRATOR_SOCKET_PATH`, …).

## Build and test

```sh
cargo test -p strator-launcher     # dashboard, usage, progress, themes
cargo test -p strator-engine --bin strator-engine
```

Note: unset `HERMES_HOME` when running engine integration tests —
those tests manage their own fake homes.

## License

Apache-2.0, same as upstream. See `LICENSE` and `NOTICE`.
