import { describe, expect, it } from 'vitest';
import { buildResearchModelTeamPrompt, RESEARCH_MODEL_TEAM_PROMPT_KEY } from './researchModelTeam';

describe('research model team prompt', () => {
  it('keeps solo research on the existing single-model path', () => {
    expect(RESEARCH_MODEL_TEAM_PROMPT_KEY).toBe('research-model-team');
    expect(buildResearchModelTeamPrompt({ mode: 'solo', models: [] }, [])).toBeNull();
  });

  it('pins exact models, independent drafts, critique, and explicit degradation', () => {
    const prompt = buildResearchModelTeamPrompt(
      {
        mode: 'trio',
        models: [
          { provider: 'codex', model: 'gpt-5.6-sol' },
          { provider: 'claude', model: 'claude-opus-5' },
          { provider: 'groq', model: 'llama-4' },
        ],
      },
      ['search', 'math_mcp']
    );

    expect(prompt).toContain('codex/gpt-5.6-sol — lead researcher');
    expect(prompt).toContain('claude/claude-opus-5');
    expect(prompt).toContain('groq/llama-4');
    expect(prompt).toContain('start one asynchronous `delegate` task');
    expect(prompt).toContain('Do not load a delegate result until that outline exists');
    expect(prompt).toContain('exactly one bounded critique round');
    expect(prompt).toContain('label the result degraded');
    expect(prompt).toContain('extensions: ["search","math_mcp"]');
  });

  it('rejects incomplete and duplicate rosters', () => {
    expect(() =>
      buildResearchModelTeamPrompt(
        { mode: 'dual', models: [{ provider: 'codex', model: 'gpt-5.6-sol' }] },
        []
      )
    ).toThrow('requires exactly 2 selected models');

    expect(() =>
      buildResearchModelTeamPrompt(
        {
          mode: 'dual',
          models: [
            { provider: 'codex', model: 'gpt-5.6-sol' },
            { provider: 'codex', model: 'gpt-5.6-sol' },
          ],
        },
        []
      )
    ).toThrow('must be distinct');
  });
});
