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
export const MAX_RESEARCH_INITIAL_FILE_BYTES = 20 * 1024 * 1024;

export function researchInitialInputCount(inputs: ResearchInitialInputs): number {
  return inputs.files.length + inputs.texts.filter((text) => text.trim()).length;
}

export function sessionExperienceFrom(value: unknown): SessionExperience {
  return value === 'research' ? 'research' : 'chat';
}
