/**
 * Backend boundary. The UI talks to exactly one `Backend`:
 *  - `TauriBackend` when running inside the Tauri shell (invoke + events)
 *  - `BrowserDemoBackend` when opened in a plain browser via `pnpm dev`, so the
 *    whole UX can be iterated on without Rust or Claude Code (PRD §16.6).
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Settings, Snapshot } from '../state/State';
import { BrowserDemoBackend } from './demo';

/** Event names emitted by the Rust side (see src-tauri/src/state/mod.rs). */
export const SNAPSHOT_EVENT = 'notch:snapshot';
export const OPEN_SETTINGS_EVENT = 'notch:open-settings';

export interface HooksStatus {
  installed: boolean;
  settingsPath: string | null;
  port: number;
  listening: boolean;
  message: string | null;
}

export type Unsubscribe = () => void;

export interface Backend {
  readonly kind: 'tauri' | 'browser';
  getSnapshot(): Promise<Snapshot>;
  onSnapshot(cb: (snapshot: Snapshot) => void): Unsubscribe;
  setDemoMode(enabled: boolean): Promise<Snapshot>;
  /** Bring the VS Code / terminal window owning the session to the foreground. */
  focusClaude(sessionId: string | null): Promise<boolean>;
  getSettings(): Promise<Settings>;
  setSettings(settings: Settings): Promise<Settings>;
  installHooks(): Promise<HooksStatus>;
  hooksStatus(): Promise<HooksStatus>;
  onOpenSettings(cb: () => void): Unsubscribe;
  /** Shrink the transparent window to the rendered content so it blocks as few clicks as possible. */
  resizeToContent(width: number, height: number): Promise<void>;
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export function createBackend(): Backend {
  return isTauri() ? new TauriBackend() : new BrowserDemoBackend();
}

class TauriBackend implements Backend {
  readonly kind = 'tauri' as const;

  getSnapshot(): Promise<Snapshot> {
    return invoke<Snapshot>('get_snapshot');
  }

  onSnapshot(cb: (snapshot: Snapshot) => void): Unsubscribe {
    return subscribe<Snapshot>(SNAPSHOT_EVENT, cb);
  }

  setDemoMode(enabled: boolean): Promise<Snapshot> {
    return invoke<Snapshot>('set_demo_mode', { enabled });
  }

  focusClaude(sessionId: string | null): Promise<boolean> {
    return invoke<boolean>('focus_claude', { sessionId });
  }

  getSettings(): Promise<Settings> {
    return invoke<Settings>('get_settings');
  }

  setSettings(settings: Settings): Promise<Settings> {
    return invoke<Settings>('set_settings', { settings });
  }

  installHooks(): Promise<HooksStatus> {
    return invoke<HooksStatus>('install_hooks');
  }

  hooksStatus(): Promise<HooksStatus> {
    return invoke<HooksStatus>('hooks_status');
  }

  onOpenSettings(cb: () => void): Unsubscribe {
    return subscribe<void>(OPEN_SETTINGS_EVENT, () => cb());
  }

  resizeToContent(width: number, height: number): Promise<void> {
    return invoke<void>('resize_to_content', { width, height });
  }
}

/** `listen` is async; wrap it so callers get a synchronous unsubscribe handle. */
function subscribe<T>(event: string, cb: (payload: T) => void): Unsubscribe {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  listen<T>(event, (e) => cb(e.payload))
    .then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    })
    .catch((err: unknown) => console.error(`failed to listen for ${event}`, err));
  return () => {
    cancelled = true;
    unlisten?.();
  };
}
