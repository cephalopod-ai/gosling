import type { GooseSessionNotification_unstable } from '@aaif/goose-sdk';
import type { SessionNotification } from '@agentclientprotocol/sdk';
import { AppEvents } from '../constants/events';
import { acpChatSessionActions, acpChatSessionStore } from './chatSessionStore';

export function handleAcpSessionNotification(notification: SessionNotification): Promise<void> {
  const sessionNameBeforeNotification = acpChatSessionStore.getSnapshot(
    notification.sessionId
  )?.session?.name;
  const updatedName =
    notification.update.sessionUpdate === 'session_info_update'
      ? notification.update.title
      : undefined;
  acpChatSessionActions.applyAcpSessionNotification(notification);

  if (updatedName && updatedName !== sessionNameBeforeNotification) {
    window.dispatchEvent(
      new CustomEvent(AppEvents.SESSION_RENAMED, {
        detail: { sessionId: notification.sessionId, newName: updatedName },
      })
    );
  }

  return Promise.resolve();
}

export function handleAcpGooseSessionNotification(
  notification: GooseSessionNotification_unstable
): Promise<void> {
  acpChatSessionActions.applyAcpGooseSessionNotification(notification);
  return Promise.resolve();
}
