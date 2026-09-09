import { describe, expect, it } from 'vitest';
import { describeActivity, folderName } from '../src/ui/TrafficSignal/describe';
import { emptyActivity, type SessionInfo } from '../src/state/State';

function session(overrides: Partial<SessionInfo> = {}): SessionInfo {
  return {
    sessionId: 'a',
    state: 'waiting',
    cwd: 'C:\\Personal\\projects\\claude-notch',
    lastEvent: 'waiting_for_input',
    lastEventAt: 1,
    detail: 'Needs permission: Bash',
    ...overrides,
  };
}

describe('folderName', () => {
  it('handles Windows and POSIX paths and trailing separators', () => {
    expect(folderName('C:\\Personal\\projects\\claude-notch')).toBe('claude-notch');
    expect(folderName('C:\\Personal\\projects\\claude-notch\\')).toBe('claude-notch');
    expect(folderName('/home/me/work/app')).toBe('app');
    expect(folderName('')).toBeNull();
    expect(folderName(null)).toBeNull();
  });
});

describe('describeActivity', () => {
  it('explains why attention is requested and where', () => {
    const a = {
      ...emptyActivity(0),
      state: 'waiting' as const,
      reason: 'Needs permission: Bash',
      focusSessionId: 'a',
      sessions: [session()],
      source: 'hooks' as const,
    };
    expect(describeActivity(a)).toBe('Needs permission: Bash · claude-notch');
  });

  it('mentions other concurrent sessions', () => {
    const a = {
      ...emptyActivity(0),
      state: 'waiting' as const,
      reason: 'Needs permission: Bash',
      focusSessionId: 'a',
      sessions: [session(), session({ sessionId: 'b', state: 'working', cwd: 'D:\\other' })],
      source: 'hooks' as const,
    };
    expect(describeActivity(a)).toBe('Needs permission: Bash · claude-notch · +1 other session');
  });

  it('falls back to an honest empty state', () => {
    expect(describeActivity(emptyActivity(0))).toBe('No Claude Code session detected');
    expect(describeActivity({ ...emptyActivity(0), source: 'hooks' })).toBe('No active session');
    expect(describeActivity({ ...emptyActivity(0), source: 'demo' })).toBe('Demo mode');
  });
});
