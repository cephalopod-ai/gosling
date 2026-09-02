// Owns backend certificate trust records, TOFU pinning, and session verifier installation.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports the trust helpers; it re-exports none.

import type { Certificate, Session } from 'electron';
import { Buffer } from 'node:buffer';

interface BackendCertificateTrust {
  hostname: string;
  fingerprint: string | null;
}

export interface BackendCertificateTrustRegistration {
  trust: BackendCertificateTrust;
  release: () => void;
}

const trustedBackendCertificates = new Set<BackendCertificateTrust>();
const backendCertificateVerifierSessions = new WeakSet<Session>();

function normalizeHostname(hostname: string): string {
  return hostname.toLowerCase();
}

export function normalizeFingerprint(fp: string): string {
  if (fp.startsWith('sha256/')) {
    const b64 = fp.slice('sha256/'.length);
    const buf = Buffer.from(b64, 'base64');
    return Array.from(buf)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join(':')
      .toUpperCase();
  }
  return fp.toUpperCase();
}

export function trustBackendCertificate(
  hostname: string,
  fingerprint: string | null
): BackendCertificateTrustRegistration {
  const trust: BackendCertificateTrust = {
    hostname: normalizeHostname(hostname),
    fingerprint: fingerprint ? normalizeFingerprint(fingerprint) : null,
  };
  trustedBackendCertificates.add(trust);
  return {
    trust,
    release: () => {
      trustedBackendCertificates.delete(trust);
    },
  };
}

function getBackendCertificateTrusts(hostname: string): BackendCertificateTrust[] {
  const normalizedHostname = normalizeHostname(hostname);
  return [...trustedBackendCertificates].filter((trust) => trust.hostname === normalizedHostname);
}

export function verifyBackendCertificate(hostname: string, fingerprint: string): boolean {
  const normalizedFingerprint = normalizeFingerprint(fingerprint);
  const trusts = getBackendCertificateTrusts(hostname);
  if (trusts.length === 0) {
    return false;
  }

  if (trusts.some((trust) => trust.fingerprint === normalizedFingerprint)) {
    return true;
  }

  const tofuTrust = trusts.find((trust) => trust.fingerprint === null);
  if (tofuTrust) {
    tofuTrust.fingerprint = normalizedFingerprint;
    return true;
  }

  return false;
}

export function isTrustedHost(hostname: string): boolean {
  return getBackendCertificateTrusts(hostname).length > 0;
}

export function installBackendCertificateVerifier(targetSession: Session): void {
  if (backendCertificateVerifierSessions.has(targetSession)) {
    return;
  }

  targetSession.setCertificateVerifyProc((request, callback) => {
    if (!isTrustedHost(request.hostname)) {
      callback(-3);
      return;
    }

    const certificate = request.certificate as Certificate & {
      fingerprint256?: string;
    };
    const fingerprint = certificate.fingerprint256 ?? certificate.fingerprint;
    const match = verifyBackendCertificate(request.hostname, fingerprint);
    callback(match ? 0 : -2);
  });
  backendCertificateVerifierSessions.add(targetSession);
}
