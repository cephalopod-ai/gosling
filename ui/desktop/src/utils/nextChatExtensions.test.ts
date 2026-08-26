import { describe, expect, it } from 'vitest';
import {
  createNextChatExtensionDraft,
  selectResearchSessionExtensions,
  selectNextChatExtensions,
  toggleNextChatExtension,
} from './nextChatExtensions';
import type { FixedExtensionEntry } from '../components/ConfigContext';

const extension = (name: string, enabled: boolean): FixedExtensionEntry => ({
  name,
  enabled,
  type: 'builtin',
  description: `${name} extension`,
});

describe('nextChatExtensions', () => {
  it('creates a draft from enabled configured extensions', () => {
    const draft = createNextChatExtensionDraft([
      extension('developer', true),
      extension('memory', false),
    ]);

    expect([...draft.selectedNames]).toEqual(['developer']);
  });

  it('toggles selected extension names', () => {
    const draft = createNextChatExtensionDraft([extension('developer', true)]);

    const withoutDeveloper = toggleNextChatExtension(draft, extension('developer', true));
    expect(withoutDeveloper.selectedNames.has('developer')).toBe(false);

    const withMemory = toggleNextChatExtension(withoutDeveloper, extension('memory', false));
    expect([...withMemory.selectedNames]).toEqual(['memory']);
  });

  it('selects extension configs without the enabled field', () => {
    const extensions = [
      { ...extension('developer', true), configKey: 'developer-key' },
      extension('memory', false),
    ];
    const selected = selectNextChatExtensions(extensions, {
      selectedNames: new Set(['memory']),
    });

    expect(selected).toEqual([
      {
        name: 'memory',
        type: 'builtin',
        description: 'memory extension',
      },
    ]);
  });

  it('forces Math MCP into Solo research without changing its global enabled state', () => {
    const math = extension('math_mcp', false);
    const selected = selectResearchSessionExtensions(
      [extension('developer', true), math],
      null,
      'solo'
    );

    expect(selected.extensionConfigs.map(({ name }) => name)).toEqual(['developer', 'math_mcp']);
    expect(selected.missingRequiredNames).toEqual([]);
    expect(math.enabled).toBe(false);
  });

  it('forces Math MCP and summon into multi-model research and reports missing requirements', () => {
    const selected = selectResearchSessionExtensions(
      [extension('developer', true), extension('math_mcp', false)],
      { selectedNames: new Set() },
      'trio'
    );

    expect(selected.extensionConfigs.map(({ name }) => name)).toEqual(['math_mcp']);
    expect(selected.missingRequiredNames).toEqual(['summon']);
  });
});
