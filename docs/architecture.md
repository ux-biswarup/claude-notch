# Claude Notch — Architecture

Implementation map for the product described in [PRD.md](PRD.md). The PRD is the
source of truth for *what* and *why*; this document records *how* the code base
is organised, the contracts between its parts, and what still has to be validated.
The visual/motion specification lives in [ux.md](ux.md).

## 1. Stack and constraints

| Concern | Decision |
|---|---|
| Shell | Tauri 2 on WebView2 (ships with Windows 10/11) |
| Backend | Rust, 2024 edition, stable toolchain |
| Front end | Vanilla TypeScript + Vite 8, no framework — the whole UI is ~20 KB |
| Tests | `cargo test` (core crate), Vitest (presentation logic) |
| Distribution | Single `claude-notch.exe` (portable) or NSIS installer in `currentUser` mode |
| Privileges | User space only (PRD §10): no services, drivers, elevation or machine-wide installs |

**Build-toolchain note.** Compiling the Tauri shell on Windows needs the MSVC
linker from Visual Studio Build Tools ("Desktop development with C++"), whose
installer requires administrator rights. The `core` crate deliberately has no
Tauri or Win32 dependency, so it builds and tests with the GNU toolchain that
rustup installs without admin:

```
cargo +stable-x86_64-pc-windows-gnu test --manifest-path src-tauri/Cargo.toml -p claude-notch-core
```

**Endpoint-protection note.** On the machine this scaffold was created on,
Defender's ASR rule `01443614-CD74-433A-B99E-2ECDC07BFC25` ("block executable
files unless prevalence/age/trusted") is enforced by IT and blocked *every*
locally linked executable, including Cargo build scripts — so neither
`cargo test` nor `cargo check` can complete there without an exclusion for
`src-tauri\target\`. Dependency resolution (`Cargo.lock`) and `cargo fmt` work,
and both Rust crates parse cleanly, but the Rust code has **not been compiled**
yet; expect a round of compiler-driven fixes when it first builds (see §12.4).

## 2. Module map

```
src/                          UI (TypeScript)
├── app/
│   ├── main.ts               bootstrap; picks the backend
│   ├── App.ts                composes UsageNotch + TrafficSignal + Nudge + Settings
│   ├── ipc.ts                Backend interface; Tauri implementation (invoke/listen)
│   └── demo.ts               browser-only backend running the demo script (no Rust needed)
├── state/
│   ├── State.ts              the IPC data contract (mirrors core::snapshot & friends)
│   └── ClaudeStateMachine.ts presentation reducer: state transition → motion + one-shot effect
└── ui/
    ├── UsageNotch/           "How much Claude do I have?"  (+ format.ts: stale/unavailable rules)
    ├── TrafficSignal/        "Does Claude need me?"        (+ describe.ts: click-card copy)
    ├── Nudge/                owns all animation; reduced-motion handling
    └── Settings/             minimal V1 settings panel

src-tauri/                    Cargo workspace (root package = the Tauri app)
├── core/                     claude-notch-core — pure Rust, no Tauri, no Win32
│   └── src/
│       ├── state/state_machine.rs   ClaudeState, ActivityEvent, transition()
│       ├── state/store.rs           ActivityStore: per-session state, aggregation, pruning
│       ├── hooks.rs                 Claude Code hook payload → Observation; settings.json (un)install
│       ├── usage.rs                 credentials + usage response parsing, staleness, PollPolicy
│       ├── demo.rs                  scripted session + demo usage reading
│       ├── http.rs                  tiny HTTP/1.1 request parser for the hook listener
│       ├── snapshot.rs              Snapshot / Fidelity / ActivitySource (serde, camelCase)
│       └── time.rs                  now_ms()
└── src/                      the Tauri app (claude_notch_lib)
    ├── main.rs, lib.rs       builder, plugins, setup
    ├── commands.rs           #[tauri::command]s used by ipc.ts
    ├── state/mod.rs          AppState: store + usage + demo flag + settings; emits notch:snapshot
    ├── settings.rs           Settings struct, load/save (%APPDATA%\com.claude-notch.desktop)
    ├── tray.rs               tray menu: demo, connect hooks, settings, snap, quit
    ├── providers/
    │   ├── claude_activity.rs  loopback listener (127.0.0.1:47831/hook) + hook installer
    │   ├── claude_usage.rs     reqwest polling loop
    │   └── demo.rs             demo runner
    └── windows/
        ├── overlay.rs        top-right placement, content-sized transparent window
        ├── focus.rs          EnumWindows + SetForegroundWindow ("Open Claude Code")
        └── startup.rs        launch on sign-in (HKCU Run via tauri-plugin-autostart)
