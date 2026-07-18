import type { ProductOutputFolder, ProductType, Workspace } from '@repo-makeover/gosling-sdk';

const EXTENSION_TYPES: Record<string, ProductType> = {
  avi: 'video',
  bmp: 'image',
  c: 'code',
  cc: 'code',
  cpp: 'code',
  cs: 'code',
  csv: 'spreadsheet',
  db: 'data',
  doc: 'document',
  docx: 'document',
  gif: 'image',
  go: 'code',
  graphml: 'data',
  heic: 'image',
  html: 'code',
  java: 'code',
  jpeg: 'image',
  jpg: 'image',
  js: 'code',
  json: 'data',
  jsonl: 'data',
  jsx: 'code',
  key: 'presentation',
  md: 'document',
  mdown: 'document',
  mkv: 'video',
  mov: 'video',
  mp4: 'video',
  odp: 'presentation',
  ods: 'spreadsheet',
  odt: 'document',
  parquet: 'data',
  pdf: 'document',
  png: 'image',
  ppt: 'presentation',
  pptx: 'presentation',
  py: 'code',
  r: 'code',
  rs: 'code',
  rtf: 'document',
  sqlite: 'data',
  svg: 'image',
  swift: 'code',
  tar: 'export',
  tex: 'document',
  tif: 'image',
  tiff: 'image',
  toml: 'code',
  ts: 'code',
  tsv: 'spreadsheet',
  tsx: 'code',
  txt: 'document',
  webm: 'video',
  webp: 'image',
  xls: 'spreadsheet',
  xlsx: 'spreadsheet',
  xml: 'data',
  yaml: 'data',
  yml: 'data',
  zip: 'export',
};

const MIME_TYPES: Record<string, ProductType> = {
  'application/json': 'data',
  'application/pdf': 'document',
  'application/rtf': 'document',
  'application/vnd.ms-excel': 'spreadsheet',
  'application/vnd.ms-powerpoint': 'presentation',
  'application/vnd.openxmlformats-officedocument.presentationml.presentation': 'presentation',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet': 'spreadsheet',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document': 'document',
  'application/zip': 'export',
  'text/csv': 'spreadsheet',
  'text/markdown': 'document',
  'text/plain': 'document',
};

const MIME_EXTENSIONS: Record<string, string> = {
  'application/json': 'json',
  'application/pdf': 'pdf',
  'application/rtf': 'rtf',
  'application/vnd.ms-excel': 'xls',
  'application/vnd.ms-powerpoint': 'ppt',
  'application/vnd.openxmlformats-officedocument.presentationml.presentation': 'pptx',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet': 'xlsx',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document': 'docx',
  'image/gif': 'gif',
  'image/jpeg': 'jpg',
  'image/png': 'png',
  'image/svg+xml': 'svg',
  'image/webp': 'webp',
  'text/csv': 'csv',
  'text/html': 'html',
  'text/markdown': 'md',
  'text/plain': 'txt',
  'video/mp4': 'mp4',
  'video/webm': 'webm',
};

const WINDOWS_RESERVED_NAME = /^(aux|con|nul|prn|com[1-9]|lpt[1-9])(\.|$)/i;
const MAX_FILENAME_BYTES = 180;
const MAX_EXTENSION_BYTES = 20;
const textEncoder = new TextEncoder();

function truncateUtf8(value: string, maxBytes: number): string {
  if (textEncoder.encode(value).length <= maxBytes) return value;

  let result = '';
  let byteLength = 0;
  for (const character of value) {
    const characterBytes = textEncoder.encode(character).length;
    if (byteLength + characterBytes > maxBytes) break;
    result += character;
    byteLength += characterBytes;
  }
  return result;
}

export function safeArtifactFileName(name: string): string {
  const leaf = name.split(/[\\/]/).pop() ?? '';
  const withoutControlCharacters = Array.from(leaf, (character) =>
    character.charCodeAt(0) <= 31 ? '-' : character
  ).join('');
  const sanitized = withoutControlCharacters
    .replace(/[<>:"|?*]/g, '-')
    .replace(/[. ]+$/g, '')
    .trim();
  if (!sanitized || sanitized === '.' || sanitized === '..') return 'artifact';
  const portableName = WINDOWS_RESERVED_NAME.test(sanitized) ? `_${sanitized}` : sanitized;
  if (textEncoder.encode(portableName).length <= MAX_FILENAME_BYTES) return portableName;

  const extensionIndex = portableName.lastIndexOf('.');
  const extension =
    extensionIndex > 0 ? truncateUtf8(portableName.slice(extensionIndex), MAX_EXTENSION_BYTES) : '';
  const stem = extension ? portableName.slice(0, extensionIndex) : portableName;
  const stemBudget = MAX_FILENAME_BYTES - textEncoder.encode(extension).length;
  return `${truncateUtf8(stem, stemBudget)}${extension}`;
}

function extensionFromName(name: string): string | null {
  const leaf = name.split(/[\\/]/).pop() ?? name;
  const extension = leaf.includes('.') ? leaf.split('.').pop()?.toLowerCase() : null;
  return extension || null;
}

export function inferArtifactProductType(input: {
  mimeType?: string;
  productType?: ProductType;
  suggestedName: string;
}): ProductType {
  if (input.productType) return input.productType;
  const extension = extensionFromName(input.suggestedName);
  if (extension && EXTENSION_TYPES[extension]) return EXTENSION_TYPES[extension];
  const mimeType = input.mimeType?.split(';', 1)[0]?.trim().toLowerCase();
  if (mimeType) {
    if (mimeType.startsWith('image/')) return 'image';
    if (mimeType.startsWith('video/')) return 'video';
    const mappedMimeType = MIME_TYPES[mimeType];
    if (mappedMimeType) return mappedMimeType;
  }
  return 'other';
}

export function suggestedArtifactFileName(name: string, mimeType?: string): string {
  const safeName = safeArtifactFileName(name);
  if (extensionFromName(safeName)) return safeName;
  const normalizedMimeType = mimeType?.split(';', 1)[0]?.trim().toLowerCase();
  const extension = normalizedMimeType ? MIME_EXTENSIONS[normalizedMimeType] : undefined;
  return extension ? `${safeName}.${extension}` : safeName;
}

export function selectArtifactOutput<
  T extends Pick<ProductOutputFolder, 'isDefault' | 'productTypes'>,
>(outputs: T[], productType: ProductType): T | null {
  return (
    outputs.find((output) => output.productTypes.includes(productType)) ??
    outputs.find((output) => output.isDefault) ??
    outputs[0] ??
    null
  );
}

export function joinArtifactPath(directory: string, fileName: string): string {
  const safeName = safeArtifactFileName(fileName);
  if (!directory) return safeName;
  const separator = directory.includes('\\') && !directory.includes('/') ? '\\' : '/';
  return `${directory.replace(/[\\/]+$/g, '')}${separator}${safeName}`;
}

export function resolveWorkspaceArtifact(
  workspace: Workspace,
  input: { mimeType?: string; productType?: ProductType; suggestedName: string }
): {
  defaultPath: string;
  output: ProductOutputFolder | null;
  productType: ProductType;
} {
  const productType = inferArtifactProductType(input);
  const output = selectArtifactOutput(workspace.productOutputFolders, productType);
  const fileName = suggestedArtifactFileName(input.suggestedName, input.mimeType);
  return {
    defaultPath: output ? joinArtifactPath(output.path, fileName) : fileName,
    output,
    productType,
  };
}
