import type { SessionArtifactDto } from '@repo-makeover/gosling-sdk';

export interface SessionArtifactAlias {
  artifact: SessionArtifactDto;
  target: SessionArtifactDto;
}

export interface CoalescedSessionArtifacts {
  aliases: SessionArtifactAlias[];
  artifacts: SessionArtifactDto[];
}

function fileName(displayPath: string): string {
  return displayPath.split(/[\\/]/).pop() ?? displayPath;
}

function isQualifiedPath(displayPath: string): boolean {
  return displayPath !== fileName(displayPath);
}

function aliasKey(artifact: SessionArtifactDto): string | null {
  if (
    !artifact.sourceId ||
    artifact.provenance !== 'assistant_message' ||
    artifact.relation !== 'referenced'
  ) {
    return null;
  }
  return `${artifact.sessionId}\0${artifact.sourceId}\0${fileName(artifact.displayPath)}`;
}

export function coalesceSessionArtifactAliases(
  artifacts: SessionArtifactDto[]
): CoalescedSessionArtifacts {
  const qualified = new Map<string, { ambiguous: boolean; artifact: SessionArtifactDto }>();

  for (const artifact of artifacts) {
    const key = aliasKey(artifact);
    if (!key || !isQualifiedPath(artifact.displayPath)) continue;
    const current = qualified.get(key);
    if (!current) {
      qualified.set(key, { ambiguous: false, artifact });
    } else if (current.artifact.resolvedPath !== artifact.resolvedPath) {
      current.ambiguous = true;
    }
  }

  const aliases: SessionArtifactAlias[] = [];
  const coalesced = artifacts.filter((artifact) => {
    const key = aliasKey(artifact);
    const preferred = key ? qualified.get(key) : undefined;
    if (
      !preferred ||
      preferred.ambiguous ||
      isQualifiedPath(artifact.displayPath) ||
      preferred.artifact.resolvedPath === artifact.resolvedPath
    ) {
      return true;
    }
    aliases.push({ artifact, target: preferred.artifact });
    return false;
  });

  return { aliases, artifacts: coalesced };
}
