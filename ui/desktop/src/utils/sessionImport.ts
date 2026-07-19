import fs from 'node:fs/promises';
import { TextDecoder } from 'node:util';
import { MAX_SESSION_IMPORT_BYTES } from './sessionImportConstants';

export async function readBoundedSessionImportFile(
  filePath: string,
  maxBytes = MAX_SESSION_IMPORT_BYTES
): Promise<string> {
  const handle = await fs.open(filePath, 'r');
  try {
    const metadata = await handle.stat();
    if (metadata.size > maxBytes) {
      throw new Error(`Session import exceeds the ${maxBytes / (1024 * 1024)} MiB limit`);
    }

    const chunks: Buffer[] = [];
    let total = 0;
    while (total <= maxBytes) {
      const buffer = Buffer.allocUnsafe(Math.min(64 * 1024, maxBytes + 1 - total));
      const { bytesRead } = await handle.read(buffer, 0, buffer.length, null);
      if (bytesRead === 0) break;
      chunks.push(buffer.subarray(0, bytesRead));
      total += bytesRead;
    }
    if (total > maxBytes) {
      throw new Error(`Session import exceeds the ${maxBytes / (1024 * 1024)} MiB limit`);
    }

    return new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks, total));
  } finally {
    await handle.close();
  }
}
