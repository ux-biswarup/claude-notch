# Claude Notch for Windows — Product & Technical Plan

## 1. Overview

**Working name:** Claude Notch

> A tiny ambient interface that tells you when Claude is working, waiting, finished, or needs attention — without making you watch the terminal.

The Windows app will be inspired by the existing [Codenotch](https://github.com/vinzdg/codenotch) project, reusing its useful architectural ideas while implementing a Windows-native experience.

The final UX has two vertically stacked surfaces in the top-right corner of the display:

1. **Claude usage notch** — inspired by Codenotch, but showing Claude only.
2. **Four-light traffic signal** — an ambient status indicator for Claude Code.

The goal is not to replicate Codenotch pixel-for-pixel. The goal is to reuse its proven concepts and reduce implementation effort.

---

# 2. Product Problem

When using Claude Code inside VS Code, users frequently have to look back at the terminal to understand whether Claude is:

- still working,
- waiting for input,
- finished,
- or encountering an error.

This creates unnecessary attention switching.

### Product opportunity

Move the question:

> "Is Claude ready for me?"

out of the coding environment and into the user's peripheral vision.

The interface should be:

- ambient
- persistent
- glanceable
- non-intrusive
- fast to understand

---

# 3. Product Vision

Claude should be able to disappear from the user's attention while it works.

The Notch should only ask for attention when Claude needs it.

### Core value proposition

The two surfaces answer two different questions:

**Usage notch**

> How much Claude do I have?

**Traffic signal**

> Does Claude need me?

---

# 4. Goals

## Primary goal

Give the user immediate awareness of Claude Code's state without requiring them to open or inspect VS Code.

## Secondary goals

- Show Claude usage/limit information in a compact surface.
- Provide a clear visual language for Claude activity.
- Nudge the user when Claude needs attention.
- Quickly return the user to the relevant Claude Code session.
- Work without administrator privileges.
- Keep the application lightweight and unobtrusive.

---

# 5. Non-Goals — V1

Do not build these initially:

- Cursor integration
- Codex integration
- Gemini integration
- Multi-provider support
- Claude Desktop integration
- Full notification-management system
- AI conversation history
- Prompt management
- Authentication UI
- Multi-account management
- Cloud backend
- Telemetry or analytics by default

### V1 scope

**Claude Code on Windows.**

---

# 6. Final UX

## 6.1 Top Usage Notch

The top surface is inspired by Codenotch.

Conceptually:

```text
┌──────────────────────┐
│       Claude         │
│        ◯ 72%         │
└──────────────────────┘
```

It should expose, where reliably available:

- current Claude usage
- reset window/time
- account/session information where appropriate
- stale/error state

The UI should remain compact and visually quiet.

---

## 6.2 Four-Light Traffic Signal

Below the usage notch:

```text
       Claude
       ──────
        ◯ 72%

         ●
         ●
         ●
         ●
```

The four lights represent Claude Code's current state.

| Light | State | Meaning |
|---|---|---|
| Green | Working | Claude is actively doing work |
| Amber | Waiting | Claude is waiting for user input |
| White | Done | Claude completed the current task |
| Red | Error | Claude encountered a problem |

Only one state should be visually active at a time.

The traffic-light metaphor is an interaction language, not a literal traffic-light semantics.

---

# 7. State Machine

The state engine is the core of the product.

```text
                    ┌─────────────┐
                    │    IDLE     │
                    └──────┬──────┘
                           │
                      task starts
                           │
                           ▼
                    ┌─────────────┐
                    │   WORKING   │
                    │    GREEN    │
                    └──────┬──────┘
                           │
             ┌─────────────┴─────────────┐
             │                           │
             ▼                           ▼
       needs input                   completes
             │                           │
             ▼                           ▼
       ┌──────────┐                ┌──────────┐
       │ WAITING  │                │   DONE   │
       │  AMBER   │                │  WHITE   │
       └────┬─────┘                └──────────┘
            │
       user responds
            │
            ▼
         WORKING

WORKING ───────────────► ERROR
                         RED
```

### Canonical states

```text
IDLE
WORKING
WAITING
DONE
ERROR
```

The UI should consume these normalized states rather than knowing how Claude Code internally works.

---

# 8. Nudge Behavior

The signal should communicate through subtle motion as well as color/state.

## Working

- Green active light.
- Very subtle breathing/pulse.
- No notification.

Meaning:

> Claude is doing its thing. Don't interrupt.

## Waiting

- Amber active light.
- Slow periodic pulse.
- Optional subtle expansion/nudge.

Meaning:

> Claude needs your input.

This should be the strongest attention state.

## Done

- White active light.
- One short expansion/pulse.
- Returns to quiet resting state.

Meaning:

> Claude completed the task.

No persistent toast by default.

## Error

- Red active light.
- Short double pulse.
- Optional system notification depending on settings.

Meaning:

> Claude encountered a problem.

---

# 9. Interaction

Clicking the traffic signal should expose enough context to understand why attention is being requested.

Example:

```text
┌─────────────────────────┐
│ Claude                  │
│                         │
│ ● Waiting for you       │
│                         │
│ Open Claude Code    →   │
└─────────────────────────┘
```

When possible, clicking should:

1. Identify the relevant Claude Code session.
2. Identify the owning VS Code/terminal window.
3. Bring that window to the foreground.
4. Focus Claude Code if technically reliable.

If precise focus is not reliable in V1, simply bringing the owning window forward is acceptable.

---

# 10. Windows Constraints

The application must work without administrator privileges.

## V1 must be a user-space application

Avoid:

- Windows services
- kernel drivers
- machine-wide installation
- elevated processes
- installation into `Program Files`
- system-level hooks requiring elevation

The application should be capable of running from a user directory such as:

```text
%USERPROFILE%\Apps\ClaudeNotch\
```

A portable executable is preferred for the first version.

### Startup

Optional per-user startup may be supported later.

The product should not require administrator privileges for normal operation.

---

# 11. Recommended Technology

## Tauri + Rust + TypeScript

Recommended architecture:

```text
┌─────────────────────────────────────┐
│           Claude Notch              │
├─────────────────────────────────────┤
│                                     │
│  Windows Shell / Tauri              │
│                                     │
│  ┌───────────────────────────────┐  │
│  │       State Engine            │  │
│  └───────────────┬───────────────┘  │
│                  │                  │
│        ┌─────────┴─────────┐        │
│        ▼                   ▼        │
│  Claude Usage          Claude State │
│    Provider              Provider   │
│        │                   │        │
│        └─────────┬─────────┘        │
│                  ▼                  │
│             Usage / State           │
│                Store                │
│                  │                  │
│                  ▼                  │
│             UI Renderer             │
│                                     │
└─────────────────────────────────────┘
```

### Why Tauri

- Small desktop footprint.
- Rust gives us access to Windows APIs.
- TypeScript/HTML/CSS gives us fast UI iteration.
- Easy to create a polished custom overlay.
- Good fit for a designer/developer workflow.
- Suitable for a portable Windows application.

---

# 12. Provider Architecture

Preserve the abstraction used by Codenotch rather than coupling the UI directly to Claude.

Conceptually:

```text
Provider
   │
   ├── get_usage()
   ├── get_reset_time()
   ├── get_status()
   └── get_fidelity()
```

V1 implementation:

```text
ClaudeProvider
```

Future providers can later be added without changing the UI/state architecture:

```text
ClaudeProvider
CodexProvider
CursorProvider
GeminiProvider
```

---

# 13. Separate Usage from Activity

This is an important architectural improvement.

Do not make one provider responsible for every type of information.

## Claude Usage Provider

```text
ClaudeUsageProvider
        │
        ├── percentage
        ├── reset time
        └── account/session metadata
```

## Claude Activity Provider

```text
ClaudeActivityProvider
        │
        ├── idle
        ├── working
        ├── waiting
        ├── done
        └── error
```

Combined:

```text
                  Claude
                    │
             ┌──────┴──────┐
             │             │
           Usage        Activity
             │             │
             ▼             ▼
           72%          WORKING
```

This separation makes future provider changes significantly easier.

---

# 14. Claude State Engine

Create a central state machine:

```text
ClaudeStateMachine
```

It receives observations/events such as:

```text
PROCESS_STARTED
OUTPUT_RECEIVED
TOOL_RUNNING
WAITING_FOR_INPUT
PROCESS_EXITED
ERROR_DETECTED
```

and normalizes them into:

```text
WORKING
WAITING
DONE
ERROR
IDLE
```

The UI should only know:

```text
ClaudeState = Working
```

It should not care whether the state came from a process, hook, local event, or another implementation detail.

---

# 15. UI Architecture

Recommended UI components:

```text
App
│
├── UsageNotch
├── TrafficSignal
├── Nudge
└── Settings
```

## UsageNotch

Displays:

```text
Claude
◯ 72%
```

## TrafficSignal

Displays four state lights:

```text
●
●
●
●
```

## Nudge

Owns temporary animation and attention behavior.

## Settings

Keep V1 minimal:

```text
Claude Notch

Appearance
  ○ Always visible
  ○ Edge trigger

Notifications
  ☑ Task completed
  ☑ Needs input
  ☑ Errors

Startup
  ☑ Launch on sign-in

Position
  ● Top-right
```

---

# 16. What to Reuse From Codenotch

The existing Codenotch repository is valuable primarily as an architectural and behavioral reference.

Repository:

https://github.com/vinzdg/codenotch

## Reuse conceptually

### 1. Provider abstraction

Codenotch already separates usage retrieval behind a provider concept.

Use the same principle for:

```text
ClaudeUsageProvider
```

### 2. Usage store

Reuse the idea of a central store that polls providers and preserves the last good reading.

### 3. Fidelity

Preserve the distinction between:

```text
official
derived
manual
```

This prevents the UI from presenting uncertain data as authoritative.

### 4. Error degradation

Do not display an apparently valid usage percentage when the underlying data is stale.

Prefer:

```text
72% · stale
```

or:

```text
Usage unavailable
```

### 5. Polling and backoff

Reuse the principle of polling with sensible backoff and rate-limit handling.

### 6. Demo mode

Keep the demo-mode concept.

The new demo mode should allow the entire UX to be tested without Claude running:

```text
Working
↓
Waiting
↓
Done
↓
Error
```

This will make UI/animation development much faster.

---

# 17. What NOT to Port

Do not attempt a literal Swift/macOS-to-Windows conversion.

Avoid carrying over macOS-specific implementation details such as:

- SwiftUI
- NSWindow
- NSStatusItem
- macOS Keychain integration
- macOS notch geometry
- Dock integration
- macOS-specific window management

Instead:

> Port the behavioral and architectural concepts, not the platform implementation.

---

# 18. Proposed Repository Structure

```text
claude-notch/
│
├── src/
│   ├── ui/
│   │   ├── UsageNotch/
│   │   ├── TrafficSignal/
│   │   ├── Nudge/
│   │   └── Settings/
│   │
│   ├── state/
│   │   ├── ClaudeStateMachine.ts
│   │   └── State.ts
│   │
│   └── app/
│
├── src-tauri/
│   ├── providers/
│   │   ├── claude_usage.rs
│   │   └── claude_activity.rs
│   │
│   ├── state/
│   │   └── state_machine.rs
│   │
│   ├── windows/
│   │   ├── overlay.rs
│   │   ├── focus.rs
│   │   └── startup.rs
│   │
│   └── main.rs
│
├── tests/
│
├── docs/
│   ├── PRD.md
│   ├── architecture.md
│   └── ux.md
│
└── README.md
```

---

# 19. Build Strategy

Do not build all functionality simultaneously.

## Phase 1 — Windows Shell

Build:

```text
.exe
 ↓
always-on-top
 ↓
top-right
 ↓
transparent
 ↓
no admin
```

Success criterion:

The app can sit unobtrusively over any application.

---

## Phase 2 — Traffic Signal

Initially hardcode the four states:

```text
GREEN
AMBER
WHITE
RED
```

Build and perfect:

- sizing
- spacing
- placement
- animation
- transitions
- nudge behavior
- reduced-motion behavior

This lets UX development happen independently from Claude integration.

---

## Phase 3 — Claude Activity Detection

Connect:

```text
Claude Code
      ↓
Activity Provider
      ↓
State Machine
      ↓
Traffic Signal
```

Investigate the most reliable supported Claude Code signals/events available at implementation time.

Do not rely on undocumented behavior until it has been validated.

---

## Phase 4 — Usage

Port the Codenotch usage concept:

```text
Claude
◯ 72%
```

Add:

- usage retrieval
- reset information
- stale handling
- error states
- fidelity metadata

---

## Phase 5 — Focus Behavior

Implement:

```text
Signal
   ↓
Claude Code
   ↓
VS Code foreground
```

Start with reliable foregrounding before attempting precise terminal/input focus.

---

## Phase 6 — Persistence

Add:

- startup preference
- position
- animation preferences
- notification preferences

---

## Phase 7 — Packaging

Produce:

```text
ClaudeNotch.exe
```

with a portable/user-space distribution option.

---

# 20. MVP 0.1

The first useful version should look conceptually like:

```text
┌───────────────────────┐
│       Claude          │
│        ◯ 72%          │
└───────────────────────┘
          │
          ▼
          ●   ← GREEN
          ●
          ●
          ●
```

## Functional requirements

- [ ] Runs without administrator privileges
- [ ] Always-on-top
- [ ] Anchored to top-right
- [ ] Claude usage displayed
- [ ] Claude Code activity detected
- [ ] Working state
- [ ] Waiting state
- [ ] Done state
- [ ] Error state
- [ ] Animated state transitions
- [ ] Completion nudge
- [ ] Waiting-for-input nudge
- [ ] Click → focus Claude Code
- [ ] Graceful unavailable/stale state
- [ ] Demo mode
- [ ] No credentials stored unnecessarily by the app

---

# 21. Technical Risks

The highest-risk area is **not the UI**.

It is reliable Claude Code state detection.

We need to establish exactly how to distinguish:

```text
Claude is working
        vs
Claude is waiting
        vs
Claude is finished
        vs
Claude errored
```

The implementation should first investigate supported Claude Code hooks/events and Windows process/window behavior.

### Risk hierarchy

| Risk | Impact | Priority |
|---|---:|---:|
| Claude state detection | High | P0 |
| Usage retrieval | High | P0 |
| Window focus | Medium | P1 |
| Always-on-top overlay | Medium | P1 |
| Animation | Low | P2 |
| Settings | Low | P2 |
| Packaging | Medium | P1 |

---

# 22. Design Principles

### 1. Ambient, not intrusive

The app should occupy very little visual attention.

### 2. State over text

The user should understand Claude's status without reading a sentence.

### 3. Motion is information

Animation should communicate state transitions, not decorate the UI.

### 4. Ask for attention only when necessary

Working should be quiet.

Waiting should attract attention.

Done should provide a short acknowledgement.

Error should be clear.

### 5. Never pretend certainty

If usage or state is unavailable or stale, communicate that explicitly.

### 6. Separate observation from presentation

Claude detection belongs in providers/state management.

The UI should consume normalized state.

### 7. Design for the periphery

The user should be able to understand the signal without moving their focus away from their primary work.

---

# 23. Product Statement

The product can be summarized as:

> **Claude should disappear from your attention while it works. The Notch only asks for your attention when Claude needs it.**

The usage surface answers:

> **How much Claude do I have?**

The traffic signal answers:

> **Does Claude need me?**

That distinction is the foundation of the product.

---

# 24. Recommended Next Step

Before writing the Windows implementation, perform a technical reconnaissance of the Codenotch repository and Claude Code's current Windows capabilities.

Specifically map:

```text
Codenotch file/class
        ↓
Windows/Tauri equivalent
```

and validate:

```text
Claude Code
    ↓
available hooks/events/process signals
    ↓
WORKING / WAITING / DONE / ERROR
```

Once this is confirmed, implementation can proceed incrementally rather than discovering architectural problems halfway through the build.
