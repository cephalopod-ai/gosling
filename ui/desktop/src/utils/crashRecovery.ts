import type { AcpResumeIntegrity } from '../acp/sessions';
import { getTextAndImageContent, type Message } from '../types/message';

export function canSafelyAutoResume(
  resumeIntegrity: AcpResumeIntegrity,
  messages: Message[]
): boolean {
  if (resumeIntegrity !== 'uncertain') return false;

  let latestHumanMessageIndex = -1;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role !== 'user') continue;
    const { textContent, imagePaths } = getTextAndImageContent(message);
    if (textContent.trim() || imagePaths.length > 0) {
      latestHumanMessageIndex = index;
      break;
    }
  }
  if (latestHumanMessageIndex < 0) return false;

  const requestedToolIds = new Set<string>();
  const completedToolIds = new Set<string>();
  for (const message of messages.slice(latestHumanMessageIndex + 1)) {
    for (const content of message.content) {
      switch (content.type) {
        case 'toolRequest':
          requestedToolIds.add(content.id);
          break;
        case 'toolResponse':
          if (content.toolResult.status !== 'success') return false;
          completedToolIds.add(content.id);
          break;
        case 'toolConfirmationRequest':
        case 'actionRequired':
        case 'frontendToolRequest':
          return false;
      }
    }
  }

  return [...requestedToolIds].every((toolId) => completedToolIds.has(toolId));
}
