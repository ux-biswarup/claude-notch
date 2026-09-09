import type { ActivitySnapshot } from '../../state/State';

/** Last path segment of a Windows or POSIX path. */
export function folderName(cwd: string | null | undefined): string | null {
  if (!cwd) return null;
  const trimmed = cwd.replace(/[\\/]+$/, '');
  const last = trimmed.split(/[\\/]/).pop();
  return last && last.length > 0 ? last : null;
}

/** Secondary line of the click card: why attention is requested and where (PRD §9). */
export function describeActivity(a: ActivitySnapshot): string {
  const focus = a.sessions.find((s) => s.sessionId === a.focusSessionId) ?? null;
  const parts: string[] = [];
  if (a.reason) parts.push(a.reason);
  const folder = folderName(focus?.cwd);
  if (folder) parts.push(folder);
  const others = a.sessions.length - (focus ? 1 : 0);
  if (others > 0) parts.push(`+${others} other session${others === 1 ? '' : 's'}`);
  if (parts.length > 0) return parts.join(' · ');
  if (a.source === 'demo') return 'Demo mode';
  if (a.source === 'none') return 'No Claude Code session detected';
  return 'No active session';
}
