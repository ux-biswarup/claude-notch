/**
 * Presentation state machine.
 *
 * The canonical activity machine (hook events → WORKING/WAITING/DONE/ERROR/IDLE)
 * lives in Rust (`src-tauri/core/src/state/state_machine.rs`) so every provider
 * is normalised before the UI sees it. This module decides how a *transition*
 * between normalised states should look: which continuous motion a state has
 * and which one-shot attention effect a transition fires (PRD §8, §22.3).
 *
 * It is a pure reducer so it can be unit tested without a DOM.
 */
import type { ClaudeState } from './State';

/** One-shot attention effects the Nudge component plays on a transition. */
export type NudgeEffect = 'done-pop' | 'error-double-pulse' | 'waiting-enter';

/** Continuous motion attached to the current state. */
export type Motion = 'breathe' | 'pulse-slow' | 'none';

export interface PresentationState {
  state: ClaudeState;
  previous: ClaudeState | null;
  motion: Motion;
  /** Pending one-shot effect; cleared with {@link consumeEffect} once played. */
  effect: NudgeEffect | null;
  /** Increments whenever a new effect is queued so identical effects re-trigger. */
  effectSeq: number;
  /** Short label for the click card and accessibility text. */
  label: string;
  enteredAt: number;
}

export function initialPresentation(now = Date.now()): PresentationState {
  return {
    state: 'idle',
    previous: null,
    motion: 'none',
    effect: null,
    effectSeq: 0,
    label: labelFor('idle'),
    enteredAt: now,
  };
}

/** Working breathes very subtly; Waiting pulses slowly; everything else rests. */
export function motionFor(state: ClaudeState): Motion {
  switch (state) {
    case 'working':
      return 'breathe';
    case 'waiting':
      return 'pulse-slow';
    default:
      return 'none';
  }
}

export function labelFor(state: ClaudeState): string {
  switch (state) {
    case 'working':
      return 'Working';
    case 'waiting':
      return 'Waiting for you';
    case 'done':
      return 'Done';
    case 'error':
      return 'Something went wrong';
    case 'idle':
      return 'Idle';
  }
}

/** Effects are tied to *entering* a state, never to staying in it. */
export function effectForTransition(from: ClaudeState, to: ClaudeState): NudgeEffect | null {
  if (from === to) return null;
  switch (to) {
    case 'done':
      return 'done-pop';
    case 'error':
      return 'error-double-pulse';
    case 'waiting':
      return 'waiting-enter';
    default:
      return null;
  }
}

/**
 * Apply a new normalised state. Returns the *same object* when nothing changed
 * so callers can use identity to skip re-rendering and avoid replaying effects.
 */
export function reducePresentation(
  prev: PresentationState,
  next: ClaudeState,
  now = Date.now(),
): PresentationState {
  if (prev.state === next) return prev;
  const effect = effectForTransition(prev.state, next);
  return {
    state: next,
    previous: prev.state,
    motion: motionFor(next),
    effect,
    effectSeq: effect ? prev.effectSeq + 1 : prev.effectSeq,
    label: labelFor(next),
    enteredAt: now,
  };
}

/** Mark the pending effect as played. */
export function consumeEffect(p: PresentationState): PresentationState {
  return p.effect === null ? p : { ...p, effect: null };
}
