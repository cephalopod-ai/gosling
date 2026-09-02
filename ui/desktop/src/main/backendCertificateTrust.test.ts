import { describe, expect, it } from 'vitest';
import {
  isTrustedHost,
  normalizeFingerprint,
  trustBackendCertificate,
  verifyBackendCertificate,
} from './backendCertificateTrust';

describe('backend certificate trust', () => {
  it('normalizes hexadecimal and sha256 fingerprints', () => {
    expect(normalizeFingerprint('aa:bb')).toBe('AA:BB');
    expect(normalizeFingerprint(`sha256/${Buffer.from([0xaa, 0xbb]).toString('base64')}`)).toBe(
      'AA:BB'
    );
  });

  it('matches hostnames case-insensitively and releases exact pins', () => {
    const registration = trustBackendCertificate('LOCALHOST', 'aa:bb');
    expect(isTrustedHost('localhost')).toBe(true);
    expect(verifyBackendCertificate('localhost', 'AA:BB')).toBe(true);
    expect(verifyBackendCertificate('localhost', 'CC:DD')).toBe(false);
    registration.release();
    expect(isTrustedHost('localhost')).toBe(false);
  });

  it('pins the first TOFU fingerprint', () => {
    const registration = trustBackendCertificate('example.test', null);
    expect(verifyBackendCertificate('example.test', 'AA:BB')).toBe(true);
    expect(verifyBackendCertificate('example.test', 'CC:DD')).toBe(false);
    registration.release();
  });
});
