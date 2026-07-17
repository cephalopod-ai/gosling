import type { ArtifactKind } from './types';

const KIND_BY_EXTENSION: Record<string, ArtifactKind> = {
  csv: 'csv',
  gif: 'image',
  graphml: 'graphml',
  htm: 'html',
  html: 'html',
  jpeg: 'image',
  jpg: 'image',
  json: 'json',
  jsonl: 'jsonl',
  md: 'markdown',
  markdown: 'markdown',
  pdf: 'pdf',
  png: 'image',
  svg: 'svg',
  txt: 'text',
  webp: 'image',
};

const FILE_ARGUMENT_KEYS = new Set([
  'artifact',
  'artifacts',
  'destination',
  'destination_path',
  'destinations',
  'deliverable',
  'deliverables',
  'file',
  'file_path',
  'files',
  'output',
  'output_file',
  'output_files',
  'output_path',
  'output_paths',
  'outputs',
  'path',
  'paths',
  'source',
  'source_path',
  'sources',
  'uri',
  'uris',
]);

function normalizeArgumentKey(key: string): string {
  return key.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
}

export function localFilePathFromUri(value: string): string | null {
  const candidate = value.trim();
  if (!candidate) return null;
  if (!/^[a-z][a-z0-9+.-]*:/i.test(candidate)) return candidate;
  if (!candidate.toLowerCase().startsWith('file:')) return null;

  try {
    const url = new URL(candidate);
    const decodedPath = decodeURIComponent(url.pathname);
    if (url.host) return `//${url.host}${decodedPath}`;
    return /^\/[a-z]:\//i.test(decodedPath) ? decodedPath.slice(1) : decodedPath;
  } catch {
    return null;
  }
}

export function viewableFilePathsFromToolArguments(argumentsValue: unknown): string[] {
  const paths = new Set<string>();

  const visit = (value: unknown, isFileValue: boolean) => {
    if (typeof value === 'string') {
      if (!isFileValue) return;
      const localPath = localFilePathFromUri(value);
      if (localPath && artifactKindFromPath(localPath) !== 'unknown') paths.add(localPath);
      return;
    }
    if (Array.isArray(value)) {
      value.forEach((item) => visit(item, isFileValue));
      return;
    }
    if (!value || typeof value !== 'object') return;
    Object.entries(value).forEach(([key, nestedValue]) =>
      visit(nestedValue, FILE_ARGUMENT_KEYS.has(normalizeArgumentKey(key)))
    );
  };

  visit(argumentsValue, false);
  return [...paths];
}

export function artifactKindFromPath(path: string): ArtifactKind {
  const extension = path.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase();
  return extension ? (KIND_BY_EXTENSION[extension] ?? 'unknown') : 'unknown';
}

export function artifactKindFromMimeType(mimeType: string): ArtifactKind {
  const normalized = mimeType.toLowerCase();
  if (normalized === 'text/markdown') return 'markdown';
  if (normalized === 'text/csv') return 'csv';
  if (normalized === 'application/json') return 'json';
  if (normalized === 'text/html') return 'html';
  if (normalized === 'image/svg+xml') return 'svg';
  if (normalized.startsWith('image/')) return 'image';
  if (normalized.startsWith('text/')) return 'text';
  return 'unknown';
}

export function artifactTitleFromPath(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function parseCsv(content: string, maxRows = 200, maxColumns = 50): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let quoted = false;

  for (let index = 0; index < content.length && rows.length < maxRows; index += 1) {
    const character = content[index];
    if (quoted) {
      if (character === '"' && content[index + 1] === '"') {
        field += '"';
        index += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        field += character;
      }
      continue;
    }
    if (character === '"') {
      quoted = true;
    } else if (character === ',') {
      if (row.length < maxColumns) row.push(field);
      field = '';
    } else if (character === '\n') {
      if (row.length < maxColumns) row.push(field.replace(/\r$/, ''));
      rows.push(row);
      row = [];
      field = '';
    } else {
      field += character;
    }
  }
  if ((field || row.length) && rows.length < maxRows) {
    if (row.length < maxColumns) row.push(field.replace(/\r$/, ''));
    rows.push(row);
  }
  return rows;
}

export function addSandboxCsp(html: string): string {
  const policy =
    "default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'; script-src 'unsafe-inline' blob:; font-src data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
  const meta = `<meta http-equiv="Content-Security-Policy" content="${policy}">`;
  if (/<head[\s>]/i.test(html)) {
    return html.replace(/<head([^>]*)>/i, `<head$1>${meta}`);
  }
  return `<!doctype html><html><head>${meta}</head><body>${html}</body></html>`;
}
