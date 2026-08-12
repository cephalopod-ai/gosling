import { describe, expect, it } from 'vitest';
import type { Message, MessageContent } from '../types/message';
import { identifyCollapsibleToolActivityGroups } from './toolCallChaining';

function message(role: Message['role'], content: MessageContent[], id: string): Message {
  return {
    id,
    role,
    content,
    created: 1,
    metadata: { agentVisible: true, userVisible: true },
  };
}

function toolRequest(id: string): MessageContent {
  return {
    type: 'toolRequest',
    id,
    toolCall: { status: 'success', value: { name: 'developer__shell', arguments: {} } },
  };
}

function toolResponse(id: string): MessageContent {
  return { type: 'toolResponse', id, toolResult: { status: 'success', value: { content: [] } } };
}

describe('identifyCollapsibleToolActivityGroups', () => {
  it('groups consecutive tool-only messages across their response messages', () => {
    const messages = [
      message('assistant', [toolRequest('one')], 'request-one'),
      message('user', [toolResponse('one')], 'response-one'),
      message('assistant', [toolRequest('two')], 'request-two'),
      message('user', [toolResponse('two')], 'response-two'),
      message('assistant', [toolRequest('three')], 'request-three'),
    ];

    expect(identifyCollapsibleToolActivityGroups(messages)).toEqual([[0, 2, 4]]);
  });

  it('does not hide assistant prose inside an activity group', () => {
    const messages = [
      message(
        'assistant',
        [{ type: 'text', text: 'I will check that now.' }, toolRequest('one')],
        'request-one'
      ),
      message('assistant', [toolRequest('two')], 'request-two'),
      message('assistant', [toolRequest('three')], 'request-three'),
    ];

    expect(identifyCollapsibleToolActivityGroups(messages)).toEqual([[1, 2]]);
  });

  it('does not bridge across hidden messages or visible text alongside a response', () => {
    const hiddenMessage = message('assistant', [toolRequest('hidden')], 'hidden');
    hiddenMessage.metadata.userVisible = false;
    const messages = [
      message('assistant', [toolRequest('one')], 'request-one'),
      hiddenMessage,
      message('assistant', [toolRequest('two')], 'request-two'),
      message(
        'user',
        [
          toolResponse('two'),
          { type: 'image', data: 'image-data', mimeType: 'image/png' },
          { type: 'text', text: 'Stop here.' },
        ],
        'response-two'
      ),
      message('assistant', [toolRequest('three')], 'request-three'),
    ];

    expect(identifyCollapsibleToolActivityGroups(messages)).toEqual([]);
  });

  it('does not hide an assistant image inside an activity group', () => {
    const messages = [
      message(
        'assistant',
        [{ type: 'image', data: 'image-data', mimeType: 'image/png' }, toolRequest('one')],
        'request-one'
      ),
      message('assistant', [toolRequest('two')], 'request-two'),
      message('assistant', [toolRequest('three')], 'request-three'),
    ];

    expect(identifyCollapsibleToolActivityGroups(messages)).toEqual([[1, 2]]);
  });

  it('groups multiple requests in one tool-only message but leaves one request visible', () => {
    expect(
      identifyCollapsibleToolActivityGroups([
        message('assistant', [toolRequest('one'), toolRequest('two')], 'requests'),
      ])
    ).toEqual([[0]]);

    expect(
      identifyCollapsibleToolActivityGroups([message('assistant', [toolRequest('one')], 'request')])
    ).toEqual([]);
  });
});
