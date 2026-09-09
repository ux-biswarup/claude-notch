/**
 * Owns temporary animation and attention behaviour (PRD §8, §15).
 *
 * - Continuous motion (`data-motion`): Working breathes, Waiting pulses slowly.
 * - One-shot effects (`fx-*` classes): Done pops once, Error double-pulses,
 *   Waiting expands briefly on entry.
 * - Reduced motion: honours the OS preference unless the user overrides it.
 */
import type { NudgeEffect, PresentationState } from '../../state/ClaudeStateMachine';
import type { ReducedMotion } from '../../state/State';
import './Nudge.css';

/** Must be ≥ the CSS animation durations in Nudge.css. */
const EFFECT_MS: Record<NudgeEffect, number> = {
  'done-pop': 750,
  'error-double-pulse': 1050,
  'waiting-enter': 650,
};

export class Nudge {
  private pref: ReducedMotion = 'system';
  private readonly mq = window.matchMedia('(prefers-reduced-motion: reduce)');
  private last: PresentationState | null = null;
  private effectTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly target: HTMLElement) {
    this.mq.addEventListener('change', () => this.reapply());
  }

  get reduced(): boolean {
    return this.pref === 'on' || (this.pref === 'system' && this.mq.matches);
  }

  setPreference(pref: ReducedMotion): void {
    this.pref = pref;
    document.documentElement.dataset.reducedMotion = pref;
    this.reapply();
  }

  applyMotion(p: PresentationState): void {
    this.last = p;
    this.target.dataset.motion = this.reduced ? 'none' : p.motion;
  }

  play(p: PresentationState): void {
    if (p.effect === null || this.reduced) return;
    const cls = `fx-${p.effect}`;
    // Remove + reflow + add restarts the animation if the same effect repeats.
    this.target.classList.remove(cls);
    void this.target.offsetWidth;
    this.target.classList.add(cls);
    if (this.effectTimer !== null) clearTimeout(this.effectTimer);
    this.effectTimer = setTimeout(() => {
      this.target.classList.remove(cls);
      this.effectTimer = null;
    }, EFFECT_MS[p.effect]);
  }

  private reapply(): void {
    if (this.last) this.applyMotion(this.last);
  }
}
