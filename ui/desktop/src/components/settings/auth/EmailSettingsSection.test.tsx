import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import type { EmailRequest, EmailSnapshot } from '../../../types/email';
import EmailSettingsSection from './EmailSettingsSection';

vi.mock('../models/subcomponents/SwitchModelModal', () => ({
  SwitchModelModal: ({
    onModelSelected,
    onClose,
  }: {
    onModelSelected: (model: string, provider: string) => void;
    onClose: () => void;
  }) => (
    <button
      type="button"
      onClick={() => {
        onModelSelected('local-model', 'ollama');
        onClose();
      }}
    >
      Select model
    </button>
  ),
}));

const empty: EmailSnapshot = {
  revision: 0,
  account: null,
  activity: [],
  lastStatus: null,
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
  it('saves Gmail POP settings, its own model, instructions, and independent allowlists', async () => {
    const { request, restore } = bridge();
    try {
      render(<EmailSettingsSection setView={vi.fn()} />, { wrapper: IntlTestWrapper });
      await waitFor(() => expect(screen.getByLabelText('Provider')).toBeEnabled());
      fireEvent.change(screen.getByLabelText('Provider'), { target: { value: 'gmail' } });
      fireEvent.change(screen.getByLabelText('Incoming protocol'), { target: { value: 'pop' } });
      fireEvent.change(screen.getByLabelText('Dedicated mailbox address'), {
        target: { value: 'gosling@gmail.com' },
      });
      fireEvent.change(screen.getByLabelText('Google app password'), {
        target: { value: 'test-app-password' },
      });
      expect(screen.getByRole('button', { name: 'Save email settings' })).toBeDisabled();
      fireEvent.click(screen.getByRole('button', { name: 'Choose email model' }));
      fireEvent.click(screen.getByRole('button', { name: 'Select model' }));
      expect(screen.getByText('ollama / local-model')).toBeVisible();

      const instructions = 'File receipts in Receipts.\nKeep replies short.';
      fireEvent.click(screen.getByRole('button', { name: 'Instructions' }));
      fireEvent.change(screen.getByRole('textbox', { name: 'Email instructions' }), {
        target: { value: instructions },
      });
      fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
      fireEvent.click(screen.getByRole('button', { name: 'Instructions' }));
      expect(screen.getByRole('textbox', { name: 'Email instructions' })).toHaveValue(instructions);
      fireEvent.change(screen.getByRole('textbox', { name: 'Email instructions' }), {
        target: { value: 'Discard this edit.' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

      expect(screen.getByLabelText('Allowed senders')).not.toBeVisible();
      fireEvent.click(screen.getByText('Receive allowlist'));
      fireEvent.change(screen.getByLabelText('Allowed senders'), {
        target: { value: ' incoming@example.org\n\n owner@example.com ' },
      });
      expect(screen.getByLabelText('Allowed recipients')).not.toBeVisible();
      fireEvent.click(screen.getByText('Send allowlist'));
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
            instructions,
            senders: ['incoming@example.org', 'owner@example.com'],
            recipients: ['owner@example.com', 'trusted@example.net'],
          }),
        })
      );
      expect(request.mock.calls.map(([value]) => value.op)).toEqual(['status', 'save']);
    } finally {
      restore();
    }
  });

  it('allows inbox processing after an SMTP failure while keeping replies disabled', async () => {
    const { request, restore } = bridge();
    const endpoint = { host: 'localhost', port: 993, startTls: false, username: '' };
    const account: NonNullable<EmailSnapshot['account']> = {
      enabled: false,
      tested: false,
      smtpTested: false,
      hasCredentials: true,
      settings: {
        address: 'gosling@example.com',
        protocol: 'imap',
        incoming: endpoint,
        outgoing: endpoint,
        provider: 'ollama',
        model: 'local-model',
        instructions: '',
        senders: ['owner@example.com'],
        recipients: ['owner@example.com'],
        folders: [],
        archive: '',
        trash: '',
        includeExisting: false,
        localArchive: false,
      },
    };
    const inboxVerified: EmailSnapshot = {
      ...empty,
      revision: 1,
      account: { ...account, tested: true },
      lastStatus: 'Inbox connection verified. SMTP connection timed out.',
    };
    request
      .mockResolvedValueOnce({ ...empty, account })
      .mockResolvedValueOnce(inboxVerified)
      .mockResolvedValueOnce({
        ...inboxVerified,
        revision: 2,
        account: { ...account, tested: true, enabled: true },
        activity: [
          {
            id: 'draft',
            at: '2026-11-20T14:00:00Z',
            subject: 'Hello',
            status: 'Reply ready for review',
            draft: { recipient: 'owner@example.com', text: 'Hello there' },
            localBody: null,
            localFolder: null,
            trashed: null,
          },
        ],
      });
    try {
      render(<EmailSettingsSection setView={vi.fn()} />, { wrapper: IntlTestWrapper });
      expect(await screen.findByRole('button', { name: 'Enable email' })).toBeDisabled();
      fireEvent.click(screen.getByRole('button', { name: 'Test connection' }));
      expect(await screen.findByText(inboxVerified.lastStatus!)).toBeVisible();
      expect(screen.getByText(/Sending is unavailable/)).toBeVisible();
      const enable = screen.getByRole('button', { name: 'Enable email' });
      expect(enable).toBeEnabled();
      fireEvent.click(enable);
      await screen.findByText(/Monitoring inbox/);
      expect(request).toHaveBeenLastCalledWith({ op: 'enable', revision: 1 });
      expect(screen.getByRole('button', { name: 'Process now' })).toBeEnabled();
      fireEvent.click(screen.getByText('Mail activity (1)'));
      fireEvent.click(screen.getByText('Review reply to owner@example.com'));
      expect(screen.getByRole('button', { name: 'Send reply' })).toBeDisabled();
      expect(screen.queryByText('Create reply code')).toBeNull();

      const disable = screen.getByRole('button', { name: 'Disable email' });
      expect(disable).toBeEnabled();
      request.mockResolvedValueOnce({ ...inboxVerified, revision: 3 });
      fireEvent.click(disable);
      expect(await screen.findByRole('button', { name: 'Enable email' })).toBeEnabled();
      expect(request).toHaveBeenLastCalledWith({ op: 'pause', revision: 2 });
      expect(screen.getByRole('button', { name: 'Process now' })).toBeDisabled();
    } finally {
      restore();
    }
  });
});
