// Owns remote extension allowlist retrieval and YAML command extraction.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports getAllowList; it re-exports none.

import * as yaml from 'yaml';

/// Bounds on the extension allowlist fetch. It gates what may execute, so it
/// must not be able to hang startup or exhaust memory. (SECN-GSL-002)
const ALLOWLIST_FETCH_TIMEOUT_MS = 10_000;
const ALLOWLIST_MAX_BYTES = 1024 * 1024;

export async function getAllowList(): Promise<string[]> {
  if (!process.env.GOSLING_ALLOWLIST) {
    return [];
  }

  // This fetch decides which extensions may run, so it is a security input.
  // It previously had no scheme check, no timeout, and no size cap: a plain
  // `http://` URL could be rewritten in transit, an unresponsive host hung
  // startup indefinitely, and an oversized body was read whole into the main
  // process. (SECN-GSL-002)
  const allowlistUrl = new URL(process.env.GOSLING_ALLOWLIST);
  if (allowlistUrl.protocol !== 'https:') {
    throw new Error(
      `GOSLING_ALLOWLIST must use https (got ${allowlistUrl.protocol}); the extension allowlist ` +
        'decides what is allowed to run and must not be fetched over a modifiable channel'
    );
  }

  const abort = new AbortController();
  const timeout = setTimeout(() => abort.abort(), ALLOWLIST_FETCH_TIMEOUT_MS);
  let response: Response;
  try {
    response = await fetch(allowlistUrl, { signal: abort.signal });
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    throw new Error(
      `Failed to fetch allowed extensions: ${response.status} ${response.statusText}`
    );
  }

  // Parse the YAML content
  const rawYaml = await response.text();
  if (rawYaml.length > ALLOWLIST_MAX_BYTES) {
    throw new Error(
      `Extension allowlist is larger than ${ALLOWLIST_MAX_BYTES} bytes; refusing to parse it`
    );
  }
  const yamlContent = rawYaml;
  const parsedYaml = yaml.parse(yamlContent);

  // Extract the commands from the extensions array
  if (parsedYaml && parsedYaml.extensions && Array.isArray(parsedYaml.extensions)) {
    const commands = parsedYaml.extensions.map(
      (ext: { id: string; command: string }) => ext.command
    );
    console.log(`Fetched ${commands.length} allowed extension commands`);
    return commands;
  } else {
    console.error('Invalid YAML structure:', parsedYaml);
    return [];
  }
}
