import type { UserInput } from './message';

export type SessionExperience = 'chat' | 'research';

export type ResearchTeamMode = 'solo' | 'dual' | 'trio';

export interface ResearchModelSelection {
  provider: string;
  model: string;
}

export interface ResearchTeamConfiguration {
  mode: ResearchTeamMode;
  models: ResearchModelSelection[];
}

export function researchTeamSize(mode: ResearchTeamMode): number {
  if (mode === 'trio') return 3;
  if (mode === 'dual') return 2;
  return 1;
}

export interface ActiveSessionView {
  sessionId: string;
  initialMessage?: UserInput;
  noAutoSubmit?: boolean;
  sessionExperience: SessionExperience;
}

export interface ResearchInitialInputFile {
  id: string;
  name: string;
  path: string;
  sizeBytes: number;
}

export interface ResearchInitialInputs {
  texts: string[];
  files: ResearchInitialInputFile[];
}

export const MAX_RESEARCH_INITIAL_INPUTS = 16;
export const MAX_RESEARCH_INITIAL_TEXT_BYTES = 256 * 1024;
export const MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES = 512 * 1024;
export const MAX_RESEARCH_INITIAL_FILE_BYTES = 20 * 1024 * 1024;
export const MAX_RESEARCH_INITIAL_IMAGE_BYTES = 5 * 1024 * 1024;
export const MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES = 10 * 1024 * 1024;

export const RESEARCH_INITIAL_SUPPORTED_EXTENSIONS = [
  'pdf',
  'doc',
  'docx',
  'ppt',
  'pptx',
  'xls',
  'xlsx',
  'png',
  'jpg',
  'jpeg',
  'jpe',
  'jfif',
  'webp',
  'gif',
  'bmp',
  'dib',
  'tif',
  'tiff',
  'ico',
  'tga',
  'pbm',
  'pgm',
  'ppm',
  'pnm',
  'svg',
  'ps',
  'eps',
  'txt',
  'text',
  'md',
  'markdown',
  'rst',
  'rtf',
  'log',
  'csv',
  'tsv',
  'json',
  'jsonl',
  'ndjson',
  'yaml',
  'yml',
  'toml',
  'xml',
  'html',
  'htm',
  'css',
  'ini',
  'cfg',
  'conf',
  'properties',
  'env',
  'tex',
  'bib',
  'rs',
  'js',
  'mjs',
  'cjs',
  'jsx',
  'ts',
  'tsx',
  'py',
  'go',
  'java',
  'c',
  'h',
  'cc',
  'cpp',
  'cxx',
  'hpp',
  'swift',
  'kt',
  'kts',
  'rb',
  'php',
  'sh',
  'bash',
  'zsh',
  'fish',
  'ps1',
  'bat',
  'cmd',
  'sql',
] as const;

export const RESEARCH_INITIAL_FILE_ACCEPT = '*/*';

const RESEARCH_INITIAL_IMAGE_EXTENSIONS = new Set([
  'bmp',
  'dib',
  'gif',
  'ico',
  'jpe',
  'jfif',
  'jpeg',
  'jpg',
  'pbm',
  'pgm',
  'pnm',
  'png',
  'ppm',
  'tga',
  'tif',
  'tiff',
  'webp',
]);

export function researchInitialTextBytes(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

export function isResearchInitialImageFile(fileName: string): boolean {
  const extension = fileName.split('.').pop()?.toLowerCase();
  return extension !== undefined && RESEARCH_INITIAL_IMAGE_EXTENSIONS.has(extension);
}

export function researchInitialInputCount(inputs: ResearchInitialInputs): number {
  return inputs.files.length + inputs.texts.filter((text) => text.trim()).length;
}

export function sessionExperienceFrom(value: unknown): SessionExperience {
  return value === 'research' ? 'research' : 'chat';
}
