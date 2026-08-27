import { describe, expect, it } from 'vitest';
import { buildResearchLibraryPrompt, RESEARCH_LIBRARY_PROMPT_KEY } from './researchLibrary';

describe('research library prompt', () => {
  it('requires final deliverables in both session Outputs and the durable library', () => {
    const prompt = buildResearchLibraryPrompt('/Users/tester/Documents/Gosling Research Library');

    expect(RESEARCH_LIBRARY_PROMPT_KEY).toBe('research-library');
    expect(prompt).toContain('/Users/tester/Documents/Gosling Research Library');
    expect(prompt).toContain('Session Outputs');
    expect(prompt).toContain('Research Library');
    expect(prompt).toContain("session's Outputs inventory");
    expect(prompt).toContain('across sessions and threads');
    expect(prompt).toContain('identical final content');
    expect(prompt).toContain('Never overwrite either copy silently');
    expect(prompt).toContain('secondary, potentially stale context');
    expect(prompt).toContain('not as a source of truth');
    expect(prompt).toContain('verify its load-bearing claims against current primary evidence');
  });
});
