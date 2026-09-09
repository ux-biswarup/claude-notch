/**
 * Browser-only backend. Runs the demo script in-process so `pnpm dev` shows the
 * complete UX (Working → Waiting → Done → Error) without Tauri or Claude Code.
 *
 * Keyboard (browser preview only):
 *   →  / Space   advance to the next scripted step (stops auto-cycling)
 *   d            toggle demo mode
 *   s            toggle the settings panel
 */
import {
  DEFAULT_SETTINGS,
  emptySnapshot,
  type ActivityEvent,
  type ClaudeState,
  type Settings,
  type Snapshot,
  type UsageSnapshot,
} from '../state/State';
import type { Backend, HooksStatus, Unsubscribe } from './ipc';

export interface DemoStep {
  event: ActivityEvent;
  state: ClaudeState;
  detail: string;
  holdMs: number;
}

/** Mirrors `core::demo::script()` so the browser preview and Tauri demo mode behave the same. */
export const DEMO_SCRIPT: readonly DemoStep[] = [
  { event: 'process_started', state: 'idle', detail: 'Session started', holdMs: 1500 },
  { event: 'prompt_submitted', state: 'working', detail: 'Reading the request', holdMs: 2000 },
  { event: 'tool_running', state: 'working', detail: 'Running Bash', holdMs: 3000 },
  { event: 'waiting_for_input', state: 'waiting', detail: 'Needs permission: Edit', holdMs: 5000 },
  { event: 'output_received', state: 'working', detail: 'Applying edit', holdMs: 2500 },
  { event: 'task_completed', state: 'done', detail: 'Task completed', holdMs: 4000 },
  { event: 'prompt_submitted', state: 'working', detail: 'Working on a follow-up', holdMs: 2000 },
  { event: 'error_detected', state: 'error', detail: 'API request failed', holdMs: 4000 },
  { event: 'process_exited', state: 'idle', detail: 'Session ended', holdMs: 1500 },
];

export const DEMO_SESSION = 'demo';
export const DEMO_CWD = 'C:\\Personal\\projects\\claude-notch';

export function demoUsage(nowMs: number): UsageSnapshot {
  const fiveHourReset = nowMs + (2 * 60 + 13) * 60_000;
  return {
    status: 'ok',
    percent: 72,
    resetsAt: fiveHourReset,
    windowLabel: '5h',
    fidelity: 'manual',
    account: 'Demo',
    fetchedAt: nowMs,
    error: null,
    windows: [
      { id: 'five_hour', label: '5h', percent: 72, resetsAt: fiveHourReset },
      { id: 'seven_day', label: '7d', percent: 31, resetsAt: nowMs + 3 * 86_400_000 },
    ],
  };
}

const SETTINGS_KEY = 'claude-notch.settings';

export class BrowserDemoBackend implements Backend {
  readonly kind = 'browser' as const;

  private snapshot: Snapshot = emptySnapshot();
  private settings: Settings = loadSettings();
  private readonly listeners = new Set<(s: Snapshot) => void>();
  private readonly settingsListeners = new Set<() => void>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private index = -1;

  constructor() {
    this.setDemo(true);
    if (typeof window !== 'undefined') this.bindKeys();
  }

  getSnapshot(): Promise<Snapshot> {
    return Promise.resolve(this.snapshot);
  }

  onSnapshot(cb: (snapshot: Snapshot) => void): Unsubscribe {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  setDemoMode(enabled: boolean): Promise<Snapshot> {
    this.setDemo(enabled);
    return Promise.resolve(this.snapshot);
  }

  focusClaude(sessionId: string | null): Promise<boolean> {
    console.info('[demo] focus Claude Code requested for session', sessionId);
    return Promise.resolve(false);
  }

  getSettings(): Promise<Settings> {
    return Promise.resolve(this.settings);
  }

  setSettings(settings: Settings): Promise<Settings> {
    this.settings = settings;
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    } catch {
      /* private mode etc. – settings simply don't persist */
    }
    return Promise.resolve(settings);
  }

  installHooks(): Promise<HooksStatus> {
    return Promise.resolve(this.hooksStatusSync('Hooks can only be installed from the desktop app.'));
  }

  hooksStatus(): Promise<HooksStatus> {
    return Promise.resolve(this.hooksStatusSync(null));
  }

  onOpenSettings(cb: () => void): Unsubscribe {
    this.settingsListeners.add(cb);
    return () => this.settingsListeners.delete(cb);
  }

  resizeToContent(): Promise<void> {
    return Promise.resolve();
  }

  /** Advance to the next scripted step and publish a snapshot. */
  advance(): DemoStep {
    this.index = (this.index + 1) % DEMO_SCRIPT.length;
    const step = DEMO_SCRIPT[this.index]!;
    const now = Date.now();
    const sessions =
      step.event === 'process_exited'
        ? []
        : [
            {
              sessionId: DEMO_SESSION,
              state: step.state,
              cwd: DEMO_CWD,
              lastEvent: step.event,
              lastEventAt: now,
              detail: step.detail,
            },
          ];
    this.snapshot = {
      activity: {
        state: step.state,
        reason: sessions.length ? step.detail : null,
        focusSessionId: sessions[0]?.sessionId ?? null,
        sessions,
        source: 'demo',
        updatedAt: now,
      },
      usage: demoUsage(now),
      demoMode: true,
    };
    this.emit();
    return step;
  }

  private hooksStatusSync(message: string | null): HooksStatus {
    return { installed: false, settingsPath: null, port: this.settings.hookPort, listening: false, message };
  }

  private setDemo(on: boolean): void {
    this.stopTimer();
    if (on) {
      this.index = -1;
      this.tick();
    } else {
      this.snapshot = { ...emptySnapshot(), demoMode: false };
      this.emit();
    }
  }

  private readonly tick = (): void => {
    const step = this.advance();
    this.timer = setTimeout(this.tick, step.holdMs);
  };

  private stopTimer(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  private emit(): void {
    for (const cb of this.listeners) cb(this.snapshot);
  }

  private bindKeys(): void {
    window.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowRight' || e.key === ' ') {
        e.preventDefault();
        this.stopTimer();
        this.advance();
      } else if (e.key === 'd') {
        void this.setDemoMode(!this.snapshot.demoMode);
      } else if (e.key === 's') {
        for (const cb of this.settingsListeners) cb();
      }
    });
  }
}

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (raw) return { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Partial<Settings>) };
  } catch {
    /* fall through to defaults */
  }
  return { ...DEFAULT_SETTINGS };
}
