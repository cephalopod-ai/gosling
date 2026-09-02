/**
 * Owns certificate trust registrations for active desktop backend leases.
 *
 * Extracted from ui/desktop/src/main.ts during behavior-preserving modularization.
 * The Electron entrypoint remains the compatibility facade, owns the app-level
 * certificate-error listener, and delegates per-session verification here.
 */
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

export class BackendCertificateTrustRegistry {
  private readonly trustedCertificates = new Set<BackendCertificateTrust>();
  private readonly verifierSessions = new WeakSet<Session>();

  normalizeFingerprint(fp: string): string {
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

  trust(hostname: string, fingerprint: string | null): BackendCertificateTrustRegistration {
    const trust: BackendCertificateTrust = {
      hostname: this.normalizeHostname(hostname),
      fingerprint: fingerprint ? this.normalizeFingerprint(fingerprint) : null,
    };
    this.trustedCertificates.add(trust);
    return {
      trust,
      release: () => {
        this.trustedCertificates.delete(trust);
      },
    };
  }

  verify(hostname: string, fingerprint: string): boolean {
    const normalizedFingerprint = this.normalizeFingerprint(fingerprint);
    const trusts = this.getTrusts(hostname);
    if (trusts.length === 0) {
      return false;
    }

    if (trusts.some((trust) => trust.fingerprint === normalizedFingerprint)) {
      return true;
    }

    const tofuTrust = trusts.find((trust) => trust.fingerprint === null);
    if (tofuTrust) {
      // TOFU: pin the certificate from the first successful handshake.
      tofuTrust.fingerprint = normalizedFingerprint;
      return true;
    }

    return false;
  }

  isTrustedHost(hostname: string): boolean {
    return this.getTrusts(hostname).length > 0;
  }

  installVerifier(targetSession: Session): void {
    if (this.verifierSessions.has(targetSession)) {
      return;
    }

    targetSession.setCertificateVerifyProc((request, callback) => {
      if (!this.isTrustedHost(request.hostname)) {
        callback(-3);
        return;
      }

      const certificate = request.certificate as Certificate & {
        fingerprint256?: string;
      };
      const fingerprint = certificate.fingerprint256 ?? certificate.fingerprint;
      const match = this.verify(request.hostname, fingerprint);
      callback(match ? 0 : -2);
    });
    this.verifierSessions.add(targetSession);
  }

  private normalizeHostname(hostname: string): string {
    return hostname.toLowerCase();
  }

  private getTrusts(hostname: string): BackendCertificateTrust[] {
    const normalizedHostname = this.normalizeHostname(hostname);
    return [...this.trustedCertificates].filter((trust) => trust.hostname === normalizedHostname);
  }
}
