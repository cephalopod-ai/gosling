import type { UserInput } from './message';

export type SessionExperience = 'chat' | 'research';

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
  text: string;
  files: ResearchInitialInputFile[];
}

export const MAX_RESEARCH_INITIAL_INPUTS = 16;
export const MAX_RESEARCH_INITIAL_FILE_BYTES = 20 * 1024 * 1024;

export function researchInitialInputCount(inputs: ResearchInitialInputs): number {
  return inputs.files.length + (inputs.text.trim() ? 1 : 0);
}

export function sessionExperienceFrom(value: unknown): SessionExperience {
  return value === 'research' ? 'research' : 'chat';
}
