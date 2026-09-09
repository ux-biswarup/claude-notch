/**
 * Shared data contract between the Rust backend (src-tauri) and the UI.
 *
 * The Rust side serialises with serde `rename_all = "camelCase"`; enums are
 * lowercase / snake_case strings. Keep this file in sync with
 * `src-tauri/core/src/**`, `src-tauri/src/settings.rs` and docs/architecture.md.
 */

/** Normalised Claude Code activity state. Exactly one is active at a time (PRD §7). */
export type ClaudeState = 'idle' | 'working' | 'waiting' | 'done' | 'error';

export const CLAUDE_STATES: readonly ClaudeState[] = ['idle', 'working', 'waiting', 'done', 'error'];

/** Raw observations emitted by providers (PRD §14). Shown in the UI only as diagnostics. */
export type ActivityEvent =
  | 'process_started'
  | 'prompt_submitted'
  | 'tool_running'
  | 'output_received'
  | 'waiting_for_input'
  | 'task_completed'
  | 'error_detected'
  | 'process_exited';

/** How trustworthy a usage reading is (PRD §16.3). */
export type Fidelity = 'official' | 'derived' | 'manual';

export type UsageStatus = 'ok' | 'stale' | 'unavailable';

export type ActivitySource = 'hooks' | 'demo' | 'none';

export interface SessionInfo {
  sessionId: string;
  state: ClaudeState;
  /** Working directory reported by Claude Code; used to find the owning VS Code window. */
  cwd: string | null;
  lastEvent: ActivityEvent;
  /** Epoch milliseconds. */
  lastEventAt: number;
  /** Human-readable context, e.g. "Needs permission: Bash". */
  detail: string | null;
}

export interface ActivitySnapshot {
  /** Aggregate across sessions (waiting > error > working > done > idle). */
  state: ClaudeState;
  reason: string | null;
  /** Session that drives the aggregate state; target for "Open Claude Code". */
  focusSessionId: string | null;
  sessions: SessionInfo[];
  source: ActivitySource;
  updatedAt: number;
}

export interface UsageWindow {
  id: string;
  label: string;
  percent: number;
  resetsAt: number | null;
}

export interface UsageSnapshot {
  status: UsageStatus;
  /** 0–100 for the primary window, or null when unavailable. */
  percent: number | null;
  resetsAt: number | null;
  windowLabel: string;
  fidelity: Fidelity;
  account: string | null;
  fetchedAt: number | null;
  error: string | null;
  windows: UsageWindow[];
}

export interface Snapshot {
  activity: ActivitySnapshot;
  usage: UsageSnapshot;
  demoMode: boolean;
}

export type Appearance = 'always' | 'edge';
export type Position = 'top-right';
export type ReducedMotion = 'system' | 'on' | 'off';

export interface Settings {
  appearance: Appearance;
  notifyDone: boolean;
  notifyWaiting: boolean;
  notifyError: boolean;
  launchOnSignIn: boolean;
  position: Position;
  reducedMotion: ReducedMotion;
  /** Loopback port the Claude Code hooks post to. */
  hookPort: number;
}

export const DEFAULT_HOOK_PORT = 47831;

export const DEFAULT_SETTINGS: Settings = {
  appearance: 'always',
  notifyDone: false,
  notifyWaiting: true,
  notifyError: true,
  launchOnSignIn: false,
  position: 'top-right',
  reducedMotion: 'system',
  hookPort: DEFAULT_HOOK_PORT,
};

export function emptyUsage(error: string | null = null): UsageSnapshot {
  return {
    status: 'unavailable',
    percent: null,
    resetsAt: null,
    windowLabel: '5h',
    fidelity: 'manual',
    account: null,
    fetchedAt: null,
    error,
    windows: [],
  };
}

export function emptyActivity(now = Date.now()): ActivitySnapshot {
  return { state: 'idle', reason: null, focusSessionId: null, sessions: [], source: 'none', updatedAt: now };
}

export function emptySnapshot(now = Date.now()): Snapshot {
  return { activity: emptyActivity(now), usage: emptyUsage(), demoMode: false };
}
