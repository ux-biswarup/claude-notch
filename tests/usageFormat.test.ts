import { describe, expect, it } from 'vitest';
import { formatDuration, formatUsage } from '../src/ui/UsageNotch/format';
import { emptyUsage, type UsageSnapshot } from '../src/state/State';

const NOW = 1_700_000_000_000;

function reading(overrides: Partial<UsageSnapshot> = {}): UsageSnapshot {
  return {
    status: 'ok',
    percent: 72,
    resetsAt: NOW + (2 * 60 + 13) * 60_000,
    windowLabel: '5h',
    fidelity: 'official',
    account: 'Max',
    fetchedAt: NOW - 30_000,
    error: null,
    windows: [],
    ...overrides,
  };
}

describe('formatUsage', () => {
  it('shows the percentage and the reset countdown', () => {
    const v = formatUsage(reading(), NOW);
    expect(v.primary).toBe('72%');
    expect(v.secondary).toBe('Resets in 2h 13m');
    expect(v.tone).toBe('warn');
    expect(v.ratio).toBeCloseTo(0.72);
    expect(v.tooltip).toContain('official');
    expect(v.tooltip).toContain('Max');
  });

  it('never hides staleness (PRD §16.4)', () => {
    const v = formatUsage(reading({ status: 'stale', fetchedAt: NOW - 20 * 60_000 }), NOW);
    expect(v.primary).toBe('72% · stale');
    expect(v.tone).toBe('stale');
    expect(v.tooltip).toContain('last reading 20m ago');
  });

  it('says unavailable instead of inventing a number', () => {
    const v = formatUsage(emptyUsage('credentials not found'), NOW);
    expect(v.primary).toBe('—');
    expect(v.secondary).toBe('Usage unavailable');
    expect(v.tone).toBe('off');
    expect(v.ratio).toBe(0);
    expect(v.tooltip).toBe('credentials not found');
  });

  it('escalates tone with usage', () => {
    expect(formatUsage(reading({ percent: 12 }), NOW).tone).toBe('ok');
    expect(formatUsage(reading({ percent: 70 }), NOW).tone).toBe('warn');
    expect(formatUsage(reading({ percent: 95 }), NOW).tone).toBe('high');
  });

  it('clamps out-of-range percentages', () => {
    expect(formatUsage(reading({ percent: 140 }), NOW).primary).toBe('100%');
    expect(formatUsage(reading({ percent: -5 }), NOW).primary).toBe('0%');
  });

  it('handles a reset that already passed', () => {
    expect(formatUsage(reading({ resetsAt: NOW - 1 }), NOW).secondary).toBe('Resets now');
    expect(formatUsage(reading({ resetsAt: null }), NOW).secondary).toBeNull();
  });
});

describe('formatDuration', () => {
  it('formats compactly', () => {
    expect(formatDuration(0)).toBe('now');
    expect(formatDuration(20_000)).toBe('<1m');
    expect(formatDuration(45 * 60_000)).toBe('45m');
    expect(formatDuration(60 * 60_000)).toBe('1h');
    expect(formatDuration((2 * 60 + 13) * 60_000)).toBe('2h 13m');
    expect(formatDuration(3 * 86_400_000 + 4 * 3_600_000)).toBe('3d 4h');
    expect(formatDuration(2 * 86_400_000)).toBe('2d');
  });
});
