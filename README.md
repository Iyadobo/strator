# Strator

The home your coding agents live in. Type `strator`, get a dashboard:
launch an agent, watch usage and progress, change the theme — then drop
into a full terminal multiplexer where every agent runs in a real pane.

Strator is a fork of [Herdr](https://github.com/herdrdev/herdr) (Apache-2.0),
rebranded and extended with a multi-window home, usage/progress tracking,
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

No tabs. One screen, several small windows side by side — a union of
terminal windows and widgets, all visible at once:

- **Launch** (top left) — open a harness: CLI default (`Enter`) or
  BYOK model provider key (`b`); provider-first catalog launch (`l`).
- **Usage** (bottom left) — launch history: counts by provider,
  harness, model, project. Scrollable.
- **Progress** (bottom right) — live agents: working / blocked / done,
  with a done bar. Scrollable.
- **Themes + backgrounds** (top right) — click a theme or background
  and it applies live to the engine config.

Mouse-first: click a window to focus it, click a row to select,
double-click (or `Enter`) to run/apply, wheel scrolls the window
under the cursor. Keyboard still works everywhere: `Tab` moves
between windows, arrows move, `r` refreshes, `q` quits.
Inside the multiplexer itself, panes, tabs, and workspaces are
already fully mouse-operated upstream.

## Open a harness: default first, BYOK second

Default is the harness's own setup. BYOK is the secondary opt-in.

```sh
strator open                     # DEFAULT: installed harness, its own login,
                                 # default model, native model picker
strator byok --harness codex     # BYOK: your OpenAI (model provider) key
strator byok --harness claude --provider anthropic
strator byok                     # BYOK: pick harness, then provider
strator open --harness claude --model opus   # explicit override
```

CLI-default mode passes **no** `--model`: the harness keeps its own
login, default model, and in-CLI picker. In the dashboard, `Enter` is
CLI-default, `b` is BYOK, `l` is the advanced provider-first catalog
launch (`strator launch`).

BYOK means a **model provider key** (OpenAI, Anthropic, OpenRouter, …) —
not a harness key. Keys come from the environment, the OS credential
store, or masked entry. **Every resolved key is saved** to the OS
credential store, scoped to provider + endpoint only, so one saved
OpenAI key serves Codex, OpenCode, Pi, and every other harness
launched on OpenAI. The next BYOK launch on that provider needs
no key at all. Manage them with
`strator keys save|status|forget --provider ID`. Never a plain file.
There is no `--api-key` flag.

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
forest, plum, mono, paper, none). Apply from the Themes window or:

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
