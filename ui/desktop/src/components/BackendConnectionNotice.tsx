import { useEffect, useState, useSyncExternalStore } from 'react';
import {
  getAcpClient,
  getAcpConnectionStatus,
  subscribeAcpConnectionStatus,
} from '../acp/acpConnection';
import { defineMessages, useIntl } from '../i18n';
import { Button } from './ui/button';
import { describeAcpError } from '../acp/errors';

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
  const [reconnectError, setReconnectError] = useState<string | null>(null);
  useEffect(() => {
    if (status === 'connected') setReconnectError(null);
  }, [status]);
  if (status !== 'disconnected' && status !== 'reconnecting') return null;

  return (
    <div
      role="alert"
      className="flex flex-wrap items-center justify-center gap-3 p-2 text-text-primary bg-background-secondary"
    >
      <span>{intl.formatMessage(i18n.interrupted)}</span>
      <Button
        variant="outline"
        size="sm"
        disabled={status === 'reconnecting'}
        aria-busy={status === 'reconnecting'}
        onClick={() => {
          setReconnectError(null);
          void getAcpClient().catch((error) => setReconnectError(describeAcpError(error)));
        }}
      >
        {intl.formatMessage(i18n.reconnect)}
      </Button>
      {reconnectError && <p className="w-full text-center">{reconnectError}</p>}
    </div>
  );
}
