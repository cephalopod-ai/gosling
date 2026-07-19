import { describe, expect, it } from 'vitest';

import {
  defaultSettings,
  isSettingKey,
  isSettingValue,
  resolveStoredSettings,
  settingKeys,
} from './settings';

describe('resolveStoredSettings', () => {
  it('migrates the legacy externalGoosed key without retaining its secret', () => {
    const result = resolveStoredSettings({
      externalGoosed: {
        enabled: true,
        url: 'https://example.test',
        secret: 'SENTINEL_EXTERNAL_SECRET',
        certFingerprint: 'fingerprint',
      },
    });

    expect(result.migratedLegacyExternalBackend).toBe(true);
    expect(result.removedPersistedExternalSecret).toBe(true);
    expect(result.legacyExternalBackendSecret).toBe('SENTINEL_EXTERNAL_SECRET');
    expect(result.settings.externalGoslingd).toEqual({
      enabled: true,
      url: 'https://example.test',
      secret: '',
      secretConfigured: true,
      certFingerprint: 'fingerprint',
    });
    expect(JSON.stringify(result.settings)).not.toContain('SENTINEL_EXTERNAL_SECRET');
  });

  it('prefers externalGoslingd when both external backend keys are present', () => {
    const result = resolveStoredSettings({
      externalGoosed: {
        enabled: true,
        url: 'https://legacy.example.test',
        secret: 'legacy-secret',
      },
      externalGoslingd: {
        enabled: true,
        url: 'https://current.example.test',
        secret: 'current-secret',
      },
    });

    expect(result.migratedLegacyExternalBackend).toBe(false);
    expect(result.legacyExternalBackendSecret).toBe('current-secret');
    expect(result.settings.externalGoslingd).toEqual({
      ...defaultSettings.externalGoslingd,
      enabled: true,
      url: 'https://current.example.test',
      secretConfigured: true,
    });
  });

  it('removes legacy plaintext secret profiles from the settings contract', () => {
    const result = resolveStoredSettings({
      managedSecretProfiles: [
        {
          id: 'profile-1',
          entries: [{ key: 'TOKEN', value: 'SENTINEL_PROFILE_SECRET' }],
        },
      ],
    });

    expect(result.removedLegacyManagedSecretProfiles).toBe(true);
    expect(JSON.stringify(result.settings)).not.toContain('SENTINEL_PROFILE_SECRET');
    expect(result.settings).not.toHaveProperty('managedSecretProfiles');
  });
});

describe('setting IPC schemas', () => {
  it('keeps the runtime key list in sync with Settings', () => {
    expect(settingKeys.every(isSettingKey)).toBe(true);
    expect(isSettingKey('__proto__')).toBe(false);
  });

  it('rejects malformed values for every structured or bounded setting', () => {
    expect(isSettingValue('showDockIcon', 'yes')).toBe(false);
    expect(isSettingValue('theme', 'system')).toBe(false);
    expect(isSettingValue('archiveFolder', 'x'.repeat(4097))).toBe(false);
    expect(isSettingValue('seenAnnouncementIds', new Array(1001).fill('id'))).toBe(false);
    expect(isSettingValue('archivedSessionFiles', { session: 42 })).toBe(false);
    expect(
      isSettingValue('externalGoslingd', {
        enabled: true,
        url: 'https://example.test',
        secret: '',
        unexpected: true,
      })
    ).toBe(false);
    expect(isSettingValue('keyboardShortcuts', { focusWindow: null })).toBe(false);
  });
});
