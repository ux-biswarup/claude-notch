/**
 * Composes the two surfaces (UsageNotch above TrafficSignal), the Nudge that
 * owns motion, and the minimal Settings panel (PRD §15).
 */
import type { Backend } from './ipc';
import type { Snapshot } from '../state/State';
import {
  consumeEffect,
  initialPresentation,
  reducePresentation,
  type PresentationState,
} from '../state/ClaudeStateMachine';
import { UsageNotch } from '../ui/UsageNotch/UsageNotch';
import { TrafficSignal } from '../ui/TrafficSignal/TrafficSignal';
import { Nudge } from '../ui/Nudge/Nudge';
import { SettingsPanel } from '../ui/Settings/Settings';

export class App {
  private readonly usage = new UsageNotch();
  private readonly signal: TrafficSignal;
  private readonly nudge: Nudge;
  private readonly settings: SettingsPanel;
  private readonly shell = document.createElement('main');
  private presentation: PresentationState = initialPresentation();
  private snapshot: Snapshot | null = null;

  constructor(
    private readonly root: HTMLElement,
    private readonly backend: Backend,
  ) {
    this.signal = new TrafficSignal({ onOpenClaude: () => void this.openClaude() });
    this.nudge = new Nudge(this.signal.lights);
    this.settings = new SettingsPanel(backend, {
      onChange: (s) => this.nudge.setPreference(s.reducedMotion),
    });
  }

  async start(): Promise<void> {
    this.shell.className = 'notch';
    this.shell.append(this.usage.element, this.signal.element, this.settings.element);
    this.root.replaceChildren(this.shell);

    const settings = await this.backend.getSettings();
    this.nudge.setPreference(settings.reducedMotion);
    this.settings.setValues(settings);

    this.backend.onSnapshot((s) => this.apply(s));
    this.backend.onOpenSettings(() => this.settings.toggle());
    this.apply(await this.backend.getSnapshot());

    this.observeSize();
    // Relative times ("Resets in 2h 13m") drift; refresh them once a minute.
    setInterval(() => {
      if (this.snapshot) this.usage.update(this.snapshot.usage);
    }, 60_000);
  }

  private apply(snapshot: Snapshot): void {
    this.snapshot = snapshot;
    document.documentElement.dataset.demo = String(snapshot.demoMode);
    this.usage.update(snapshot.usage);

    const next = reducePresentation(this.presentation, snapshot.activity.state);
    if (next !== this.presentation) {
      this.presentation = consumeEffect(next);
      this.signal.update(next, snapshot.activity);
      this.nudge.applyMotion(next);
      this.nudge.play(next);
    } else {
      this.signal.updateDetails(next, snapshot.activity);
    }
  }

  private async openClaude(): Promise<void> {
    try {
      const ok = await this.backend.focusClaude(this.snapshot?.activity.focusSessionId ?? null);
      if (!ok) this.signal.flashMessage('Could not find the Claude Code window.');
    } catch (err) {
      this.signal.flashMessage(String(err));
    }
  }

  /** Keep the (transparent) native window no larger than the rendered content. */
  private observeSize(): void {
    if (typeof ResizeObserver === 'undefined') return;
    let pending = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(() => {
        const rect = this.shell.getBoundingClientRect();
        const pad = 12; // room for glow/shadow outside the boxes
        void this.backend
          .resizeToContent(Math.ceil(rect.width + pad * 2), Math.ceil(rect.height + pad * 2))
          .catch((err: unknown) => console.warn('resizeToContent failed', err));
      });
    });
    observer.observe(this.shell);
  }
}
