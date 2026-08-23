import { describe, expect, it } from 'vitest';
import { addToRecentModels } from './recentModels';

describe('addToRecentModels', () => {
  it('promotes the selected model without duplicates and retains five entries', () => {
    const current = [
      { provider: 'openai', model: 'gpt-5' },
      { provider: 'anthropic', model: 'claude-sonnet' },
      { provider: 'google', model: 'gemini-3' },
      { provider: 'xai', model: 'grok-4' },
      { provider: 'mistral', model: 'mistral-large' },
    ];

    expect(addToRecentModels(current, 'anthropic', 'claude-sonnet')).toEqual([
      { provider: 'anthropic', model: 'claude-sonnet' },
      { provider: 'openai', model: 'gpt-5' },
      { provider: 'google', model: 'gemini-3' },
      { provider: 'xai', model: 'grok-4' },
      { provider: 'mistral', model: 'mistral-large' },
    ]);
  });
});
