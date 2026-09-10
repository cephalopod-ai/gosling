import type { SessionArtifactDto } from '@repo-makeover/gosling-sdk';
import { describe, expect, it } from 'vitest';
import { coalesceSessionArtifactAliases } from './sessionArtifactAliases';

function artifact(displayPath: string, resolvedPath: string): SessionArtifactDto {
  return {
    sessionId: 'session-1',
    displayPath,
    resolvedPath,
    baseWorkingDir: '/workspace',
    relation: 'referenced',
    provenance: 'assistant_message',
    sourceId: 'message-1',
    firstSeenAt: '2026-09-09T00:00:00Z',
    lastSeenAt: '2026-09-09T00:00:00Z',
  };
}

describe('coalesceSessionArtifactAliases', () => {
  it('folds a bare assistant alias into the qualified output from the same message', () => {
    const output = artifact('Outputs/report.md', '/workspace/Outputs/report.md');
    const missingAlias = artifact('report.md', '/workspace/report.md');

    expect(coalesceSessionArtifactAliases([output, missingAlias])).toEqual({
      artifacts: [output],
      aliases: [{ artifact: missingAlias, target: output }],
    });
  });

  it('keeps same-name files when the assistant named more than one qualified location', () => {
    const first = artifact('first/report.md', '/workspace/first/report.md');
    const second = artifact('second/report.md', '/workspace/second/report.md');
    const bare = artifact('report.md', '/workspace/report.md');

    expect(coalesceSessionArtifactAliases([first, second, bare])).toEqual({
      artifacts: [first, second, bare],
      aliases: [],
    });
  });

  it('does not fold tool-produced artifacts that happen to share a source identifier', () => {
    const output = {
      ...artifact('Outputs/report.md', '/workspace/Outputs/report.md'),
      provenance: 'built_in_tool' as const,
      relation: 'created' as const,
    };
    const rootFile = {
      ...artifact('report.md', '/workspace/report.md'),
      provenance: 'built_in_tool' as const,
      relation: 'created' as const,
    };

    expect(coalesceSessionArtifactAliases([output, rootFile])).toEqual({
      artifacts: [output, rootFile],
      aliases: [],
    });
  });
});
