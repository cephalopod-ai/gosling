import { afterEach, describe, expect, it, vi } from 'vitest';
import type { GoslingToolCallResponse_unstable } from '@repo-makeover/gosling-sdk';
import type { Message } from '../types/message';
import { discoverBackgroundTasks } from './backgroundTasks';
import {
  checkSessionRunStatus,
  parseBackgroundTaskStatus,
  RUN_STATUS_TIMEOUT_MS,
} from './runStatus';

const { sessionInfo, callTool } = vi.hoisted(() => ({ sessionInfo: vi.fn(), callTool: vi.fn() }));
vi.mock('./acpConnection', () => ({
  getAcpClient: vi.fn(async () => ({
    gosling: { sessionInfo_unstable: sessionInfo, toolsCall_unstable: callTool },
  })),
}));

const task = { id: '20260916_1', description: 'Verify sources', state: 'running' as const };

function response(status: string, idle = '0s'): GoslingToolCallResponse_unstable {
  return {
    content: [{ type: 'text', text: `**Idle:** ${idle}` }],
    isError: false,
    _meta: { task_status: status, turns_taken: 3 },
  };
}

function toolMessage(content: Message['content']): Message {
  return {
    role: 'assistant',
    created: 123,
    content,
    metadata: { userVisible: true, agentVisible: true },
  };
}

function delegation(async = true): Message[] {
  return [
    toolMessage([
      {
        type: 'toolRequest',
        id: 'delegate-1',
        toolCall: {
          status: 'success',
          value: { name: 'delegate', arguments: { async } },
        },
      },
    ]),
    toolMessage([
      {
        type: 'toolResponse',
        id: 'delegate-1',
        toolResult: {
          status: 'success',
          value: {
            content: [
              {
                type: 'text',
                text: `Task ${task.id} started in background: "${task.description}"`,
              },
            ],
          },
        },
      },
    ]),
  ];
}

afterEach(() => {
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe('background task evidence', () => {
  it('discovers tasks only from a matched async delegate result', () => {
    expect(discoverBackgroundTasks(delegation())).toEqual([task]);
    expect(discoverBackgroundTasks(delegation(false))).toEqual([]);
    expect(discoverBackgroundTasks(delegation().slice(1))).toEqual([]);
    expect(
      discoverBackgroundTasks([
        toolMessage([
          { type: 'text', text: `Task ${task.id} started in background: "Verify sources"` },
        ]),
      ])
    ).toEqual([]);
    expect(
      discoverBackgroundTasks(
        delegation().map((message) => ({
          ...message,
          metadata: { ...message.metadata, importedUntrusted: true },
        }))
      )
    ).toEqual([]);
  });

  it('stops tracking activity when the agent collects a terminal task result', () => {
    const messages = [
      ...delegation(),
      toolMessage([
        {
          type: 'toolRequest',
          id: 'load-1',
          toolCall: { status: 'success', value: { name: 'load', arguments: { source: task.id } } },
        },
        {
          type: 'toolResponse',
          id: 'load-1',
          toolResult: {
            status: 'success',
            value: { content: [{ type: 'text', text: '**Status:** ✓ Completed' }] },
          },
        },
      ]),
    ];
    expect(discoverBackgroundTasks(messages)).toEqual([{ ...task, state: 'completed' }]);
  });

  it('distinguishes idle work from fresh activity and terminal failures', () => {
    expect(parseBackgroundTaskStatus(task, response('running', '14m'), 123).state).toBe('running');
    expect(parseBackgroundTaskStatus(task, response('running', '15m'), 123)).toMatchObject({
      state: 'quiet',
      turns: 3,
      idleMs: 900_000,
    });
    expect(parseBackgroundTaskStatus(task, response('completed'), 123).state).toBe('completed');
    expect(parseBackgroundTaskStatus(task, response('panicked'), 123).state).toBe('failed');
    expect(
      parseBackgroundTaskStatus(task, { ...response('running'), isError: true }, 123).state
    ).toBe('unknown');
    expect(parseBackgroundTaskStatus(task, { content: [], isError: false }, 123).state).toBe(
      'unknown'
    );
  });
});

describe('live status checks', () => {
  it('checks the backend and peeks without consuming or cancelling task results', async () => {
    sessionInfo.mockResolvedValue({ session: {} });
    callTool.mockResolvedValue(response('running', '1m'));
    const status = await checkSessionRunStatus('live-status', [task]);
    expect(sessionInfo).toHaveBeenCalledWith({ sessionId: 'live-status' });
    expect(callTool).toHaveBeenCalledWith({
      sessionId: 'live-status',
      name: 'load',
      arguments: { source: task.id, peek: true },
    });
    expect(status).toMatchObject({
      backendResponded: true,
      tasks: [{ state: 'running', turns: 3 }],
    });
  });

  it('reports denied checks as unverified even when the backend answers', async () => {
    sessionInfo.mockResolvedValue({ session: {} });
    callTool.mockRejectedValue(new Error('Tool requires approval'));
    expect(await checkSessionRunStatus('denied-status', [task])).toMatchObject({
      backendResponded: true,
      tasks: [{ state: 'unknown', error: 'Tool requires approval' }],
    });
  });

  it('bounds a hung check and reuses its pending request rather than piling up calls', async () => {
    vi.useFakeTimers();
    let resolveInfo!: (value: unknown) => void;
    sessionInfo.mockReturnValue(
      new Promise((resolve) => {
        resolveInfo = resolve;
      })
    );
    const first = expect(checkSessionRunStatus('hung-status', [])).rejects.toThrow(
      'Status check timed out'
    );
    await vi.advanceTimersByTimeAsync(RUN_STATUS_TIMEOUT_MS);
    await first;
    const second = expect(checkSessionRunStatus('hung-status', [])).rejects.toThrow(
      'Status check timed out'
    );
    await vi.advanceTimersByTimeAsync(RUN_STATUS_TIMEOUT_MS);
    await second;
    expect(sessionInfo).toHaveBeenCalledOnce();
    resolveInfo({ session: {} });
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(0);
  });
});
