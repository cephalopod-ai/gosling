import { describeAcpError } from '../acp/errors';
import { errorMessage } from './conversionUtils';

const SECRET_ASSIGNMENT =
  /(["']?(?:api[_-]?key|access[_-]?token|refresh[_-]?token|password|secret|authorization|cookie|value)["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;}\]]+)/gi;
const BEARER_TOKEN = /\bBearer\s+[^\s,;}\]]+/gi;
const PROVIDER_TOKEN = /\bsk-[A-Za-z0-9_-]{8,}\b/g;
const INVALID_PARAMS_CODE = -32602;

export function workspaceErrorMessage(cause: unknown, fallback: string): string {
  return describeWorkspaceError(cause, fallback)
    .slice(0, 600)
    .replace(SECRET_ASSIGNMENT, '$1[redacted]')
    .replace(BEARER_TOKEN, 'Bearer [redacted]')
    .replace(PROVIDER_TOKEN, '[redacted]');
}

function describeWorkspaceError(cause: unknown, fallback: string): string {
  if (typeof cause !== 'object' || cause === null || !('code' in cause) || !('data' in cause)) {
    return errorMessage(cause, fallback);
  }
  // A rejected ACP request carries the backend's reason ("workspace name is already in use")
  // in `data`; its message is only the generic JSON-RPC "Invalid params".
  const { code, data } = cause as { code: unknown; data: unknown };
  if (code === INVALID_PARAMS_CODE && typeof data === 'string' && data.trim()) {
    return data;
  }
  return describeAcpError(cause);
}
