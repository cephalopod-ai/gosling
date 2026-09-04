import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Message, MessageContent } from '../types/message';
import ProgressiveMessageList from './ProgressiveMessageList';

vi.mock('./GoslingMessage', () => ({
  default: ({ message }: { message: Message }) => {
    const content = message.content.find(
      (candidate) => candidate.type === 'toolRequest' || candidate.type === 'text'
    );
    return (
      <div>
        {content?.type === 'toolRequest'
          ? content.id
          : content?.type === 'text'
            ? content.text
            : null}
      </div>
    );
  },
}));

vi.mock('./UserMessage', () => ({
  default: ({ message, canRetry }: { message: Message; canRetry: boolean }) => (
    <div data-can-retry={String(canRetry)}>
      {message.content.find((content) => content.type === 'text')?.text}
    </div>
  ),
}));

function message(role: Message['role'], content: MessageContent[], id: string): Message {
  return {
    id,
    role,
    content,
    created: 1,
    metadata: { agentVisible: true, userVisible: true },
  };
}

function toolResponse(id: string): MessageContent {
  return { type: 'toolResponse', id, toolResult: { status: 'success', value: { content: [] } } };
}

function toolRequest(id: string): MessageContent {
  return {
    type: 'toolRequest',
    id,
    toolCall: { status: 'success', value: { name: 'developer__shell', arguments: {} } },
  };
}

function resolvedModelMessage(id: string, modelName: string): Message {
  const result = message('assistant', [{ type: 'text', text: id }], id);
  result.metadata.inference = {
    provider: 'test-provider',
    requestedModel: modelName,
    resolvedModel: modelName,
  };
  return result;
}

describe('ProgressiveMessageList tool activity', () => {
  it('marks user messages as thread navigation landmarks', () => {
    const { container } = render(
      <ProgressiveMessageList
        messages={[
          message('user', [{ type: 'text', text: 'First prompt' }], 'user-one'),
          message('assistant', [{ type: 'text', text: 'Answer' }], 'assistant-one'),
        ]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
        threadTurnAttribute="data-thread-turn-index"
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(container.querySelector('[data-thread-turn-index="0"]')).toHaveClass('user');
    expect(container.querySelector('[data-thread-turn-index="1"]')).toBeNull();
  });

  it('renders every message immediately when thread navigation requests it', () => {
    render(
      <ProgressiveMessageList
        messages={[
          message('user', [{ type: 'text', text: 'First prompt' }], 'user-one'),
          message('assistant', [{ type: 'text', text: 'First answer' }], 'assistant-one'),
          message('user', [{ type: 'text', text: 'Second prompt' }], 'user-two'),
        ]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
        batchSize={1}
        batchDelay={60_000}
        showLoadingThreshold={1}
        forceRenderAll
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.getByText('First prompt')).toBeInTheDocument();
    expect(screen.getByText('First answer')).toBeInTheDocument();
    expect(screen.getByText('Second prompt')).toBeInTheDocument();
    expect(screen.queryByText(/Loading messages/)).not.toBeInTheDocument();
  });

  it('collapses a run of tool activity without hiding adjacent prose', async () => {
    render(
      <ProgressiveMessageList
        messages={[
          message('assistant', [toolRequest('tool-one')], 'request-one'),
          message('assistant', [toolRequest('tool-two')], 'request-two'),
          message('assistant', [{ type: 'text', text: 'Finished.' }], 'final'),
        ]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
      />,
      { wrapper: IntlTestWrapper }
    );

    const trigger = screen.getByRole('button', { name: 'Activity (2)' });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('tool-one')).not.toBeInTheDocument();
    expect(screen.queryByText('tool-two')).not.toBeInTheDocument();
    expect(screen.getByText('Finished.')).toBeInTheDocument();

    await userEvent.click(trigger);

    expect(screen.getByText('tool-one')).toBeInTheDocument();
    expect(screen.getByText('tool-two')).toBeInTheDocument();
  });

  it('opens a group containing a pending approval', () => {
    render(
      <ProgressiveMessageList
        messages={[
          message('assistant', [toolRequest('tool-one')], 'request-one'),
          message('assistant', [toolRequest('tool-two')], 'request-two'),
          message(
            'assistant',
            [
              {
                type: 'toolConfirmationRequest',
                id: 'tool-two',
                arguments: {},
                toolName: 'developer__shell',
              },
            ],
            'confirmation'
          ),
        ]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.getByRole('button', { name: 'Activity (2)' })).toHaveAttribute(
      'aria-expanded',
      'true'
    );
    expect(screen.getByText('tool-two')).toBeInTheDocument();
  });

  it('renders model disclosures without duplicating recorded switches', () => {
    render(
      <ProgressiveMessageList
        messages={[
          resolvedModelMessage('answer-a', 'model-a'),
          message('assistant', [{ type: 'text', text: 'No model metadata' }], 'interlude'),
          resolvedModelMessage('answer-b', 'model-b'),
          message(
            'assistant',
            [
              {
                type: 'systemNotification',
                msg: 'Model changed: model-b → model-c',
                notificationType: 'inlineMessage',
              },
            ],
            'model-switch'
          ),
          resolvedModelMessage('answer-c', 'model-c'),
          resolvedModelMessage('answer-d', 'model-d'),
        ]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.getByText('Model changed: model-a → model-b')).toBeInTheDocument();
    expect(screen.getAllByText('Model changed: model-b → model-c')).toHaveLength(1);
    expect(screen.getByText('Model changed: model-c → model-d')).toBeInTheDocument();
  });
});

describe('ProgressiveMessageList retry affordance', () => {
  it('offers retry only on the latest user prompt', () => {
    const { container } = render(
      <ProgressiveMessageList
        messages={[
          message('user', [{ type: 'text', text: 'First prompt' }], 'user-one'),
          message('assistant', [{ type: 'text', text: 'First answer' }], 'assistant-one'),
          message('user', [{ type: 'text', text: 'Second prompt' }], 'user-two'),
          message('user', [toolResponse('tool-one')], 'user-three'),
        ]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
      />,
      { wrapper: IntlTestWrapper }
    );

    const prompts = container.querySelectorAll('[data-can-retry]');
    expect(Array.from(prompts).map((prompt) => prompt.getAttribute('data-can-retry'))).toEqual([
      'false',
      'true',
    ]);
  });

  it('hides retry while a response is streaming', () => {
    const { container } = render(
      <ProgressiveMessageList
        messages={[message('user', [{ type: 'text', text: 'First prompt' }], 'user-one')]}
        chat={{ sessionId: 'session-one' }}
        isUserMessage={(candidate) => candidate.role === 'user'}
        isStreamingMessage
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(container.querySelector('[data-can-retry="true"]')).toBeNull();
  });
});
