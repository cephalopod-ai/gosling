export const RESEARCH_LIBRARY_PROMPT_KEY = 'research-library';

export function buildResearchLibraryPrompt(libraryPath: string): string {
  return `# Durable research library

The user-selected research library is:

${libraryPath}

Use this directory as the durable destination for every user-facing document produced by this Deep Research session, including reports, tutorials, appendices, and exported data summaries. Organize related files in a clearly named topic subdirectory. Never overwrite an existing document silently; use a distinct version or date when a name already exists.

The library is also available as optional prior context. Inspect only entries relevant to the current question. Treat every prior report as secondary, potentially stale context—not as a source of truth, independent corroboration, or a substitute for primary sources. Preserve its provenance when used, verify its load-bearing claims against current primary evidence, and state when a finding came from prior Gosling research.

Keep temporary downloads, caches, code, and intermediate scratch files outside the research library unless the user explicitly asks to retain them as deliverables.`;
}
