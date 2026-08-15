import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  createShellSettingsStore,
  defaultShellLocalSettings,
  loadShellLocalSettings,
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

describe('shell local settings recovery and store', () => {
  const temporaryRoot = () =>
    fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-settings-recovery-'));

  it('reports each unreadable document class without repairing the bytes', () => {
    const root = temporaryRoot();
    const missing = path.join(root, 'missing.json');
    expect(loadShellLocalSettings(missing)).toEqual({
      settings: defaultShellLocalSettings(),
      recovery: { status: 'absent', schemaVersion: null },
    });

    const malformed = path.join(root, 'malformed.json');
    fs.writeFileSync(malformed, '{ not json');
    expect(loadShellLocalSettings(malformed).recovery).toEqual({
      status: 'malformed',
      schemaVersion: null,
    });
    expect(fs.readFileSync(malformed, 'utf8')).toBe('{ not json');

    const future = path.join(root, 'future.json');
    fs.writeFileSync(future, JSON.stringify({ schemaVersion: 99, appearance: {}, workspace: {} }));
    expect(loadShellLocalSettings(future).recovery).toEqual({
      status: 'unsupported_schema',
      schemaVersion: 99,
    });
    expect(loadShellLocalSettings(future).settings).toEqual(defaultShellLocalSettings());

    const oversized = path.join(root, 'oversized.json');
    fs.writeFileSync(oversized, 'x'.repeat(17 * 1024));
    expect(loadShellLocalSettings(oversized).recovery.status).toBe('malformed');
  });

  it('accepts a current-schema document as loaded', () => {
    const root = temporaryRoot();
    const file = path.join(root, 'shell-settings.json');
    const settings = defaultShellLocalSettings();
    settings.appearance.theme = 'dark';
    writeShellLocalSettings(file, settings);

    expect(loadShellLocalSettings(file)).toEqual({
      settings,
      recovery: { status: 'loaded', schemaVersion: 1 },
    });
  });

  it('exposes one operation per allowlisted field and no arbitrary key writer', () => {
    const root = temporaryRoot();
    const file = path.join(root, 'shell-settings.json');
    const store = createShellSettingsStore(file);

    expect(Object.keys(store).sort()).toEqual([
      'read',
      'recovery',
      'reset',
      'setAppearance',
      'setLastWorkingDirectory',
      'setPreferredCredentialProfileId',
    ]);

    store.setAppearance({ theme: 'dark', textScale: 1.5 });
    store.setLastWorkingDirectory(root);
    const written = store.setPreferredCredentialProfileId('profile-reference');

    expect(written).toEqual({
      schemaVersion: 1,
      appearance: { theme: 'dark', textScale: 1.5 },
      workspace: { lastWorkingDirectory: root, preferredCredentialProfileId: 'profile-reference' },
    });
    expect(readShellLocalSettings(file)).toEqual(written);
    expect(fs.statSync(file).mode & 0o777).toBe(0o600);
  });

  it('refuses to overwrite a document it could not parse until reset is explicit', () => {
    const root = temporaryRoot();
    const file = path.join(root, 'shell-settings.json');
    fs.writeFileSync(file, '{ not json');
    const store = createShellSettingsStore(file);

    expect(store.recovery().status).toBe('malformed');
    expect(() => store.setLastWorkingDirectory(root)).toThrow('recovery state');
    expect(fs.readFileSync(file, 'utf8')).toBe('{ not json');

    store.reset();

    expect(store.recovery()).toEqual({ status: 'loaded', schemaVersion: 1 });
    expect(readShellLocalSettings(file)).toEqual(defaultShellLocalSettings());
  });

  it('keeps two product identities isolated from each other', () => {
    const root = temporaryRoot();
    const first = createShellSettingsStore(path.join(root, 'shell-a', 'shell-settings.json'));
    const second = createShellSettingsStore(path.join(root, 'shell-b', 'shell-settings.json'));

    first.setLastWorkingDirectory(path.join(root, 'a-project'));
    second.setLastWorkingDirectory(path.join(root, 'b-project'));

    expect(first.read().workspace.lastWorkingDirectory).toBe(path.join(root, 'a-project'));
    expect(second.read().workspace.lastWorkingDirectory).toBe(path.join(root, 'b-project'));
  });
});
