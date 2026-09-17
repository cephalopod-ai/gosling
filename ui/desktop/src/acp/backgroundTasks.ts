import type { Message } from '../types/message';
import { isRecord } from './adapter/shared';

export type BackgroundTaskState =
  | 'running'
  | 'quiet'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'unknown';

export interface BackgroundTaskReference {
  id: string;
  description: string;
  state: BackgroundTaskState;
}

export function toolResultText(value: unknown): string {
  if (!isRecord(value) || !Array.isArray(value.content)) return '';
  return value.content
    .flatMap((block: unknown) =>
      isRecord(block) && block.type === 'text' && typeof block.text === 'string' ? [block.text] : []
    )
    .join('\n');
}

function terminalTaskState(text: string): BackgroundTaskState | null {
  const status = text.match(/^\*\*Status:\*\* (.+)$/m)?.[1];
  if (status === '✓ Completed') return 'completed';
  if (status === '✗ Failed' || status === '✗ Panicked') return 'failed';
  if (status === '⊘ Cancelled') return 'cancelled';
  return null;
}

export function discoverBackgroundTasks(messages: Message[]): BackgroundTaskReference[] {
  const requests = new Map<string, { name: string; args: Record<string, unknown> }>();
  const tasks = new Map<string, BackgroundTaskReference>();

  for (const message of messages) {
    if (message.metadata.importedUntrusted) continue;
    for (const content of message.content) {
      if (content.type === 'toolRequest' && content.toolCall.status === 'success') {
        const call = content.toolCall.value;
        if (!isRecord(call) || typeof call.name !== 'string' || !isRecord(call.arguments)) continue;
        if (content.metadata?.extensionName && content.metadata.extensionName !== 'summon')
          continue;
        const name = call.name.replace(/^summon__/, '');
        if (name === 'delegate' || name === 'load') {
          requests.set(content.id, { name, args: call.arguments });
        }
      }
      if (content.type !== 'toolResponse' || content.toolResult.status !== 'success') continue;
      const request = requests.get(content.id);
      if (!request) continue;
      const text = toolResultText(content.toolResult.value);

      // Only the host's async delegate result introduces a task; prose is not liveness evidence.
      if (request.name === 'delegate' && request.args.async === true) {
        const match = text.match(/^Task (\d{8}_\d+) started in background: "([\s\S]*?)"(?:\n|$)/);
        if (match) {
          tasks.set(match[1], { id: match[1], description: match[2], state: 'running' });
        }
      } else if (request.name === 'load' && typeof request.args.source === 'string') {
        const task = tasks.get(request.args.source);
        const state = terminalTaskState(text);
        if (task && state) tasks.set(task.id, { ...task, state });
      }
    }
  }
  return [...tasks.values()];
}

export function isBackgroundTaskActive(task: BackgroundTaskReference): boolean {
  return task.state === 'running' || task.state === 'quiet' || task.state === 'unknown';
}
