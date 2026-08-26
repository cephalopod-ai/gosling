import type { ExtensionConfig } from '../types/extensions';
import type { FixedExtensionEntry } from '../components/ConfigContext';
import type { ResearchTeamMode } from '../types/sessionExperience';

export type NextChatExtensionDraft = {
  selectedNames: Set<string>;
};

export type ResearchSessionExtensions = {
  extensionConfigs: ExtensionConfig[];
  missingRequiredNames: string[];
};

function extensionConfig(extension: FixedExtensionEntry): ExtensionConfig {
  const { enabled: _enabled, configKey: _configKey, ...config } = extension;
  return config as ExtensionConfig;
}

export function requiredResearchExtensionNames(mode: ResearchTeamMode): string[] {
  return mode === 'solo' ? ['math_mcp'] : ['math_mcp', 'summon'];
}

export function createNextChatExtensionDraft(
  allExtensions: FixedExtensionEntry[] = []
): NextChatExtensionDraft {
  return {
    selectedNames: new Set(
      allExtensions.filter((extension) => extension.enabled).map((extension) => extension.name)
    ),
  };
}

export function selectNextChatExtensions(
  allExtensions: FixedExtensionEntry[],
  draft: NextChatExtensionDraft
): ExtensionConfig[] {
  return allExtensions
    .filter((extension) => draft.selectedNames.has(extension.name))
    .map(extensionConfig);
}

export function selectResearchSessionExtensions(
  allExtensions: FixedExtensionEntry[],
  draft: NextChatExtensionDraft | null,
  mode: ResearchTeamMode
): ResearchSessionExtensions {
  const requiredNames = requiredResearchExtensionNames(mode);
  const configuredByName = new Map(
    allExtensions.map((extension) => [extension.name, extension] as const)
  );
  const selected = draft
    ? selectNextChatExtensions(allExtensions, draft)
    : allExtensions.filter((extension) => extension.enabled).map(extensionConfig);
  const selectedNames = new Set(selected.map((extension) => extension.name));

  for (const name of requiredNames) {
    const extension = configuredByName.get(name);
    if (extension && !selectedNames.has(name)) {
      selected.push(extensionConfig(extension));
      selectedNames.add(name);
    }
  }

  return {
    extensionConfigs: selected,
    missingRequiredNames: requiredNames.filter((name) => !configuredByName.has(name)),
  };
}

export function isNextChatExtensionSelected(
  extension: FixedExtensionEntry,
  draft: NextChatExtensionDraft
): boolean {
  return draft.selectedNames.has(extension.name);
}

export function toggleNextChatExtension(
  draft: NextChatExtensionDraft,
  extension: FixedExtensionEntry
): NextChatExtensionDraft {
  const selectedNames = new Set(draft.selectedNames);

  if (selectedNames.has(extension.name)) {
    selectedNames.delete(extension.name);
  } else {
    selectedNames.add(extension.name);
  }

  return { selectedNames };
}
