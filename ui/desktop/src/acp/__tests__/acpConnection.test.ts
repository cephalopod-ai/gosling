import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const clients = vi.hoisted(
  () =>
    [] as Array<{
      initialize: ReturnType<typeof vi.fn>;
      disconnect: () => void;
      closed: Promise<void>;
    }>
);
const closeStream = vi.hoisted(() => vi.fn());
const initialize = vi.hoisted(() => vi.fn());

vi.mock('@repo-makeover/gosling-sdk', () => ({
  DEFAULT_GOSLING_MCP_HOST_CAPABILITIES: {},
  GoslingClient: class {
    initialize = initialize;
    disconnect!: () => void;
    closed = new Promise<void>((resolve) => {
      this.disconnect = resolve;
    });
    constructor() {
      clients.push(this);
    }
  },
}));
vi.mock('../createWebSocketStream', () => ({
  createWebSocketStream: () => ({ close: closeStream }),
}));
vi.mock('../chatNotifications', () => ({
  handleAcpGoslingSessionNotification: vi.fn(),
  handleAcpSessionNotification: vi.fn(),
}));
vi.mock('../elicitationRequests', () => ({ requestAcpElicitation: vi.fn() }));
vi.mock('../permissionRequests', () => ({ requestAcpPermission: vi.fn() }));

describe('ACP connection status', () => {
  beforeEach(() => {
    vi.resetModules();
    clients.length = 0;
    closeStream.mockClear();
    initialize.mockReset().mockResolvedValue({ protocolVersion: 1 });
    window.electron.getAcpUrl = vi.fn().mockResolvedValue({ url: 'ws://localhost:3000/acp' });
  });

  afterEach(() => vi.useRealTimers());

  it('reports an idle disconnect immediately and reconnects without issuing a prompt', async () => {
    const connection = await import('../acpConnection');
    const states: string[] = [];
    const unsubscribe = connection.subscribeAcpConnectionStatus(() =>
      states.push(connection.getAcpConnectionStatus())
    );
    expect(connection.getAcpConnectionStatus()).toBe('idle');
    const first = await connection.getAcpClient();
    expect(await connection.getAcpClient()).toBe(first);
    expect(connection.getAcpConnectionGeneration()).toBe(1);
    clients[0].disconnect();
    await Promise.resolve();
    expect(connection.getAcpConnectionStatus()).toBe('disconnected');
    expect(connection.getAcpClientSync()).toBeNull();
    const [second, shared] = await Promise.all([
      connection.getAcpClient(),
      connection.getAcpClient(),
    ]);
    expect(second).not.toBe(first);
    expect(shared).toBe(second);
    expect(connection.getAcpConnectionGeneration()).toBe(2);
    expect(states).toEqual([
      'connecting',
      'connected',
      'disconnected',
      'reconnecting',
      'connected',
    ]);
    unsubscribe();
    clients[1].disconnect();
    await Promise.resolve();
    expect(states).toHaveLength(5);
  });

  it('keeps a failed initialize retryable and closes the failed transport', async () => {
    vi.useFakeTimers();
    initialize.mockReturnValueOnce(new Promise(() => {}));
    const connection = await import('../acpConnection');
    const request = connection.getAcpClient();
    const rejection = expect(request).rejects.toThrow('ACP initialize timed out');
    await vi.advanceTimersByTimeAsync(10000);
    await rejection;
    expect(connection.getAcpConnectionStatus()).toBe('disconnected');
    expect(closeStream).toHaveBeenCalledOnce();
    await expect(connection.getAcpClient()).resolves.toBeDefined();
    expect(connection.getAcpConnectionStatus()).toBe('connected');
  });
});
