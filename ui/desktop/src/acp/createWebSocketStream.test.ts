import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createWebSocketStream,
  MAX_ACP_MESSAGE_CHARS,
  MAX_BUFFERED_ACP_MESSAGES,
} from './createWebSocketStream';

class MockWebSocket extends EventTarget {
  static instance: MockWebSocket;
  close = vi.fn();
  send = vi.fn();

  constructor(_url: string) {
    super();
    MockWebSocket.instance = this;
  }

  receive(message: unknown): void {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(message) }));
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('createWebSocketStream', () => {
  it('closes a peer that exceeds the bounded receive queue', () => {
    vi.stubGlobal('WebSocket', MockWebSocket);
    createWebSocketStream('ws://127.0.0.1:64027/acp');

    for (let index = 0; index <= MAX_BUFFERED_ACP_MESSAGES; index += 1) {
      MockWebSocket.instance.receive({ index });
    }

    expect(MockWebSocket.instance.close).toHaveBeenCalledWith(
      1009,
      'ACP receive buffer limit exceeded'
    );
  });

  it('rejects a single oversized message before parsing it', () => {
    vi.stubGlobal('WebSocket', MockWebSocket);
    createWebSocketStream('ws://127.0.0.1:64027/acp');

    MockWebSocket.instance.dispatchEvent(
      new MessageEvent('message', { data: ' '.repeat(MAX_ACP_MESSAGE_CHARS + 1) })
    );

    expect(MockWebSocket.instance.close).toHaveBeenCalledWith(
      1009,
      'ACP receive buffer limit exceeded'
    );
  });
});
