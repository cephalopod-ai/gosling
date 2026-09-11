import type { Message } from './message';

export type TokenState = {
  accumulatedCacheReadTokens?: number;
  accumulatedCacheWriteTokens?: number;
  accumulatedCost?: number | null;
  accumulatedInputTokens: number;
  accumulatedOutputTokens: number;
  accumulatedTotalTokens: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  contextLimit?: number;
  contextUsageEstimated?: boolean;
  inputTokens: number;
  lastRequestTokens?: number;
  outputTokens: number;
  totalTokens: number;
};

export interface ChatType {
  sessionId: string;
  name: string;
  messages: Message[];
}
