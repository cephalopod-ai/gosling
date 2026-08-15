import { randomUUID } from 'node:crypto';
import type { SessionNotification } from '@agentclientprotocol/sdk';
import { projectShellSessionUpdate, type ShellSessionStream } from './sessionUpdateProjection';

const MAX_PROMPT_BYTES = 64 * 1024;

export interface ShellSessionRecord {
  sessionId: string;
  status: 'none' | 'creating' | 'active' | 'resuming' | 'closing';
  resumeKind: 'fresh' | 'resumed';
  resumeIntegrity: 'clean' | 'uncertain' | 'not_applicable';
  promptAttempt: { id: string; phase: 'idle' | 'streaming' | 'cancelling' } | null;
}

export interface ShellSessionUpdate {
  generation: number;
  sessionId: string;
  promptAttemptId: string | null;
  updateSeq: number;
  kind: 'started' | 'completed' | 'cancelled' | 'failed' | 'stream';
  stream?: ShellSessionStream;
}

export interface ShellSessionTransport {
  createSession(): Promise<{ sessionId: string }>;
  resumeSession(sessionId: string): Promise<{
    sessionId: string;
    resumeIntegrity?: 'clean' | 'uncertain';
  }>;
  prompt(input: { sessionId: string; text: string; messageId: string }): Promise<unknown>;
  cancel(input: { sessionId: string }): Promise<void>;
}

export interface ShellSessionController {
  read(): ShellSessionRecord;
  create(generation: number): Promise<ShellSessionRecord>;
  resume(generation: number, sessionId: string): Promise<ShellSessionRecord>;
  submit(input: { generation: number; sessionId: string; text: string }): {
    promptAttemptId: string;
  };
  cancel(input: { generation: number; sessionId: string; promptAttemptId: string }): Promise<void>;
  ingestUpdate(notification: SessionNotification): void;
  onUpdate(listener: (update: ShellSessionUpdate) => void): () => void;
  onChanged(listener: (session: ShellSessionRecord) => void): () => void;
  close(outcome?: 'cancelled' | 'failed'): void;
}

function emptySession(): ShellSessionRecord {
  return {
    sessionId: '',
    status: 'none',
    resumeKind: 'fresh',
    resumeIntegrity: 'not_applicable',
    promptAttempt: null,
  };
}

function copySession(session: ShellSessionRecord): ShellSessionRecord {
  return {
    ...session,
    promptAttempt: session.promptAttempt ? { ...session.promptAttempt } : null,
  };
}

function assertSessionId(sessionId: string): void {
  if (
    typeof sessionId !== 'string' ||
    sessionId.length === 0 ||
    sessionId.length > 512 ||
    sessionId.trim() !== sessionId
  ) {
    throw new Error('sessionId must be a non-empty bounded string');
  }
}

function assertPrompt(text: string): void {
  if (
    typeof text !== 'string' ||
    text.trim().length === 0 ||
    Buffer.byteLength(text, 'utf8') > MAX_PROMPT_BYTES
  ) {
    throw new Error('prompt text must be non-empty and at most 64 KiB');
  }
}

