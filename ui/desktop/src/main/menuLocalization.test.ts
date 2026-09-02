import type { MenuItem } from 'electron';
import { describe, expect, it } from 'vitest';
import { translateMenuLabel, translateMenuLabels } from './menuLocalization';

describe('native menu localization', () => {
  it('translates simplified Chinese locale variants and preserves fallback labels', () => {
    expect(translateMenuLabel('zh-CN', 'Settings')).toBe('设置');
    expect(translateMenuLabel('zh_CN', 'New Chat')).toBe('新建聊天');
    expect(translateMenuLabel('zh-CN', 'Unmapped')).toBe('Unmapped');
  });

  it('does not apply simplified labels to other or traditional Chinese locales', () => {
    expect(translateMenuLabel('en', 'Settings')).toBe('Settings');
    expect(translateMenuLabel('zh-TW', 'Settings')).toBe('Settings');
    expect(translateMenuLabel('zh-Hant', 'Settings')).toBe('Settings');
  });

  it('translates nested menu items', () => {
    const nested = { label: 'Settings' } as MenuItem;
    const items = [{ label: 'File', submenu: { items: [nested] } }] as unknown as MenuItem[];

    translateMenuLabels(items, (label) => translateMenuLabel('zh-CN', label));

    expect(items[0].label).toBe('文件');
    expect(nested.label).toBe('设置');
  });
});
