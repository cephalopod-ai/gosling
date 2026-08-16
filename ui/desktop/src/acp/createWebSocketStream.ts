import type { Stream } from '@repo-makeover/gosling-sdk';

export type ClosableAcpStream = Stream & {
  close: () => void;
};

export const MAX_BUFFERED_ACP_MESSAGES = 1024;
export const MAX_BUFFERED_ACP_MESSAGE_CHARS = 8_000_000;
export const MAX_ACP_MESSAGE_CHARS = MAX_BUFFERED_ACP_MESSAGE_CHARS;
const ACP_OVERSIZED_CLOSE_CODE = 4009;

export function createWebSocketStream(wsUrl: string, subprotocol?: string): ClosableAcpStream {
  // `WebSocket` cannot set request headers, so the shared secret is offered as
  // a subprotocol; the server echoes it back on acceptance (SEC-GOS-001).
  const ws = subprotocol
    ? new globalThis.WebSocket(wsUrl, subprotocol)
    : new globalThis.WebSocket(wsUrl);

  const incoming: Array<{ message: unknown; encodedLength: number }> = [];
  const waiters: Array<() => void> = [];
  let closed = false;
  let closeError: Error | undefined;
  let bufferedChars = 0;

  const closeWaiters = (error?: Error) => {
    closed = true;
    closeError ??= error;
    for (const waiter of waiters) {
      waiter();
    }
    waiters.length = 0;
  };

  function rejectOversizedPeer(errorMessage: string, closeReason: string): void {
    incoming.length = 0;
    bufferedChars = 0;
    closeWaiters(new Error(errorMessage));
    ws.close(ACP_OVERSIZED_CLOSE_CODE, closeReason);
  }

  function pushMessage(message: unknown, encodedLength: number): void {
    if (closed) {
      return;
    }
    if (
      incoming.length >= MAX_BUFFERED_ACP_MESSAGES ||
      bufferedChars + encodedLength > MAX_BUFFERED_ACP_MESSAGE_CHARS
    ) {
      rejectOversizedPeer(
        'ACP WebSocket receive buffer exceeded its limit',
        'ACP receive buffer limit exceeded'
      );
      return;
    }
    incoming.push({ message, encodedLength });
    bufferedChars += encodedLength;
    waiters.shift()?.();
  }

  function waitForMessage(): Promise<void> {
    if (incoming.length > 0 || closed) {
      return Promise.resolve();
    }
    return new Promise<void>((resolve) => waiters.push(resolve));
  }

  const openPromise = new Promise<void>((resolve, reject) => {
    ws.addEventListener('open', () => resolve(), { once: true });
    ws.addEventListener('error', () => reject(new Error('ACP WebSocket connection failed')), {
      once: true,
    });
  });

  ws.addEventListener('message', (event) => {
    if (typeof event.data !== 'string') {
      return;
    }
    if (event.data.length > MAX_ACP_MESSAGE_CHARS) {
      rejectOversizedPeer('ACP WebSocket message exceeded its limit', 'ACP message limit exceeded');
      return;
    }
    try {
      pushMessage(JSON.parse(event.data), event.data.length);
    } catch {
      // Ignore malformed messages from the transport.
    }
  });

  ws.addEventListener('close', () => closeWaiters());
  ws.addEventListener('error', () => closeWaiters(new Error('ACP WebSocket connection failed')));

  const readable = new globalThis.ReadableStream({
    async pull(controller) {
      await waitForMessage();
      if (incoming.length > 0) {
        const next = incoming.shift();
        if (next) {
          bufferedChars -= next.encodedLength;
          controller.enqueue(next.message);
        }
        return;
      }
      if (closeError) {
        controller.error(closeError);
      } else if (closed) {
        controller.close();
      }
    },
  });

  const writable = new globalThis.WritableStream({
    async write(message) {
      await openPromise;
      ws.send(JSON.stringify(message));
    },
    close() {
      ws.close();
    },
    abort() {
      ws.close();
    },
  });

  return {
    readable,
    writable,
    close: () => ws.close(),
  } as ClosableAcpStream;
}
