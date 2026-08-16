import { describe, it, expect } from 'vitest';
import { summarizeArguments } from './ToolCallConfirmation';

// The approval prompt used to show only the first line of the first matching
// argument, clipped to 140 chars and CSS-truncated again — so a shell command
// whose destructive part was on line 2 got approved unseen (WEB-GOS-002).
describe('summarizeArguments', () => {
  it('discloses every line of a multi-line command, not just the first', () => {
    const detail = summarizeArguments({
      command: 'echo "looks harmless"\nrm -rf /important/data',
    });
    expect(detail).toContain('echo "looks harmless"');
    expect(detail).toContain('rm -rf /important/data');
  });

  it('does not clip a command at the old 140-character limit', () => {
    const command = `echo ${'a'.repeat(300)}`;
    const detail = summarizeArguments({ command });
    expect(detail).toBe(command);
    expect(detail!.length).toBeGreaterThan(140);
  });

  it('still bounds a pathological payload, and says that it did', () => {
    const detail = summarizeArguments({ command: 'x'.repeat(10_000) });
    expect(detail!.length).toBeLessThan(10_000);
    expect(detail).toContain('truncated for display');
  });

  it('returns nothing when there is no recognized argument', () => {
    expect(summarizeArguments(undefined)).toBeUndefined();
    expect(summarizeArguments({})).toBeUndefined();
    expect(summarizeArguments({ command: '   ' })).toBeUndefined();
  });
});
