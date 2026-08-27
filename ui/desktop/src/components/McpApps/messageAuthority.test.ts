import { describe, expect, it, vi } from 'vitest';
import { confirmMcpAppMessage } from './messageAuthority';

describe('confirmMcpAppMessage', () => {
  it('identifies the app and submits only after explicit confirmation', () => {
    const confirm = vi.fn().mockReturnValue(true);

    expect(confirmMcpAppMessage('weather', 'Use Chicago', confirm)).toBe(true);
    expect(confirm).toHaveBeenCalledWith(expect.stringContaining('weather wants to send'));
    expect(confirm).toHaveBeenCalledWith(expect.stringContaining('Use Chicago'));
  });

  it('preserves denial and bounds untrusted preview text', () => {
    const confirm = vi.fn().mockReturnValue(false);

    expect(confirmMcpAppMessage('', 'x'.repeat(2_000), confirm)).toBe(false);
    const prompt = confirm.mock.calls[0]?.[0] as string;
    expect(prompt).toContain('MCP App wants to send');
    expect(prompt).toContain('1000 more characters');
    expect(prompt.length).toBeLessThan(1_200);
  });
});
