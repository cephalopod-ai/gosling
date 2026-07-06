import type { ExternalGoslingdConfig } from './settings';

const DEFAULT_CONNECT_SOURCES = [
  "'self'",
  'https://api.github.com',
  'https://github.com',
  'https://objects.githubusercontent.com',
];

function localAcpConnectSources(acpUrl?: string | null): string[] {
  if (!acpUrl) {
    return [];
  }

  try {
    const parsed = new URL(acpUrl);
    if (!['ws:', 'wss:'].includes(parsed.protocol)) {
      return [];
    }
    if (!['127.0.0.1', 'localhost', '::1', '[::1]'].includes(parsed.hostname)) {
      return [];
    }

    const httpUrl = new URL(parsed.toString());
    httpUrl.protocol = parsed.protocol === 'wss:' ? 'https:' : 'http:';
    return [parsed.origin, httpUrl.origin];
  } catch {
    return [];
  }
}

export function buildConnectSrc(
  externalGoslingd?: ExternalGoslingdConfig,
  localAcpUrl?: string | null
): string {
  const sources = [...DEFAULT_CONNECT_SOURCES];
  sources.push(...localAcpConnectSources(localAcpUrl));

  if (externalGoslingd?.enabled && externalGoslingd.url) {
    try {
      const externalUrl = new URL(externalGoslingd.url);
      sources.push(externalUrl.origin);
      externalUrl.protocol = externalUrl.protocol === 'https:' ? 'wss:' : 'ws:';
      sources.push(externalUrl.origin);
    } catch {
      console.warn('Invalid external goslingd URL in settings, skipping CSP entry');
    }
  }

  return sources.join(' ');
}

export function buildFrameSrc(localAcpUrl?: string | null): string {
  const sources = ["'self'"];
  const localAcpSources = localAcpConnectSources(localAcpUrl);
  const localHttpSource = localAcpSources.find(
    (source) => source.startsWith('http://') || source.startsWith('https://')
  );
  if (localHttpSource) {
    sources.push(localHttpSource);
  }

  return sources.join(' ');
}

/**
 * Returns true when upgrade-insecure-requests should be included in the CSP.
 *
 * The directive is omitted when the user has configured an external backend
 * that uses plain HTTP, because Chromium would silently rewrite those
 * requests to HTTPS. The remote server typically does not speak TLS, so the
 * upgraded requests fail with "Failed to fetch".
 *
 * Loopback addresses (127.0.0.1 / localhost) are exempt from the upgrade
 * per the CSP spec, which is why the built-in local backend is unaffected.
 */
export function shouldUpgradeInsecureRequests(externalGoslingd?: ExternalGoslingdConfig): boolean {
  if (!externalGoslingd?.enabled || !externalGoslingd.url) {
    return true;
  }

  try {
    const parsed = new URL(externalGoslingd.url);
    return parsed.protocol !== 'http:';
  } catch {
    return true;
  }
}

export function buildCSP(
  externalGoslingd?: ExternalGoslingdConfig,
  localAcpUrl?: string | null
): string {
  const connectSrc = buildConnectSrc(externalGoslingd, localAcpUrl);
  const frameSrc = buildFrameSrc(localAcpUrl);
  const upgradeDirective = shouldUpgradeInsecureRequests(externalGoslingd)
    ? 'upgrade-insecure-requests;'
    : '';

  return (
    "default-src 'self';" +
    "style-src 'self' 'unsafe-inline';" +
    "script-src 'self';" +
    "img-src 'self' data: https:;" +
    `connect-src ${connectSrc};` +
    "object-src 'none';" +
    `frame-src ${frameSrc};` +
    "font-src 'self' data: https:;" +
    "media-src 'self' mediastream:;" +
    "form-action 'none';" +
    "base-uri 'self';" +
    "manifest-src 'self';" +
    "worker-src 'self';" +
    upgradeDirective
  );
}
