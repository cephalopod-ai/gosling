import { useSyncExternalStore } from 'react';
import {
  getAcpClient,
  getAcpConnectionStatus,
  subscribeAcpConnectionStatus,
} from '../acp/acpConnection';
import { defineMessages, useIntl } from '../i18n';
import { Button } from './ui/button';

const i18n = defineMessages({
  interrupted: {
    id: 'baseChat.connectionInterrupted',
    defaultMessage: 'Connection interrupted',
  },
  reconnect: {
    id: 'baseChat.reconnect',
    defaultMessage: 'Reconnect',
  },
});

export function BackendConnectionNotice() {
  const intl = useIntl();
  const status = useSyncExternalStore(subscribeAcpConnectionStatus, getAcpConnectionStatus);
  if (status !== 'disconnected' && status !== 'reconnecting') return null;

  return (
    <div
      role="alert"
      className="flex items-center justify-center gap-3 p-2 text-text-primary bg-background-secondary"
    >
      <span>{intl.formatMessage(i18n.interrupted)}</span>
      <Button
        variant="outline"
        size="sm"
        disabled={status === 'reconnecting'}
        aria-busy={status === 'reconnecting'}
        onClick={() => void getAcpClient().catch(() => {})}
      >
        {intl.formatMessage(i18n.reconnect)}
      </Button>
    </div>
  );
}
