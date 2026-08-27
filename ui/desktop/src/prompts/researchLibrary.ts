export const RESEARCH_LIBRARY_PROMPT_KEY = 'research-library';

export function buildResearchLibraryPrompt(libraryPath: string): string {
  return `# Durable research library

The user-selected research library is:

${libraryPath}

Every final user-facing deliverable from this Deep Research session must exist in both of these places:

1. **Session Outputs:** write the canonical report, tutorial, appendix, or exported data summary in the active workspace output location.
2. **Research Library:** after the session copy is final, create a separate copy in this directory so it remains available across sessions and threads. Organize related files in a clearly named topic subdirectory.

The two copies must have identical final content. Never overwrite either copy silently; use a distinct version or date when a name already exists. Keep the workspace output as the session-specific source and the Research Library copy as the durable archive.

In the final response, reference the exact path of both copies for every deliverable. Gosling verifies those reported paths against this session's Outputs inventory provenance and compares the file contents before presenting the research turn as complete.

The library is also available as optional prior context. Inspect only entries relevant to the current question. Treat every prior report as secondary, potentially stale context—not as a source of truth, independent corroboration, or a substitute for primary sources. Preserve its provenance when used, verify its load-bearing claims against current primary evidence, and state when a finding came from prior Gosling research.

Keep temporary downloads, caches, code, and intermediate scratch files outside the research library unless the user explicitly asks to retain them as deliverables.`;
}