```

The PRD's proposed layout (§18) is followed with one addition: the `core`
crate. Everything that can be reasoned about without a window lives there and is
unit tested; the app crate only does I/O, windowing and wiring (PRD §22.6,
"separate observation from presentation").

## 3. Data flow

```
Claude Code ──hook command (curl)──► POST 127.0.0.1:47831/hook ──► core::hooks::observation_from_hook
                                                                            │
                     providers::demo (scripted) ──────────────► AppState.apply_observation
                                                                            │
                                                     core::state::ActivityStore  (one state per session)
                                                                            │  aggregate: waiting > error > working > done > idle
                                                     AppState.snapshot() ── emit "notch:snapshot" ──► ipc.ts ──► App.apply
                                                                                                                │
                                                                          reducePresentation(prev, state) ──────┤
                                                                          (motion + one-shot effect)            ▼
                                                                                                 TrafficSignal + Nudge

%USERPROFILE%\.claude\.credentials.json ──► providers::claude_usage ──► core::usage::parse_usage_response ──► UsageSnapshot ──► UsageNotch
```

## 4. Activity state machine (PRD §7, §14)

`core::state::transition(current, event)`:

| Event | From | To | Note |
|---|---|---|---|
| `process_started` | any | **idle** | new / resumed / cleared session |
| `prompt_submitted` | any | **working** | |
| `tool_running` | any | **working** | |
| `output_received` | any | **working** | includes "user answered the permission prompt" |
| `waiting_for_input` | any | **waiting** | |
| `task_completed` | error | **error** | error is sticky until fresh activity |
| `task_completed` | other | **done** | |
| `error_detected` | any | **error** | |
| `process_exited` | any | **idle** | session record is removed |

**Sessions.** `ActivityStore` keeps one record per Claude Code `session_id`
(state, cwd, last event, detail). The UI shows the *aggregate*: the session with
the highest priority (`waiting` 4 > `error` 3 > `working` 2 > `done` 1 > `idle` 0),
most recent on ties. That session is the `focusSessionId` and the target of
"Open Claude Code". Sessions silent for 12 h are pruned (a crashed Claude Code
never sends `SessionEnd`).

## 5. Claude Code signal mapping (PRD §19 phase 3, §21)

Claude Code exposes lifecycle **hooks**: shell commands run at defined moments
with a JSON payload on stdin (`session_id`, `hook_event_name`, `cwd`, …). This is
the documented, supported signal source; nothing is inferred from process or
window behaviour.

| Hook event | → ActivityEvent | Notes |
|---|---|---|
| `SessionStart` (source ≠ compact) | `process_started` | |
| `SessionStart` (source = compact) | `output_received` | compaction happens mid-turn; must not reset to idle |
| `UserPromptSubmit` | `prompt_submitted` | |
| `PreToolUse` | `tool_running` | detail: "Running <tool>" |
| `PostToolUse`, `PostToolUseFailure` | `output_received` | a failing tool is routine for Claude, not a product error |
| `PermissionRequest` | `waiting_for_input` | detail: "Needs permission: <tool>" |
| `Notification` permission_prompt / elicitation_dialog | `waiting_for_input` | |
| `Notification` idle_prompt / auth_success | *(ignored)* | idle_prompt fires ~60 s after a turn ended; Done already says that |
| `Stop` | `task_completed` | |
| `StopFailure` | `error_detected` | detail: the error text |
| `SessionEnd` | `process_exited` | |
| anything else | *(ignored)* | |

**Hook command.** One command for every subscribed event, installed into
`~/.claude/settings.json` by "Connect Claude Code" (tray or settings panel):

```sh
curl -s -m 2 -X POST -H "Content-Type: application/json" --data-binary @- http://127.0.0.1:47831/hook >/dev/null 2>&1 || true # claude-notch
```

`curl.exe` ships with Windows 10+ and Git for Windows, so no helper binary is
needed; `|| true` guarantees the hook never fails a Claude turn when Claude Notch
is not running. The trailing `# claude-notch` marker lets the installer find and
replace its own entries. Installation is a *merge*: other hooks and all other
settings are preserved (key order too, via `serde_json/preserve_order`), the
previous file is copied to `settings.json.claude-notch.bak`, and re-installing is
idempotent. `core::hooks::remove_hooks` reverses it exactly.

