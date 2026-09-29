import { acpHttpUrlFromHttpBase, statusHttpUrlFromHttpBase } from './acp/url';

const HEALTHCHECK_TIMEOUT_MS = 30000;
const HEALTHCHECK_INTERVAL_MS = 100;
const PROBE_TIMEOUT_MS = 1000;

type FetchInput = Parameters<typeof globalThis.fetch>[0];
type FetchInit = Parameters<typeof globalThis.fetch>[1];

export interface CheckServerStatusOptions {
  onEvent?: (name: string, details?: Record<string, unknown>) => void;
}

export interface CheckBackendStatusParams {
  baseUrl: string;
  serverSecret: string;
  fetch: typeof globalThis.fetch;
  errorLog?: string[];
  options?: CheckServerStatusOptions;
}

export type BackendStatus = { ready: true } | { ready: false; detail: string };

const UNREACHABLE_DETAIL =
  'The backend did not respond. Check that it is running and that the URL and port are correct.';

export const isFatalError = (line: string): boolean => {
  const fatalPatterns = [/panicked at/, /RUST_BACKTRACE/, /fatal error/i];
  return fatalPatterns.some((pattern) => pattern.test(line));
};

const delay = (timeoutMs: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, timeoutMs));

const fetchWithTimeout = async (
  fetch: typeof globalThis.fetch,
  input: FetchInput,
  init?: FetchInit
): Promise<Response> => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), PROBE_TIMEOUT_MS);

  try {
    return await fetch(input, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timeout);
  }
};

export const getBackendStatus = async ({
  baseUrl,
  serverSecret,
  fetch,
  errorLog = [],
  options = {},
}: CheckBackendStatusParams): Promise<BackendStatus> => {
  const deadline = Date.now() + HEALTHCHECK_TIMEOUT_MS;
  const statusUrl = statusHttpUrlFromHttpBase(baseUrl);
  const acpUrl = acpHttpUrlFromHttpBase(baseUrl);
  options.onEvent?.('healthcheck_start', {
    timeoutMs: HEALTHCHECK_TIMEOUT_MS,
    intervalMs: HEALTHCHECK_INTERVAL_MS,
  });

  let attempt = 1;
  let failureDetail = UNREACHABLE_DETAIL;
  while (Date.now() < deadline) {
    if (errorLog.some(isFatalError)) {
      options.onEvent?.('healthcheck_fatal_error', { attempt });
      return { ready: false, detail: 'The backend stopped with a fatal error. Check its logs.' };
    }

    try {
      const response = await fetchWithTimeout(fetch, statusUrl, {
        headers: {
          'X-Secret-Key': serverSecret,
        },
      });
      if (response.status === 401 || response.status === 403) {
        options.onEvent?.('healthcheck_auth_failed', { attempt });
        return {
          ready: false,
          detail:
            'Authentication was rejected. Check the configured backend secret and access rules.',
        };
      }
      if (response.ok) {
        const authResponse = await fetchWithTimeout(fetch, acpUrl, {
          headers: {
            'X-Secret-Key': serverSecret,
          },
        });
        // GET /acp without an SSE Accept header returns 406 after auth succeeds.
        if (authResponse.status === 406) {
          options.onEvent?.('healthcheck_success', { attempt });
          return { ready: true };
        }
        if (authResponse.status === 401 || authResponse.status === 403) {
          options.onEvent?.('healthcheck_auth_failed', { attempt });
          return {
            ready: false,
            detail:
              'Authentication was rejected. Check the configured backend secret and access rules.',
          };
        }
        failureDetail = `The ACP endpoint returned HTTP ${authResponse.status}; expected HTTP 406. Check the backend base URL and ACP server version.`;
      } else {
        failureDetail = `The status endpoint returned HTTP ${response.status}. Check the backend base URL and server health.`;
      }
    } catch (error) {
      // Electron reports TLS failures as network errors; expose the category, never raw details
      // that may contain credentials or other server-supplied content.
      if (
        error instanceof Error &&
        /certificate|\bTLS\b|\bSSL\b|ERR_CERT|ERR_SSL/i.test(error.message)
      ) {
        options.onEvent?.('healthcheck_tls_failed', { attempt });
        return {
          ready: false,
          detail:
            'TLS negotiation failed. Check the HTTPS URL, server certificate and configured fingerprint.',
        };
      }
      failureDetail = UNREACHABLE_DETAIL;
    }

    await delay(HEALTHCHECK_INTERVAL_MS);
    attempt += 1;
  }

  options.onEvent?.('healthcheck_timeout', { timeoutMs: HEALTHCHECK_TIMEOUT_MS });
  return { ready: false, detail: failureDetail };
};

export const checkBackendStatus = async (params: CheckBackendStatusParams): Promise<boolean> =>
  (await getBackendStatus(params)).ready;
