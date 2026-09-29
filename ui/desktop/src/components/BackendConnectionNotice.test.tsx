import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { BackendConnectionNotice } from './BackendConnectionNotice';

const connection = vi.hoisted(() => ({
  status: 'idle',
  listeners: new Set<() => void>(),
  reconnect: vi.fn(),
}));
vi.mock('../acp/acpConnection', () => ({
  getAcpClient: connection.reconnect,
  getAcpConnectionStatus: () => connection.status,
  subscribeAcpConnectionStatus: (listener: () => void) => {
    connection.listeners.add(listener);
    return () => connection.listeners.delete(listener);
  },
}));

function setStatus(status: string) {
  act(() => {
    connection.status = status;
    connection.listeners.forEach((listener) => listener());
  });
}

describe('BackendConnectionNotice', () => {
  beforeEach(() => {
    connection.status = 'idle';
    connection.reconnect.mockReset().mockResolvedValue({});
  });

  it('shows an idle connection loss, offers reconnect and clears after recovery', () => {
    render(<BackendConnectionNotice />, { wrapper: IntlTestWrapper });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    setStatus('connected');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    setStatus('disconnected');
    expect(screen.getByRole('alert')).toHaveTextContent('Connection interrupted');
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
    expect(connection.reconnect).toHaveBeenCalledOnce();
    setStatus('reconnecting');
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeDisabled();
    setStatus('connected');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('keeps the notice available when reconnect fails', async () => {
    connection.status = 'disconnected';
    connection.reconnect.mockRejectedValue(new Error('Offline'));
    render(<BackendConnectionNotice />, { wrapper: IntlTestWrapper });
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Reconnect' })));
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });
});
