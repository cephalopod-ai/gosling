import prompt from './research-scientific-method.md?raw';

export const RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY = 'research-scientific-method';
export const RESEARCH_SCIENTIFIC_METHOD_PROMPT = prompt;

const NO_INITIAL_INPUTS_SCOPE = `# Host-resolved session scope (binding)

No Initial Inputs were supplied for this session. Do not inspect workspace files, search for an
initial-input corpus, or claim that files were reviewed unless the user explicitly supplies or
requests them later. That includes listing the working folder or its subfolders: no tree, ls,
find, glob, grep, or read calls against them. Nothing there is research material, and even a
filename must not enter this investigation. Write only to the Session Outputs and Research
Library folders. Treat Phase 0 and the internal-program portion of Phase 2 as N/A; external
prior-art discovery and the remaining scientific-method requirements still apply.`;

const MATH_MCP_SECTION_HEADING = '## Math MCP equation routing (binding for Deep Research)';
const MATH_MCP_SECTION_END = '## Procedure';

const NO_MATH_MCP_SECTION = `## Equations (no Math MCP in this session)

No Math MCP extension is attached to this session. Do not search for equation-routing tools
or report routing status. Preserve every substantive equation exactly as written, with its
source locator, in the report itself.

`;

function withoutMathMcpRouting(prompt: string): string {
  const start = prompt.indexOf(MATH_MCP_SECTION_HEADING);
  const end = prompt.indexOf(MATH_MCP_SECTION_END, start);
  if (start === -1 || end === -1) return prompt;
  return `${prompt.slice(0, start)}${NO_MATH_MCP_SECTION}${prompt.slice(end)}`;
}

export function buildResearchScientificMethodPrompt(
  hasInitialInputs: boolean,
  hasMathMcp = true
): string {
  const base = hasMathMcp
    ? RESEARCH_SCIENTIFIC_METHOD_PROMPT
    : withoutMathMcpRouting(RESEARCH_SCIENTIFIC_METHOD_PROMPT);
  return hasInitialInputs ? base : `${base}\n\n${NO_INITIAL_INPUTS_SCOPE}`;
}
