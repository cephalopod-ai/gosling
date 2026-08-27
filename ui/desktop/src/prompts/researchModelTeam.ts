import type { ResearchTeamConfiguration } from '../types/sessionExperience';

export const RESEARCH_MODEL_TEAM_PROMPT_KEY = 'research-model-team';

export function buildResearchModelTeamPrompt(
  configuration: ResearchTeamConfiguration,
  enabledExtensionNames: string[],
  hasInitialInputs: boolean
): string | null {
  if (configuration.mode === 'solo') return null;

  const expectedModels = configuration.mode === 'dual' ? 2 : 3;
  if (configuration.models.length !== expectedModels) {
    throw new Error(
      `${configuration.mode} research requires exactly ${expectedModels} selected models`
    );
  }

  const identities = configuration.models.map(({ provider, model }) => `${provider}/${model}`);
  if (new Set(identities).size !== identities.length) {
    throw new Error('Research team models must be distinct');
  }

  const [lead, ...delegates] = configuration.models;
  const extensionArgument = enabledExtensionNames.length
    ? ` Pass \`extensions: ${JSON.stringify(enabledExtensionNames)}\` to every delegate.`
    : ' No external extensions were selected, so omit the `extensions` argument.';
  const initialInputBrief = hasInitialInputs
    ? ', the relevant Initial Input contents or stable locators'
    : '';
  const delegateRoster = delegates
    .map(
      ({ provider, model }, index) =>
        `${index + 2}. ${provider}/${model} — independent researcher and bounded peer critic.`
    )
    .join('\n');

  return `# Host-selected multi-model research protocol

This session uses a ${configuration.mode} research team selected by the user. The roster is fixed:

1. ${lead.provider}/${lead.model} — lead researcher and final synthesizer (the current session).
${delegateRoster}

Do not select, replace, rank, or silently fall back to other providers or models. If a selected delegate cannot start or fails, record the exact failed seat and reason, continue with the successful seats, and label the result degraded.

## Required workflow

1. Restate the research question, scope, and completion criteria. Preserve the scientific-method and provenance requirements already attached to this session.
2. Before reading any peer result, start one asynchronous \`delegate\` task for each non-lead roster member. Use that member's exact \`provider\` and \`model\`. Give every delegate the complete research brief${initialInputBrief}, the expected report schema, and instructions to research independently. This is an ad-hoc task: pass \`instructions\`, \`provider\`, \`model\`, and \`async: true\`; omit the \`source\` argument entirely instead of sending it as empty or null.${extensionArgument} Validate this exact payload before launch. Do not retry a rejected launch; record that roster seat as failed and continue degraded.
3. While those tasks run, the lead performs its own independent evidence collection and writes a provisional claim/evidence outline. Do not load a delegate result until that outline exists.
4. Load every first-pass result. Keep agreements, contradictions, unsupported assertions, source overlap, and genuinely independent provenance separate.
5. Run exactly one bounded critique round. Start one asynchronous task per non-lead model, again with its exact provider/model, and give it the provisional lead outline plus one peer draft when available. Ask it to identify factual errors, missing evidence, citation problems, and unresolved disagreements; do not ask for a fresh full report. The lead independently critiques the delegate drafts while those tasks run.
6. Load the critiques and synthesize one final report. Resolve disagreements from source evidence when possible. Otherwise preserve the disagreement and explain what evidence would settle it. Never turn model agreement alone into corroboration.
7. End with a compact orchestration record listing every configured seat, which calls completed or failed, any degraded behavior, and which material findings changed after critique.

The parallel first pass protects independence; the single critique round provides useful discussion without an open-ended model conversation. The lead owns the final answer, but must not erase minority findings that remain evidence-backed.`;
}
