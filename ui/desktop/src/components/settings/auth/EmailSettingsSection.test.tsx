import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { EmailRequest, EmailSnapshot } from '../../../types/email';
import EmailSettingsSection from './EmailSettingsSection';

vi.mock('../../ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({ currentProvider: 'ollama', currentModel: 'local-model' }),
}));

const empty: EmailSnapshot = {
  revision: 0,
  account: null,
  activity: [],
  lastStatus: null,
  replyCodeRecipients: [],
};

function bridge() {
  const electron = window.electron;
  const request = vi
    .fn<(request: EmailRequest) => Promise<EmailSnapshot>>()
    .mockResolvedValue(empty);
  window.electron = { ...electron, emailRequest: request };
  return {
    request,
    restore: () => {
      window.electron = electron;
    },
  };
}

describe('Email settings', () => {
  it('saves Gmail POP settings and the private allowlist without starting processing', async () => {
    const { request, restore } = bridge();
    try {
      render(<EmailSettingsSection />);
      await waitFor(() => expect(screen.getByLabelText('Provider')).toBeEnabled());
      fireEvent.change(screen.getByLabelText('Provider'), { target: { value: 'gmail' } });
      fireEvent.change(screen.getByLabelText('Incoming protocol'), { target: { value: 'pop' } });
      fireEvent.change(screen.getByLabelText('Dedicated mailbox address'), {
        target: { value: 'gosling@gmail.com' },
      });
      fireEvent.change(screen.getByLabelText('Google app password'), {
        target: { value: 'test-app-password' },
      });
      expect(screen.getByLabelText('Allowed recipients')).not.toBeVisible();
      fireEvent.click(screen.getByText('Advanced'));
      fireEvent.change(screen.getByLabelText('Allowed recipients'), {
        target: { value: ' owner@example.com\n\n trusted@example.net ' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'Save email settings' }));

      await waitFor(() =>
        expect(request).toHaveBeenCalledWith({
          op: 'save',
          revision: 0,
          incoming_password: 'test-app-password',
          outgoing_password: 'test-app-password',
          settings: expect.objectContaining({
            address: 'gosling@gmail.com',
            protocol: 'pop',
            incoming: {
              host: 'pop.gmail.com',
              port: 995,
              startTls: false,
              username: 'gosling@gmail.com',
            },
            outgoing: {
              host: 'smtp.gmail.com',
              port: 465,
              startTls: false,
              username: 'gosling@gmail.com',
            },
            provider: 'ollama',
            model: 'local-model',
            recipients: ['owner@example.com', 'trusted@example.net'],
          }),
        })
      );
      expect(request.mock.calls.map(([value]) => value.op)).toEqual(['status', 'save']);
    } finally {
      restore();
    }
  });
});
