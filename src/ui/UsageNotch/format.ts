import type { UsageSnapshot } from '../../state/State';

export type UsageTone = 'ok' | 'warn' | 'high' | 'stale' | 'off';

export interface UsageView {
  /** e.g. "72%", "72% · stale", "—" */
  primary: string;
  /** e.g. "Resets in 2h 13m", "Usage unavailable" */
  secondary: string | null;
  tone: UsageTone;
  /** 0..1 for the ring */
  ratio: number;
  tooltip: string;
}

export const WARN_AT = 70;
export const HIGH_AT = 90;

/**
 * Never present uncertain data as authoritative (PRD §16.4, §22.5):
 * stale readings keep the number but say so; missing readings say "unavailable".
 */
export function formatUsage(u: UsageSnapshot, nowMs: number): UsageView {
  if (u.status === 'unavailable' || u.percent === null) {
    return {
      primary: '—',
      secondary: 'Usage unavailable',
      tone: 'off',
      ratio: 0,
      tooltip: u.error ?? 'Usage unavailable',
    };
  }

  const pct = Math.round(clamp(u.percent, 0, 100));
  const stale = u.status === 'stale';
  const tone: UsageTone = stale ? 'stale' : pct >= HIGH_AT ? 'high' : pct >= WARN_AT ? 'warn' : 'ok';

  let secondary: string | null = null;
  if (u.resetsAt !== null) {
    const remaining = u.resetsAt - nowMs;
    secondary = remaining <= 0 ? 'Resets now' : `Resets in ${formatDuration(remaining)}`;
  }

  const tooltip = [
    `${u.windowLabel} window`,
    u.fidelity,
    u.account,
    stale && u.fetchedAt !== null ? `last reading ${formatDuration(nowMs - u.fetchedAt)} ago` : null,
    u.error,
  ]
    .filter((part): part is string => typeof part === 'string' && part.length > 0)
    .join(' · ');

  return { primary: stale ? `${pct}% · stale` : `${pct}%`, secondary, tone, ratio: pct / 100, tooltip };
}

/** Compact humanised duration: "2h 13m", "45m", "3d 4h", "<1m". */
export function formatDuration(ms: number): string {
  if (ms <= 0) return 'now';
  const totalMin = Math.round(ms / 60_000);
  if (totalMin < 1) return '<1m';
  const days = Math.floor(totalMin / 1440);
  const hours = Math.floor((totalMin % 1440) / 60);
  const mins = totalMin % 60;
  if (days > 0) return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
  if (hours > 0) return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  return `${mins}m`;
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}
