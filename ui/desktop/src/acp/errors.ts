export interface AcpCreditsExhaustedError {
  message: string;
  url?: string;
}

const CREDITS_EXHAUSTED_REASON = 'credits_exhausted';

export function parseAcpCreditsExhaustedError(error: unknown): AcpCreditsExhaustedError | null {
  const jsonRpcError = asAcpJsonRpcError(error);
  if (!jsonRpcError) {
    return null;
  }

  const { data } = jsonRpcError;
  if (!isRecord(data) || data.reason !== CREDITS_EXHAUSTED_REASON) {
    return null;
  }

  const url = typeof data.url === 'string' ? data.url : undefined;

  return {
    message: jsonRpcError.message,
    ...(url ? { url } : {}),
  };
}

/**
 * Renders an ACP JSON-RPC error the way the Rust `Display for Error` impl does:
 * the message, plus the `data` payload (often the real underlying cause, e.g. an
 * anyhow error string) when present. Without this, generic errors like "Internal
 * error" reach the UI with no way to tell what actually failed.
 */
export function describeAcpError(error: unknown): string {
  const jsonRpcError = asAcpJsonRpcError(error);
  if (!jsonRpcError) {
    return asErrorMessage(error);
  }

  const { message, data } = jsonRpcError;
  if (data === undefined || data === null) {
    return message;
  }

  const detail = typeof data === 'string' ? data : JSON.stringify(data);
  return detail && detail !== message ? `${message}: ${detail}` : message;
}

export function isAcpConnectionClosedError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : asErrorMessage(error);
  return /ACP connection closed|ACP WebSocket connection failed|WebSocket.*(?:closed|reset|failed)|Not connected/i.test(
    message
  );
}

interface AcpJsonRpcError {
  message: string;
  data?: unknown;
}

function asAcpJsonRpcError(error: unknown): AcpJsonRpcError | null {
  if (!isRecord(error)) {
    return null;
  }

  const candidate = isRecord(error.error) ? error.error : error;
  if (typeof candidate.message !== 'string') {
    return null;
  }

  return {
    message: candidate.message,
    data: candidate.data,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function asErrorMessage(error: unknown): string {
  if (isRecord(error)) {
    const candidate = isRecord(error.error) ? error.error : error;
    if (typeof candidate.message === 'string') {
      return candidate.message;
    }
  }
  return String(error);
}
