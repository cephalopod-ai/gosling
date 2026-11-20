import { spawn, type ChildProcess } from 'node:child_process';
import { BrowserWindow, dialog, type IpcMain, type WebContents } from 'electron';
import { findGoslingBinaryPath } from '../goslingServe';
import { desktopCommandChannels } from '../ipc/channels';
import type { EmailRequest, EmailSnapshot } from '../types/email';

const POLL_INTERVAL = 60_000;
const MAX_REQUEST = 65_536;
const MAX_RESPONSE = 10 * 1024 * 1024;
const OPERATIONS = new Set([
  'status',
  'save',
  'test',
  'enable',
  'pause',
  'disconnect',
  'forget',
  'poll',
  'reset',
  'createReplyCode',
  'clearReplyCodes',
  'send',
  'restore',
  'discard',
]);

interface Options {
  isPackaged: boolean;
  resourcesPath: string;
  pathRoot?: string;
}

export class DesktopEmailWorker {
  private readonly renderers = new WeakSet<WebContents>();
  private pending: Promise<unknown> = Promise.resolve();
  private timer?: NodeJS.Timeout;
  private child?: ChildProcess;
  private childOperation?: EmailRequest['op'];
  private stopped = false;
  private activePoll = false;
  private pollGeneration = 0;
  private enabled?: boolean;

  constructor(private readonly options: Options) {}

  trustRenderer(contents: WebContents): void {
    this.renderers.add(contents);
  }

  register(ipc: IpcMain): void {
    ipc.handle(desktopCommandChannels.emailRequest, async (event, request: EmailRequest) => {
      if (!this.renderers.has(event.sender) || event.senderFrame !== event.sender.mainFrame) {
        throw new Error('Email settings are available from the Gosling window');
      }
      if (!request || typeof request !== 'object' || !OPERATIONS.has(request.op)) {
        throw new Error('Invalid email operation');
      }
      const window = BrowserWindow.fromWebContents(event.sender);
      if (!window) throw new Error('Email settings window is unavailable');
      return await this.handle(window, request);
    });
  }

  start(): void {
    if (this.stopped || this.timer || process.env.ENABLE_PLAYWRIGHT) return;
    const tick = () => {
      if (this.enabled === false || this.activePoll) return;
      this.activePoll = true;
      void this.enqueue({ op: 'poll' })
        .catch(() => {
          // The worker records mailbox errors for the settings view.
        })
        .finally(() => {
          this.activePoll = false;
        });
    };
    tick();
    this.timer = setInterval(tick, POLL_INTERVAL);
    this.timer.unref();
  }

  async stop(): Promise<void> {
    this.stopped = true;
    clearInterval(this.timer);
    this.child?.kill();
    await this.pending;
  }

  private async handle(window: BrowserWindow, request: EmailRequest): Promise<EmailSnapshot> {
    if (request.op === 'send') {
      const snapshot = await this.enqueue({ op: 'status' });
      const item = snapshot.activity.find((entry) => entry.id === request.id);
      if (snapshot.revision !== request.revision || !item?.draft) {
        throw new Error('This reply changed. Refresh Email settings and try again');
      }
      const decision = await dialog.showMessageBox(window, {
        type: 'question',
        title: 'Send email reply',
        message: `Send a reply to ${item.draft.recipient}?`,
        detail: item.draft.text.slice(0, 800),
        buttons: ['Cancel', 'Send reply'],
        defaultId: 0,
        cancelId: 0,
      });
      if (decision.response !== 1) return snapshot;
    }
    if (
      ['save', 'pause', 'disconnect', 'forget', 'clearReplyCodes', 'createReplyCode'].includes(
        request.op
      )
    ) {
      this.pollGeneration += 1;
      if (this.childOperation === 'poll') this.child?.kill();
    }
    return await this.enqueue(request);
  }

  private enqueue(request: EmailRequest): Promise<EmailSnapshot> {
    const generation = this.pollGeneration;
    const result = this.pending.then(async () => {
      if (this.stopped) throw new Error('Email worker is stopping');
      if (request.op === 'poll' && generation !== this.pollGeneration) {
        throw new Error('Inbox check was cancelled for an email settings change');
      }
      const snapshot = await this.run(request);
      this.enabled = snapshot.account?.enabled ?? false;
      return snapshot;
    });
    this.pending = result.catch(() => undefined);
    return result;
  }

  private async run(request: EmailRequest): Promise<EmailSnapshot> {
    const payload = JSON.stringify(request);
    if (Buffer.byteLength(payload) >= MAX_REQUEST) throw new Error('Email request is too large');
    const binary = findGoslingBinaryPath(this.options);
    return await new Promise((resolve, reject) => {
      const env = { ...process.env };
      if (this.options.pathRoot) env.GOSLING_PATH_ROOT = this.options.pathRoot;
      if (request.op === 'poll' || request.op === 'status')
        env.GOSLING_KEYRING_NONINTERACTIVE = 'true';
      const child = spawn(binary, ['desktop-email', '--supervised'], {
        windowsHide: true,
        stdio: ['pipe', 'pipe', 'ignore'],
        env,
      });
      this.child = child;
      this.childOperation = request.op;
      const chunks: Buffer[] = [];
      let size = 0;
      const timer = setTimeout(() => child.kill(), 165_000);
      child.stdout.on('data', (chunk: Buffer) => {
        size += chunk.length;
        if (size > MAX_RESPONSE) {
          child.kill();
          return;
        }
        chunks.push(chunk);
      });
      child.stdin.on('error', () => {
        // Child close reports unsupported and prematurely ended requests.
      });
      child.once('error', () =>
        reject(new Error('Could not start the local Gosling email worker'))
      );
      child.once('close', (code) => {
        clearTimeout(timer);
        if (this.child === child) {
          this.child = undefined;
          this.childOperation = undefined;
        }
        child.stdin.destroy();
        if (code !== 0 || size > MAX_RESPONSE) {
          reject(new Error('The local Gosling binary could not complete the email operation'));
          return;
        }
        try {
          const response = JSON.parse(Buffer.concat(chunks).toString('utf8')) as {
            data?: EmailSnapshot;
            error?: string;
          };
          if (response.data) resolve(response.data);
          else reject(new Error(response.error || 'Invalid response from the email worker'));
        } catch {
          reject(new Error('Invalid response from the email worker'));
        }
      });
      child.stdin.write(`${payload}\n`);
    });
  }
}
