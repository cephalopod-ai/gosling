import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Message } from '../types/message';
import GoslingMessage from './GoslingMessage';

vi.mock('../contexts/ArtifactWorkbenchContext', () => ({
  useArtifactWorkbench: () => ({ openFile: vi.fn() }),
}));

vi.mock('./MarkdownContent', () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

vi.mock('./artifacts/ArtifactMessageLinks', () => ({
  ArtifactMessageLinks: () => null,
}));

function reply(metadata: Partial<Message['metadata']> = {}): Message {
  return {
    id: 'reply',
    role: 'assistant',
    created: 1,
    content: [{ type: 'text', text: 'LONG-START lorem ipsum lorem ip' }],
    metadata: { agentVisible: true, userVisible: true, ...metadata },
  };
}

function renderReply(message: Message) {
  render(
    <GoslingMessage
      sessionId="session-1"
      message={message}
      hideTimestamp={false}
      toolResponsesById={new Map()}
      confirmationByToolRequestId={new Map()}
      pendingConfirmationIds={new Set()}
      toolRequestIds={new Set()}
      toolCallNotifications={new Map()}
      append={vi.fn()}
      isStreaming={false}
    />,
    { wrapper: IntlTestWrapper }
  );
}

describe('GoslingMessage', () => {
  it('marks a reply that was cut off mid-stream as interrupted', () => {
    renderReply(reply({ incomplete: true }));

    expect(screen.getByText('LONG-START lorem ipsum lorem ip')).toBeInTheDocument();
    expect(screen.getByTestId('reply-interrupted')).toHaveTextContent(
      'Interrupted — this reply was cut off before it finished.'
    );
  });

  it('shows a finished reply without the interrupted marker', () => {
    renderReply(reply());

    expect(screen.getByText('LONG-START lorem ipsum lorem ip')).toBeInTheDocument();
    expect(screen.queryByTestId('reply-interrupted')).not.toBeInTheDocument();
  });
});
