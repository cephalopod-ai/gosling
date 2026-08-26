import { describe, expect, it } from 'vitest';
import {
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
});
