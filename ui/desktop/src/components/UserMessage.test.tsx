import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Message } from '../types/message';
import UserMessage from './UserMessage';

function userMessage(text: string): Message {
  return {
    id: 'user-one',
    role: 'user',
    content: [{ type: 'text', text }],
    created: 1,
    metadata: { agentVisible: true, userVisible: true },
  };
}

describe('UserMessage retry', () => {
  it('resubmits the original content in place', async () => {
    const onMessageUpdate = vi.fn();
    render(
      <UserMessage
        message={userMessage('run the tests')}
        canRetry
        onMessageUpdate={onMessageUpdate}
      />,
      { wrapper: IntlTestWrapper }
    );

    await userEvent.click(screen.getByRole('button', { name: /Resend message/ }));

    expect(onMessageUpdate).toHaveBeenCalledWith('user-one', 'run the tests', 'edit');
  });

  it('hides retry when the message is not the latest prompt', () => {
    render(
      <UserMessage
        message={userMessage('run the tests')}
        canRetry={false}
        onMessageUpdate={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.queryByRole('button', { name: /Resend message/ })).toBeNull();
    expect(screen.getByRole('button', { name: /Edit message/ })).toBeInTheDocument();
  });
});