**Listener.** `providers::claude_activity` binds `127.0.0.1:<hookPort>` (default
47831, loopback only) and speaks just enough HTTP/1.1 for curl (`core::http`).
`POST /hook` → `204`; `GET /health` → `200 ok`.

## 6. Usage provider (PRD §13, §16, phase 4)

| Aspect | Implementation |
|---|---|
| Token | Read per request from Claude Code's `%USERPROFILE%\.claude\.credentials.json` (`claudeAiOauth.accessToken`); honours `CLAUDE_CONFIG_DIR`. Never logged or persisted; `AccessToken`'s `Debug` redacts it. |
| Endpoint | `GET https://api.anthropic.com/api/oauth/usage` with `Authorization: Bearer …` and `anthropic-beta: oauth-2025-04-20` (same as Codenotch and similar tools) |
| Response | Object of windows: `five_hour`, `seven_day`, `seven_day_opus`, … each `{ utilization: 0–100, resets_at: RFC 3339 }`. Parser is lenient; primary window = first known one present. |
| Fidelity | `official` for endpoint data, `manual` for demo. The UI shows fidelity in the tooltip. |
| Polling | `PollPolicy`: base 120 s, exponential backoff ×2 per consecutive failure capped at 15 min; `Retry-After` honoured on 429; 401/403 waits the maximum (Claude Code refreshes the token when next used). |
| Staleness | Evaluated at read time: a good reading older than 3× base is `stale` → "72% · stale". A failed fetch keeps the last number as stale and records the error; with no number at all → "Usage unavailable". |
| Account label | Derived from `subscriptionType` ("Max", "Pro") — never an e-mail. |

## 7. IPC contract

Types: `src/state/State.ts` ⇄ `core::snapshot`, `core::state::store`,
`core::usage`, `src-tauri/src/settings.rs`. Rust serialises `camelCase` fields
and lowercase / snake_case enums.

| Command | Args | Returns |
|---|---|---|
| `get_snapshot` | — | `Snapshot` |
| `set_demo_mode` | `enabled: bool` | `Snapshot` |
| `focus_claude` | `sessionId?: string` | `boolean` (false = window found but Windows refused focus) |
| `get_settings` / `set_settings` | — / `settings: Settings` | `Settings` |
| `install_hooks` / `hooks_status` | — | `HooksStatus` |
| `reposition` | — | — |
| `resize_to_content` | `width, height` (logical px) | — |

| Event | Payload | When |
|---|---|---|
| `notch:snapshot` | `Snapshot` | any observation, usage reading, or demo toggle |
| `notch:open-settings` | — | tray "Settings…" |

## 8. Windows integration (PRD §10)

- **Overlay** (`tauri.conf.json` + `windows/overlay.rs`): undecorated, transparent,
  no shadow, always on top, skip taskbar, created hidden and unfocused, then placed
  at the top-right of the primary monitor's *work area* (never over the taskbar)
  and shown. The UI reports its rendered size (`resize_to_content`) so the
  transparent window is no larger than the visible surfaces and blocks as few
  clicks as possible.
- **Tray** (`tray.rs`): Demo mode ✓, Connect Claude Code…, Settings…, Snap to
  top-right, Quit. A second launch just reveals the overlay (`tauri-plugin-single-instance`).
