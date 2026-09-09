# Claude Notch — UX Specification

Visual and motion specification for the two surfaces described in
[PRD.md](PRD.md) §6, §8, §9, §15 and §22. Values here are the ones implemented in
`src/styles/base.css` and the component stylesheets; change both together.

## 1. Layout

```
                                    ┌───────────────┐
                                    │    CLAUDE     │   UsageNotch  (min 108 × ~52 px)
                                    │  ◯  72%       │
                                    │     Resets in 2h 13m
                                    └───────────────┘
                                          ● ● ● ●   ← 8 px gap
                                          ┌───┐
                                          │ ● │       TrafficSignal (12 px lights, 9 px gap,
                                          │ ● │        pill radius 999, padding 11 × 9)
                                          │ ● │
                                          │ ● │
                                          └───┘
```

- Anchored to the **top-right** of the primary monitor's work area with an 8 px
  (logical) margin. The native window is resized to the rendered content plus a
  12 px halo for glows, so the transparent area is minimal.
- Surfaces share the `.surface` style: `rgba(20,20,24,.92)` background, 1 px
  `rgba(255,255,255,.08)` border, 16 px radius, soft shadow, 12 px backdrop blur.
- Typeface: Segoe UI Variable Text → Segoe UI → system-ui. Base size 12 px.

## 2. Colour tokens

| Token | Value | Use |
|---|---|---|
| `--light-working` | `#34d399` | green light |
| `--light-waiting` | `#f5a524` | amber light |
| `--light-done` | `#f4f4f5` | white light |
| `--light-error` | `#f43f5e` | red light |
| `--light-off` | `rgba(255,255,255,.12)` | unlit lights |
| `--usage-ok / warn / high / stale` | green / amber / red / grey | usage ring by tone |
| `--fg` / `--muted` | `#ececef` / `#9a9aa3` | text |
| `--accent` | `#d97757` | focus ring only |

Exactly one light is lit at a time; it also gets a 10 px glow in its own colour.
Idle = all four unlit.

## 3. Usage notch

| Reading | Primary | Secondary | Tone |
|---|---|---|---|
| 0–69 % | `12%` | `Resets in 2h 13m` | ok (green ring) |
| 70–89 % | `72%` | … | warn (amber ring) |
| 90–100 % | `95%` | … | high (red ring) |
| stale (older than 3× poll interval, or last fetch failed) | `72% · stale` | … | stale (grey ring, muted number) |
| unavailable | `—` | `Usage unavailable` | off (no ring) |

Tooltip: `5h window · official · Max` plus `last reading 20m ago` when stale and
the last error when there is one. Fidelity is always shown here (PRD §22.5).

## 4. Traffic signal states and motion (PRD §8)

| State | Light | Continuous motion | On entry | Meaning |
|---|---|---|---|---|
| Working | green | `breathe` — scale 1→1.12, opacity .85→1, 3.2 s ease-in-out, infinite | — | "Doing its thing. Don't interrupt." |
| Waiting | amber | `pulse-slow` — scale 1→1.25 + glow 8→18 px, 2.4 s, infinite | `nudge-expand` — whole pill scales to 1.08 and back, 0.6 s | strongest attention state |
| Done | white | none | `pop` — light scales to 1.6 with a wide glow, back to 1, 0.7 s, once | short acknowledgement, then rests |
| Error | red | none | `double-pulse` — two 1.5× pulses in 1 s, once | clear, then steady |
| Idle | none | none | — | nothing to report |

Rules:

- Effects fire only on *entering* a state (`ClaudeStateMachine.ts`), never while
  staying in it, and never on the initial render.
- Reduced motion: follows `prefers-reduced-motion` unless the setting overrides
  it (`system` / `on` / `off`). When reduced, all animations and transitions are
  disabled; colour alone carries the state.
- Colour transitions between lights take 350 ms.

## 5. Interaction (PRD §9)

Clicking the signal toggles a card beneath it (188 px wide):

```
┌────────────────────────┐
│ CLAUDE                 │
│ ● Waiting for you      │
│ Needs permission: Bash · claude-notch
│ ┌────────────────────┐ │
│ │ Open Claude Code → │ │
│ └────────────────────┘ │
└────────────────────────┘
```

- Title line: state label — `Working`, `Waiting for you`, `Done`,
  `Something went wrong`, `Idle`.
- Meta line: reason · workspace folder · `+N other sessions`; falls back to
  `No Claude Code session detected` / `No active session` / `Demo mode`.
- "Open Claude Code →" brings the owning VS Code/terminal window to the front.
  Disabled when no session is known; a transient note explains failures.
- `Esc` closes the card.

## 6. Settings (PRD §15)

Shown inside the overlay from the tray ("Settings…"). Copy, in order:

```
Claude Notch                                  ×
APPEARANCE      ○ Always visible   ○ Edge trigger (later)
NOTIFICATIONS   ☐ Task completed   ☑ Needs input   ☑ Errors
MOTION          Reduced motion  [Follow Windows ▾]
STARTUP         ☐ Launch on sign-in
POSITION        ● Top-right
CLAUDE CODE     Not connected · port 47831      [Connect Claude Code]
```

## 7. System notifications

| Transition | Default | Text |
|---|---|---|
| → Waiting | on | Claude needs your input |
| → Done | off | Claude finished the task |
| → Error | on | Claude Code reported an error |

## 8. Open UX questions

- A top-right overlay sits over the close button of maximised windows. Options:
  "Edge trigger" appearance (hide until the cursor reaches the corner), or a
  small vertical offset below the title-bar zone.
- Whether Waiting should also nudge when the overlay is already in Waiting for a
  second session.
- Whether Done should fade back to Idle after a while, or stay white until the
  next event (currently: stays).
