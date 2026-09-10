import { describe, expect, it } from 'vitest';
import type { ProviderCapabilitiesDto } from '@repo-makeover/gosling-sdk';
import { continuityClassForProvider } from './providers';

function capabilities(overrides: Partial<ProviderCapabilitiesDto> = {}): ProviderCapabilitiesDto {
  return {
    contextOwnership: 'provider',
    nativeResume: 'unsupported',
    historyImport: 'unsupported',
    inPlaceModelChange: 'unsupported',
    sessionFork: 'unsupported',
    bootstrapHandoff: 'unsupported',
    bootstrapAcknowledgement: 'unsupported',
    ...overrides,
  };
}

describe('provider continuity classification', () => {
  it('classifies native resume and history import as seamless', () => {
    expect(continuityClassForProvider(capabilities({ nativeResume: 'supported' }))).toBe(
      'seamless_resume'
    );
    expect(continuityClassForProvider(capabilities({ historyImport: 'required' }))).toBe(
      'seamless_resume'
    );
  });

  it('classifies Gosling injection and provider bootstrap as summarized handoff', () => {
    expect(continuityClassForProvider(capabilities({ contextOwnership: 'gosling' }))).toBe(
      'summarized_handoff'
    );
    expect(continuityClassForProvider(capabilities({ bootstrapHandoff: 'required' }))).toBe(
      'summarized_handoff'
    );
  });

  it('fails closed to new context when no delivery capability is declared', () => {
    expect(continuityClassForProvider(capabilities())).toBe('new_context_only');
    expect(continuityClassForProvider(undefined)).toBe('new_context_only');
  });
});