- **Focus** (`windows/focus.rs`): the session's `cwd` folder name is matched
  against visible top-level window titles (VS Code preferred, then terminals);
  `SetForegroundWindow`, restoring minimised windows first and using the
  synthetic-Alt workaround if Windows refuses.
- **Startup** (`windows/startup.rs`): per-user `HKCU\…\Run` entry via
  `tauri-plugin-autostart`, driven by the "Launch on sign-in" setting.
- **Settings**: JSON at `%APPDATA%\com.claude-notch.desktop\settings.json`.
- **Notifications**: `tauri-plugin-notification`; defaults — Waiting ✓, Errors ✓,
  Done ✗ (PRD §8: no persistent toast for Done).

## 9. Demo mode (PRD §16.6)

Two implementations of the same script (`core::demo::script()` ⇄ `DEMO_SCRIPT`
in `src/app/demo.ts`): Session started → Working → Running Bash → **Waiting**
(permission) → Working → **Done** → Working → **Error** → Session ended → loop.

- In the app (tray → Demo mode) the script feeds *real* observations through
  `ActivityStore`, so it tests the whole pipeline. Hook observations are dropped
  while demoing; demo observations are dropped once demo is off (source check +
  generation token).
- In a browser (`pnpm dev`) the UI runs against `BrowserDemoBackend` — no Rust,
  no Claude Code. `→`/Space steps, `d` toggles demo, `s` opens settings.

## 10. Codenotch → Claude Notch (PRD §16, §24)

| Codenotch concept | Claude Notch equivalent |
|---|---|
| Provider abstraction | `providers::claude_activity` / `providers::claude_usage` (separate, PRD §13) |
| Usage store with last-good reading | `AppState` + `UsageSnapshot::degraded` / `with_freshness` |
| Fidelity (official / derived / manual) | `core::snapshot::Fidelity`, surfaced in the UI tooltip |
| Error degradation ("stale") | `UsageStatus::{Ok, Stale, Unavailable}` + `format.ts` |
| Polling with backoff | `core::usage::PollPolicy` |
| Demo mode | `core::demo` + `src/app/demo.ts` |
| Menu bar / notch UI (SwiftUI, NSStatusItem) | Tauri transparent overlay + tray icon (not ported literally, PRD §17) |
| macOS Keychain | Not needed: Claude Code's own credentials file is read in memory |

## 11. Phase status (PRD §19)

| Phase | Status |
|---|---|
| 1 Windows shell | Implemented (config + `overlay.rs`); **not yet compiled** — needs MSVC Build Tools |
| 2 Traffic signal | Implemented and viewable in the browser preview; motion spec in ux.md |
| 3 Activity detection | Implemented (hooks → listener → store); mapping covered by unit tests (not yet run — see §1); **end-to-end unvalidated** |
| 4 Usage | Implemented; parser covered by unit tests (not yet run — see §1); **endpoint shape unvalidated** |
| 5 Focus | Implemented (title match + SetForegroundWindow); untested on real windows |
| 6 Persistence | Settings file, autostart, notification prefs implemented |
| 7 Packaging | NSIS `currentUser` configured; portable exe is the plain release binary |

## 12. Validation list / open questions

1. **Hook event names and payload fields** — confirm against the Claude Code hooks
   reference for the installed version (2.1.x): `PermissionRequest`,
   `StopFailure`, `notification_type` values, `SessionStart.source`.
2. **Hook execution shell on Windows** — the command assumes a POSIX shell (Git
   Bash) for `>/dev/null 2>&1 || true` and the `#` marker. If hooks run under
   `cmd.exe`, switch to a small `.cmd` shim.
3. **Usage endpoint** — response shape, rate limits, and whether `utilization` is
   0–100 (assumed) or 0–1.
4. **Tauri API details** that only a compile will confirm: `Monitor::work_area`,
   tray builder method names, plugin init signatures.
5. **Foregrounding** — `SetForegroundWindow` restrictions and title matching when
   several windows share a folder name.
6. **Placement vs. maximised windows** — a top-right overlay covers the close
   button of maximised apps. PRD's "Edge trigger" appearance option is the
   planned mitigation.
