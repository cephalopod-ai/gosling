import { describe, expect, it } from 'vitest';

import { defaultSettings, resolveStoredSettings } from './settings';

describe('resolveStoredSettings', () => {
  it('migrates the legacy externalGoosed key into externalGoslingd', () => {
    const { settings, migratedLegacyExternalBackend } = resolveStoredSettings({
      externalGoosed: {
        enabled: true,
        url: 'https://example.test',
        secret: 'secret',
        certFingerprint: 'fingerprint',
      },
    });

    expect(migratedLegacyExternalBackend).toBe(true);
    expect(settings.externalGoslingd).toEqual({
      enabled: true,
      url: 'https://example.test',
      secret: 'secret',
      certFingerprint: 'fingerprint',
    });
    expect(Object.hasOwn(settings, 'externalGoosed')).toBe(false);
  });

  it('prefers externalGoslingd when both keys are present', () => {
    const { settings, migratedLegacyExternalBackend } = resolveStoredSettings({
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

    expect(migratedLegacyExternalBackend).toBe(false);
    expect(settings.externalGoslingd).toEqual({
      ...defaultSettings.externalGoslingd,
      enabled: true,
      url: 'https://current.example.test',
      secret: 'current-secret',
    });
  });
});
