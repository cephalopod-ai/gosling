import prompt from './research-scientific-method.md?raw';

export const RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY = 'research-scientific-method';
export const RESEARCH_SCIENTIFIC_METHOD_PROMPT = prompt;

const NO_INITIAL_INPUTS_SCOPE = `# Host-resolved session scope (binding)

No Initial Inputs were supplied for this session. Do not inspect workspace files, search for an
initial-input corpus, or claim that files were reviewed unless the user explicitly supplies or
requests them later. Treat Phase 0 and the internal-program portion of Phase 2 as N/A; external
prior-art discovery and the remaining scientific-method requirements still apply.`;

export function buildResearchScientificMethodPrompt(hasInitialInputs: boolean): string {
  return hasInitialInputs
    ? RESEARCH_SCIENTIFIC_METHOD_PROMPT
    : `${RESEARCH_SCIENTIFIC_METHOD_PROMPT}\n\n${NO_INITIAL_INPUTS_SCOPE}`;
}
