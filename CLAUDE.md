# Claude Notch — notes for Claude Code sessions

Ambient Windows overlay (Tauri 2 + Rust + vanilla TS) showing Claude Code's
state and Claude usage. Product plan: `docs/PRD.md`. Implementation map and the
list of things still to validate: `docs/architecture.md`. Visual spec: `docs/ux.md`.

## Commands

```sh
pnpm dev          # UI in the browser with the demo script (no Rust needed)
pnpm typecheck    # tsc --noEmit
pnpm test         # vitest (tests/)
pnpm build        # vite → dist/
pnpm core:test    # cargo test -p claude-notch-core (pure Rust crate)
pnpm app:dev      # tauri dev — needs MSVC Build Tools (link.exe)
cargo fmt --manifest-path src-tauri/Cargo.toml --all
```

If `link.exe` is missing, use the GNU toolchain for the core crate:
`cargo +stable-x86_64-pc-windows-gnu test --manifest-path src-tauri/Cargo.toml -p claude-notch-core`.
The Tauri app crate cannot be compiled without MSVC; say so rather than guessing it builds.

## Architecture rules

- **UI consumes normalised state only.** Claude Code specifics (hook names,
  payload fields) stay in `src-tauri/core/src/hooks.rs`. The UI sees
  `idle | working | waiting | done | error` and nothing else.
- **Core stays pure.** `src-tauri/core` must not depend on Tauri, tokio, reqwest
  or the `windows` crate. I/O and windowing belong in `src-tauri/src`.
- **One contract, three mirrors.** Changing a snapshot/settings field means
  updating `src/state/State.ts`, the Rust structs (`core::snapshot`,
  `core::state::store`, `core::usage`, `src/settings.rs`) and
  `docs/architecture.md §7` together.
- **Never pretend certainty.** Usage readings carry `status` and `fidelity`;
  stale data is shown as stale, missing data as unavailable.
- **No secrets.** The Claude Code access token is read per request and dropped.
  Never log it, persist it, or add it to a snapshot.
- **Demo script parity.** `core::demo::script()` and `DEMO_SCRIPT` in
  `src/app/demo.ts` must describe the same sequence.
- Tauri event names: `notch:snapshot`, `notch:open-settings`. Hook listener:
  `127.0.0.1:<hookPort>/hook` (default 47831), loopback only.

## Style

- Rust 2024 edition, `cargo fmt` defaults; wrap unsafe ops in `unsafe {}` blocks
  even inside `unsafe fn`. The `windows` crate is referenced as `::windows::…`
  inside the `windows` module.
- TypeScript strict, no framework; components are small classes that own a DOM
  element and expose `update(...)`. Pure logic goes in separate modules
  (`format.ts`, `describe.ts`, `ClaudeStateMachine.ts`) so it can be tested in Node.
