import { describe, expect, it } from 'vitest';
import { buildResearchLibraryPrompt, RESEARCH_LIBRARY_PROMPT_KEY } from './researchLibrary';

describe('research library prompt', () => {
  it('routes deliverables while keeping prior reports below primary evidence', () => {
    const prompt = buildResearchLibraryPrompt('/Users/tester/Documents/Gosling Research Library');

    expect(RESEARCH_LIBRARY_PROMPT_KEY).toBe('research-library');
    expect(prompt).toContain('/Users/tester/Documents/Gosling Research Library');
    expect(prompt).toContain('every user-facing document');
    expect(prompt).toContain('secondary, potentially stale context');
    expect(prompt).toContain('not as a source of truth');
    expect(prompt).toContain('verify its load-bearing claims against current primary evidence');
  });
});
