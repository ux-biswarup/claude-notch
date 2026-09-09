import { describe, expect, it } from 'vitest';
import {
  consumeEffect,
  effectForTransition,
  initialPresentation,
  motionFor,
  reducePresentation,
} from '../src/state/ClaudeStateMachine';
import type { ClaudeState } from '../src/state/State';

describe('presentation reducer', () => {
  it('starts idle with no motion and no effect', () => {
    const p = initialPresentation(0);
    expect(p.state).toBe('idle');
    expect(p.motion).toBe('none');
    expect(p.effect).toBeNull();
    expect(p.effectSeq).toBe(0);
  });

  it('returns the identical object when the state does not change', () => {
    const p = reducePresentation(initialPresentation(0), 'working', 1);
    expect(reducePresentation(p, 'working', 2)).toBe(p);
  });

  it('working breathes quietly and never fires an effect', () => {
    const p = reducePresentation(initialPresentation(0), 'working', 1);
    expect(p.motion).toBe('breathe');
    expect(p.effect).toBeNull();
    expect(p.label).toBe('Working');
  });

  it('waiting pulses slowly and nudges once on entry', () => {
    const working = reducePresentation(initialPresentation(0), 'working', 1);
    const waiting = reducePresentation(working, 'waiting', 2);
    expect(waiting.motion).toBe('pulse-slow');
    expect(waiting.effect).toBe('waiting-enter');
    expect(waiting.previous).toBe('working');
  });

  it('done pops once and then rests', () => {
    const working = reducePresentation(initialPresentation(0), 'working', 1);
    const done = reducePresentation(working, 'done', 2);
    expect(done.effect).toBe('done-pop');
    expect(done.motion).toBe('none');
    expect(consumeEffect(done).effect).toBeNull();
    expect(consumeEffect(done).state).toBe('done');
  });

  it('error double-pulses', () => {
    const working = reducePresentation(initialPresentation(0), 'working', 1);
    const error = reducePresentation(working, 'error', 2);
    expect(error.effect).toBe('error-double-pulse');
    expect(error.label).toBe('Something went wrong');
  });

  it('increments effectSeq so a repeated effect re-triggers', () => {
    let p = initialPresentation(0);
    p = reducePresentation(p, 'working', 1);
    p = reducePresentation(p, 'done', 2);
    const firstSeq = p.effectSeq;
    p = reducePresentation(p, 'working', 3);
    p = reducePresentation(p, 'done', 4);
    expect(p.effectSeq).toBe(firstSeq + 1);
  });

  it('consumeEffect is a no-op when nothing is pending', () => {
    const p = initialPresentation(0);
    expect(consumeEffect(p)).toBe(p);
  });

  it('walks the PRD demo cycle', () => {
    const cycle: ClaudeState[] = ['working', 'waiting', 'working', 'done', 'error', 'idle'];
    const expectedMotion = ['breathe', 'pulse-slow', 'breathe', 'none', 'none', 'none'];
    const expectedEffect = [null, 'waiting-enter', null, 'done-pop', 'error-double-pulse', null];
    let p = initialPresentation(0);
    cycle.forEach((state, i) => {
      p = reducePresentation(p, state, i + 1);
      expect(p.motion).toBe(expectedMotion[i]);
      expect(p.effect).toBe(expectedEffect[i]);
    });
  });

  it('exposes motion and effect lookups independently', () => {
    expect(motionFor('idle')).toBe('none');
    expect(effectForTransition('waiting', 'waiting')).toBeNull();
    expect(effectForTransition('idle', 'working')).toBeNull();
  });
});
