import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  defaultShellLocalSettings,
  parseShellLocalSettings,
  readShellLocalSettings,
  writeShellLocalSettings,
} from './localSettings';

describe('shell local settings', () => {
  it('returns bounded defaults when the shell has no settings file', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-settings-'));
    expect(readShellLocalSettings(path.join(root, 'missing.json'))).toEqual(
      defaultShellLocalSettings()
    );
  });

  it('round trips shell-owned settings without credential values', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-settings-'));
    const file = path.join(root, 'shell-settings.json');
    const settings = defaultShellLocalSettings();
    settings.appearance.theme = 'dark';
    settings.appearance.textScale = 1.25;
    settings.workspace.lastWorkingDirectory = path.join(root, 'project');
    settings.workspace.preferredCredentialProfileId = 'profile-reference';

    writeShellLocalSettings(file, settings);

    expect(readShellLocalSettings(file)).toEqual(settings);
    settings.appearance.theme = 'light';
    writeShellLocalSettings(file, settings);
    expect(readShellLocalSettings(file)).toEqual(settings);
    if (process.platform !== 'win32') {
      expect(fs.statSync(file).mode & 0o777).toBe(0o600);
    }
  });

  it('rejects unknown settings, including secret-shaped additions', () => {
    expect(() =>
      parseShellLocalSettings({
        ...defaultShellLocalSettings(),
        apiToken: 'must-not-be-owned-by-the-shell',
      })
    ).toThrow('shell settings must contain only');
  });

  it('requires absolute working directories and opaque credential references', () => {
    const settings = defaultShellLocalSettings();
    settings.workspace.lastWorkingDirectory = '../relative';
    expect(() => parseShellLocalSettings(settings)).toThrow('invalid shell workspace settings');

    settings.workspace.lastWorkingDirectory = null;
    settings.workspace.preferredCredentialProfileId = '';
    expect(() => parseShellLocalSettings(settings)).toThrow('invalid shell workspace settings');
  });

  it('keeps independently rooted shell settings disjoint', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-settings-'));
    const shellA = path.join(root, 'shell-a', 'shell-settings.json');
    const shellB = path.join(root, 'shell-b', 'shell-settings.json');
    const settingsA = defaultShellLocalSettings();
    settingsA.appearance.theme = 'dark';
    const settingsB = defaultShellLocalSettings();
    settingsB.appearance.theme = 'light';

    writeShellLocalSettings(shellA, settingsA);
    writeShellLocalSettings(shellB, settingsB);

    expect(readShellLocalSettings(shellA).appearance.theme).toBe('dark');
    expect(readShellLocalSettings(shellB).appearance.theme).toBe('light');
  });

  it('rejects malformed, oversized, and linked settings files where links are supported', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-settings-'));
    const malformed = path.join(root, 'malformed.json');
    fs.writeFileSync(malformed, '{"schemaVersion":');
    expect(() => readShellLocalSettings(malformed)).toThrow('shell settings are not valid JSON');

    const oversized = path.join(root, 'oversized.json');
    fs.writeFileSync(oversized, ' '.repeat(16 * 1024 + 1));
    expect(() => readShellLocalSettings(oversized)).toThrow('shell settings exceed');

    if (process.platform !== 'win32') {
      const linked = path.join(root, 'linked.json');
      fs.symlinkSync(malformed, linked);
      expect(() => readShellLocalSettings(linked)).toThrow('regular file');
    }
  });
});
