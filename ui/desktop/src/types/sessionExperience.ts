import type { UserInput } from './message';

export type SessionExperience = 'chat' | 'research';

export interface ActiveSessionView {
  sessionId: string;
  initialMessage?: UserInput;
  noAutoSubmit?: boolean;
  sessionExperience: SessionExperience;
}

export const researchInputTags = ['reports', 'links', 'text', 'prompts'] as const;

export function sessionExperienceFrom(value: unknown): SessionExperience {
  return value === 'research' ? 'research' : 'chat';
}