export function createShellSessionController(input: {
  transport: ShellSessionTransport;
  generation: () => number;
  createAttemptId?: () => string;
}): ShellSessionController {
  let session = emptySession();
  let updateSeq = 0;
  // Bumped by every close, so an open that is still awaiting the transport cannot reinstate a
  // session the shell has already released.
  let openEpoch = 0;
  const listeners = new Set<(update: ShellSessionUpdate) => void>();
  const stateListeners = new Set<(session: ShellSessionRecord) => void>();
  const createAttemptId = input.createAttemptId ?? randomUUID;

  const publish = (
    kind: ShellSessionUpdate['kind'],
    promptAttemptId: string | null,
    stream?: ShellSessionStream
  ) => {
    updateSeq += 1;
    const update: ShellSessionUpdate = {
      generation: input.generation(),
      sessionId: session.sessionId,
      promptAttemptId,
      updateSeq,
      kind,
      ...(stream ? { stream } : {}),
    };
    for (const listener of listeners) listener(update);
  };

  const publishState = () => {
    const copy = copySession(session);
    for (const listener of stateListeners) listener(copy);
  };

  const assertCurrent = (generation: number, sessionId: string) => {
    if (generation !== input.generation()) throw new Error('session request generation is stale');
    assertSessionId(sessionId);
    if (session.status !== 'active' || session.sessionId !== sessionId) {
      throw new Error('session is not active');
    }
  };

  const open = async (
    generation: number,
    kind: 'creating' | 'resuming',
    operation: () => Promise<{ sessionId: string; resumeIntegrity?: 'clean' | 'uncertain' }>
  ) => {
    if (generation !== input.generation()) throw new Error('session request generation is stale');
    if (session.status !== 'none') throw new Error('an active session already exists');
    const epoch = openEpoch;
    session = { ...emptySession(), status: kind };
    publishState();
    try {
      const opened = await operation();
      if (generation !== input.generation()) throw new Error('session request generation is stale');
      if (epoch !== openEpoch) throw new Error('session was released while it was opening');
      assertSessionId(opened.sessionId);
      session = {
        sessionId: opened.sessionId,
        status: 'active',
        resumeKind: kind === 'creating' ? 'fresh' : 'resumed',
        resumeIntegrity:
          kind === 'creating' ? 'not_applicable' : (opened.resumeIntegrity ?? 'uncertain'),
        promptAttempt: null,
      };
      updateSeq = 0;
      publishState();
      return session;
    } catch (error) {
      if (epoch === openEpoch) {
        session = emptySession();
        publishState();
      }
      throw error;
    }
  };

  return {
    read: () => copySession(session),
    create: (generation) => open(generation, 'creating', () => input.transport.createSession()),
    resume: (generation, sessionId) => {
      assertSessionId(sessionId);
      return open(generation, 'resuming', () => input.transport.resumeSession(sessionId));
    },
    submit({ generation, sessionId, text }) {
      assertCurrent(generation, sessionId);
      assertPrompt(text);
      if (session.promptAttempt) throw new Error('a prompt attempt is already active');
      const promptAttemptId = createAttemptId();
      if (
        typeof promptAttemptId !== 'string' ||
        promptAttemptId.length === 0 ||
        promptAttemptId.length > 512
      ) {
        throw new Error('prompt attempt ID is invalid');
      }
      session = {
        ...session,
        promptAttempt: { id: promptAttemptId, phase: 'streaming' },
      };
      publishState();
      publish('started', promptAttemptId);
      void input.transport
        .prompt({ sessionId, text, messageId: promptAttemptId })
        .then(() => {
          if (session.promptAttempt?.id !== promptAttemptId) return;
          const wasCancelling = session.promptAttempt.phase === 'cancelling';
          session = { ...session, promptAttempt: null };
          publishState();
          publish(wasCancelling ? 'cancelled' : 'completed', promptAttemptId);
        })
        .catch(() => {
          if (session.promptAttempt?.id !== promptAttemptId) return;
          session = { ...session, promptAttempt: null };
          publishState();
          publish('failed', promptAttemptId);
        });
      return { promptAttemptId };
    },
    async cancel({ generation, sessionId, promptAttemptId }) {
      assertCurrent(generation, sessionId);
      if (session.promptAttempt?.id !== promptAttemptId) throw new Error('prompt attempt is stale');
      if (session.promptAttempt.phase !== 'cancelling') {
        session = { ...session, promptAttempt: { id: promptAttemptId, phase: 'cancelling' } };
        publishState();
        await input.transport.cancel({ sessionId });
      }
    },
    ingestUpdate(notification) {
      if (
        notification.sessionId !== session.sessionId ||
        session.status !== 'active' ||
        !session.promptAttempt
      ) {
        return;
      }
      const stream = projectShellSessionUpdate(notification.update);
      if (stream) publish('stream', session.promptAttempt.id, stream);
    },
    onUpdate(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    onChanged(listener) {
      stateListeners.add(listener);
      return () => stateListeners.delete(listener);
    },
    close(outcome) {
      openEpoch += 1;
      const attempt = session.promptAttempt;
      if (attempt && outcome) {
        publish(outcome, attempt.id);
      }
      session = emptySession();
      updateSeq = 0;
      publishState();
    },
  };
}
