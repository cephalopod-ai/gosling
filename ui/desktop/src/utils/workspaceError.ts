import { errorMessage } from './conversionUtils';

const SECRET_ASSIGNMENT =
  /(["']?(?:api[_-]?key|access[_-]?token|refresh[_-]?token|password|secret|authorization|cookie|value)["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;}\]]+)/gi;
const BEARER_TOKEN = /\bBearer\s+[^\s,;}\]]+/gi;
const PROVIDER_TOKEN = /\bsk-[A-Za-z0-9_-]{8,}\b/g;

export function workspaceErrorMessage(cause: unknown, fallback: string): string {
  return errorMessage(cause, fallback)
    .slice(0, 600)
    .replace(SECRET_ASSIGNMENT, '$1[redacted]')
    .replace(BEARER_TOKEN, 'Bearer [redacted]')
    .replace(PROVIDER_TOKEN, '[redacted]');
}
