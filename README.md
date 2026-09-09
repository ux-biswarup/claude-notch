# Claude Notch

> A tiny ambient interface for Windows that tells you when Claude Code is
> working, waiting, finished, or needs attention — without making you watch the
> terminal.

Two surfaces sit in the top-right corner of your screen:

```
┌───────────────┐
│    CLAUDE     │   How much Claude do I have?
│  ◯  72%       │
└───────────────┘
      ●            Does Claude need me?
      ●            green working · amber waiting · white done · red error
      ●
      ●
```

Inspired by [Codenotch](https://github.com/vinzdg/codenotch); built with Tauri 2,
Rust and TypeScript. Runs entirely in user space — no admin rights, no services.

**Status:** scaffold complete, pre-alpha. See
[docs/architecture.md §11](docs/architecture.md#11-phase-status-prd-19) for
per-phase status and [docs/PRD.md](docs/PRD.md) for the product plan.

## Documentation

- [docs/PRD.md](docs/PRD.md) — product requirements and technical plan
- [docs/architecture.md](docs/architecture.md) — module map, state machine, Claude Code hook mapping, IPC contract, validation list
- [docs/ux.md](docs/ux.md) — layout, colours, motion and copy

## Prerequisites

| Tool | Needed for | Notes |
|---|---|---|
| Node 20+ and pnpm | UI, tests, browser preview | `corepack enable` or `npm i -g pnpm` |
| Rust stable (rustup) | backend | per-user install, no admin |
| Visual Studio Build Tools — "Desktop development with C++" | building the Tauri app | **requires admin** to install: `winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| WebView2 runtime | running the app | preinstalled on Windows 10/11 |

Without the Build Tools you can still do everything except compile the Tauri
shell: the UI runs in a browser and the Rust core crate builds with the GNU
toolchain (`rustup toolchain install stable-x86_64-pc-windows-gnu`).

**Managed (corporate) machines.** If Windows Defender's Attack Surface Reduction
rule *"Block executable files from running unless they meet a prevalence, age,
or trusted list criterion"* (`01443614-CD74-433A-B99E-2ECDC07BFC25`) is enforced,
every freshly compiled executable — Cargo build scripts, test binaries, the app
itself — is blocked with "Access is denied" (Defender event 1121). Rust
development then needs an IT exception: an ASR-only exclusion for the project's
`src-tauri\target\` directory and `%USERPROFILE%\.cargo\`, or the rule switched
to audit mode for this device.

## Getting started

```sh
pnpm install

# UI in the browser, running the demo script (no Rust, no Claude Code needed)
pnpm dev                 # → http://localhost:1420  (→ / Space step, d demo, s settings)

# Checks
pnpm typecheck
pnpm test                # Vitest: presentation reducer, usage formatting, click-card copy
pnpm build               # production bundle → dist/
pnpm core:test           # Rust core: state machine, hook mapping, usage parsing, http
#   without MSVC:  cargo +stable-x86_64-pc-windows-gnu test --manifest-path src-tauri/Cargo.toml -p claude-notch-core

# The real app (needs MSVC Build Tools)
pnpm app:dev             # tauri dev — overlay + tray
pnpm app:build           # tauri build → src-tauri/target/release/claude-notch.exe (+ NSIS installer)
```

## Connecting Claude Code

Claude Notch listens for Claude Code's lifecycle **hooks** on
`http://127.0.0.1:47831/hook`. Use **tray → Connect Claude Code…** (or the
settings panel) to merge the hooks into `%USERPROFILE%\.claude\settings.json`;
your existing hooks and settings are preserved and the previous file is backed up
next to it. New Claude Code sessions then report their state. Nothing leaves your
machine.

Usage is read with Claude Code's own sign-in (its credentials file is read into
memory per request and never copied). If that file is missing the notch shows
"Usage unavailable" rather than guessing.

## Repository layout

```
src/            UI: app/ (bootstrap, IPC, demo), state/ (contract, presentation reducer), ui/ (components)
src-tauri/      Tauri app (Rust). src-tauri/core is the pure, unit-tested core crate.
tests/          Vitest suites
docs/           PRD, architecture, UX
scripts/        make-icon.mjs — regenerates the icon set (pnpm icon)
```

## License

MIT — see [LICENSE](LICENSE).
