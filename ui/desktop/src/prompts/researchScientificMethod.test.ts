import { describe, expect, it } from 'vitest';
import {
  buildResearchScientificMethodPrompt,
  RESEARCH_SCIENTIFIC_METHOD_PROMPT,
  RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY,
} from './researchScientificMethod';

describe('embedded research scientific method prompt', () => {
  it('contains the selected skill and its binding research invariants', () => {
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY).toBe('research-scientific-method');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain('name: research-scientific-method');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain('Phase 0 — Detect existing tooling');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain('R1');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain('R9');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain('Append-only correction');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain(
      'Math MCP equation routing (binding for Deep Research)'
    );
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain(
      "session's **Initial Inputs** (uploaded files and pasted items)"
    );
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain(
      'Discover the tools currently exposed by the `math_mcp` server/namespace'
    );
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT).toContain('pending Math MCP review');
    expect(RESEARCH_SCIENTIFIC_METHOD_PROMPT.length).toBeGreaterThan(20_000);
  });

  it('marks file and repository discovery inapplicable when no initial inputs exist', () => {
    const prompt = buildResearchScientificMethodPrompt(false);

    expect(prompt).toContain('No Initial Inputs were supplied');
    expect(prompt).toContain('Do not inspect workspace files');
    expect(prompt).toContain('no tree, ls,\nfind, glob, grep, or read calls against them');
    expect(prompt).toContain('Treat Phase 0 and the internal-program portion of Phase 2 as N/A');
  });

  it('retains initial-input inspection when the session has inputs', () => {
    expect(buildResearchScientificMethodPrompt(true)).toBe(RESEARCH_SCIENTIFIC_METHOD_PROMPT);
  });
});
