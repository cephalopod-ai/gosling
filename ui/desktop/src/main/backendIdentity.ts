import { createHash } from 'node:crypto';
import path from 'node:path';
import { normalizeAcpHttpBaseUrl } from '../acp/url';

/** Backend-owned UI state follows the data store, not an ephemeral port or credential. */
export function backendStorageId(
  backend: { kind: 'local'; dataRoot: string } | { kind: 'external'; baseUrl: string }
): string {
  const identity =
    backend.kind === 'local'
      ? path.resolve(backend.dataRoot)
      : normalizeAcpHttpBaseUrl(backend.baseUrl);
  return `${backend.kind}-${createHash('sha256').update(identity).digest('hex')}`;
}
