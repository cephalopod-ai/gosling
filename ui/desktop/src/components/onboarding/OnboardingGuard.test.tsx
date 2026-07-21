import { describe, expect, it } from 'vitest';

import { resolveOnboardingModel } from './OnboardingGuard';

describe('resolveOnboardingModel', () => {
  const models = [{ id: 'qwen2.5:latest' }, { id: 'llama3.2:latest' }];

  it('uses an explicitly selected available model', () => {
    expect(resolveOnboardingModel('llama3.2:latest', 'qwen3', models)).toBe('llama3.2:latest');
  });

  it('uses the declared default only when the provider reports it as available', () => {
    expect(resolveOnboardingModel(undefined, 'qwen2.5:latest', models)).toBe('qwen2.5:latest');
  });

  it('falls back to the first live model instead of saving an unavailable default', () => {
    expect(resolveOnboardingModel(undefined, 'qwen3', models)).toBe('qwen2.5:latest');
  });

  it('rejects unavailable explicit models and empty provider inventories', () => {
    expect(resolveOnboardingModel('qwen3', 'qwen3', models)).toBeNull();
    expect(resolveOnboardingModel(undefined, 'qwen3', [])).toBeNull();
  });
});
