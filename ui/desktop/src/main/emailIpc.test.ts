import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import type { IpcMain, IpcMainInvokeEvent, WebContents } from 'electron';
import { describe, expect, it, vi } from 'vitest';
import type { EmailRequest, EmailSnapshot } from '../types/email';
import { DesktopEmailWorker } from './emailIpc';

const { spawn } = vi.hoisted(() => ({ spawn: vi.fn() }));
vi.mock('node:child_process', () => ({ spawn, default: { spawn } }));
vi.mock('../goslingServe', () => ({ findGoslingBinaryPath: () => '/test/gosling' }));
vi.mock('electron', () => {
  const electron = {
    BrowserWindow: { fromWebContents: () => ({}) },
    dialog: { showMessageBox: vi.fn() },
  };
  return { ...electron, default: electron };
});

const unconfigured: EmailSnapshot = {
  revision: 0,
  account: null,
  activity: [],
  lastStatus: null,
  replyCodeRecipients: [],
};

describe('Desktop email IPC', () => {
  it('cancels queued polling ahead of a settings change and rejects child frames', async () => {
    const commands: EmailRequest[] = [];
    const children: Array<{ finish: () => void }> = [];
    spawn.mockImplementation(() => {
      const child = Object.assign(new EventEmitter(), {
        stdin: new PassThrough(),
        stdout: new PassThrough(),
        kill: vi.fn(),
      });
      child.stdin.on('data', (line: Buffer) => commands.push(JSON.parse(line.toString())));
      children.push({
        finish: () => {
          child.stdout.write(JSON.stringify({ data: unconfigured }));
          child.emit('close', 0);
        },
      });
      return child;
    });

    type Handle = (event: IpcMainInvokeEvent, request: EmailRequest) => Promise<EmailSnapshot>;
    const register = vi.fn<(_channel: string, handler: Handle) => void>();
    const worker = new DesktopEmailWorker({ isPackaged: false, resourcesPath: '' });
    worker.register({ handle: register } as unknown as IpcMain);
    const handle = register.mock.calls[0][1];
    const frame = {};
    const sender = { mainFrame: frame } as unknown as WebContents;
    const event = { sender, senderFrame: frame } as unknown as IpcMainInvokeEvent;
    worker.trustRenderer(sender);

    await expect(handle({ ...event, senderFrame: null }, { op: 'status' })).rejects.toThrow(
      'available from the Gosling window'
    );
    const loaded = handle(event, { op: 'status' });
    const poll = expect(handle(event, { op: 'poll' })).rejects.toThrow('cancelled');
    const paused = handle(event, { op: 'pause', revision: 0 });
    await vi.waitFor(() => expect(commands.map(({ op }) => op)).toEqual(['status']));
    children[0].finish();
    await loaded;
    await poll;
    await vi.waitFor(() => expect(commands.map(({ op }) => op)).toEqual(['status', 'pause']));
    children[1].finish();
    await paused;
    await worker.stop();
  });

  it('releases a worker that failed to start so later requests still run', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      spawn.mockImplementationOnce(() => {
        const child = Object.assign(new EventEmitter(), {
          stdin: new PassThrough(),
          stdout: new PassThrough(),
          kill: vi.fn(),
        });
        process.nextTick(() => child.emit('error', new Error('ENOENT')));
        return child;
      });
      spawn.mockImplementationOnce(() => {
        const child = Object.assign(new EventEmitter(), {
          stdin: new PassThrough(),
          stdout: new PassThrough(),
          kill: vi.fn(),
        });
        process.nextTick(() => {
          child.stdout.write(JSON.stringify({ data: unconfigured }));
          child.emit('close', 0);
        });
        return child;
      });

      type Handle = (event: IpcMainInvokeEvent, request: EmailRequest) => Promise<EmailSnapshot>;
      const register = vi.fn<(_channel: string, handler: Handle) => void>();
      const worker = new DesktopEmailWorker({ isPackaged: false, resourcesPath: '' });
      worker.register({ handle: register } as unknown as IpcMain);
      const handle = register.mock.calls[0][1];
      const frame = {};
      const sender = { mainFrame: frame } as unknown as WebContents;
      const event = { sender, senderFrame: frame } as unknown as IpcMainInvokeEvent;
      worker.trustRenderer(sender);

      await expect(handle(event, { op: 'status' })).rejects.toThrow('Could not start');
      expect(vi.getTimerCount()).toBe(0);
      await expect(handle(event, { op: 'status' })).resolves.toEqual(unconfigured);
      expect(vi.getTimerCount()).toBe(0);
      await worker.stop();
    } finally {
      vi.useRealTimers();
    }
  });
});
