import { describe, expect, it } from 'vitest';
import { BackendCertificateTrustRegistry } from './backendCertificateTrust';

describe('BackendCertificateTrustRegistry', () => {
  it('normalizes sha256 fingerprints', () => {
    const registry = new BackendCertificateTrustRegistry();

    expect(registry.normalizeFingerprint('sha256/AQID')).toBe('01:02:03');
  });

  it('pins the first fingerprint for a TOFU registration', () => {
    const registry = new BackendCertificateTrustRegistry();
    const registration = registry.trust('LOCALHOST', null);

    expect(registry.verify('localhost', 'AA:BB')).toBe(true);
    expect(registry.verify('localhost', 'AA:BB')).toBe(true);
    expect(registry.verify('localhost', 'CC:DD')).toBe(false);

    registration.release();
    expect(registry.isTrustedHost('localhost')).toBe(false);
  });
});
