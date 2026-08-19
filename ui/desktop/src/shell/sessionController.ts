import { randomUUID } from 'node:crypto';
import type { SessionNotification } from '@agentclientprotocol/sdk';
import { classifyShellOperationFailure, type ShellOperationFailure } from './operationFailure';
import { projectShellSessionUpdate, type ShellSessionStream } from './sessionUpdateProjection';

const MAX_PROMPT_BYTES = 64 * 1024;
const MAX_TRANSCRIPT_BYTES = 48 * 1024;
const MAX_TRANSCRIPT_UPDATES = 256;

export interface ShellSessionRecord {
  sessionId: string;
  status: 'none' | 'creating' | 'active' | 'resuming' | 'closing';
  resumeKind: 'fresh' | 'resumed';
  resumeIntegrity: 'clean' | 'uncertain' | 'not_applicable';
  workingDir: string | null;
  title: string | null;
  providerId: string | null;
  modelId: string | null;
  promptAttempt: { id: string; phase: 'idle' | 'streaming' | 'cancelling' } | null;
}

export interface ShellSessionUpdate {
  generation: number;
  sessionId: string;
  promptAttemptId: string | null;
  updateSeq: number;
  kind: 'started' | 'completed' | 'cancelled' | 'failed' | 'stream';
  delivery: 'history' | 'live';
  stream?: ShellSessionStream;
  failure?: ShellOperationFailure;
}

export interface ShellTranscriptSnapshot {
  generation: number;
  sessionId: string;
  integrity: 'complete' | 'incomplete' | 'resume_uncertain';
  firstSeq: number | null;
  lastSeq: number | null;
  truncated: boolean;
  updates: ShellSessionUpdate[];
}

export interface ShellSessionTransport {
  createSession(): Promise<{
    sessionId: string;
    workingDir?: string;
    title?: string | null;
    providerId?: string | null;
    modelId?: string | null;
  }>;
  resumeSession(sessionId: string): Promise<{
    sessionId: string;
    workingDir?: string;
    title?: string | null;
    providerId?: string | null;
    modelId?: string | null;
    resumeIntegrity?: 'clean' | 'uncertain';
  }>;
  prompt(input: {
    sessionId: string;
    text: string;
    messageId: string;
    libraryItemIds?: string[];
  }): Promise<unknown>;
  cancel(input: { sessionId: string }): Promise<void>;
  setProviderModel?(input: {
    sessionId: string;
    providerId: string;
    modelId: string;
  }): Promise<void>;
}

export interface ShellSessionController {
  read(): ShellSessionRecord;
  create(generation: number): Promise<ShellSessionRecord>;
  resume(generation: number, sessionId: string): Promise<ShellSessionRecord>;
  submit(input: {
    generation: number;
    sessionId: string;
    text: string;
    libraryItemIds?: string[];
  }): {
    promptAttemptId: string;
  };
  cancel(input: { generation: number; sessionId: string; promptAttemptId: string }): Promise<void>;
  setProviderModel(input: {
    generation: number;
    providerId: string;
    modelId: string;
  }): Promise<ShellSessionRecord>;
  ingestUpdate(notification: SessionNotification): void;
  readTranscript(generation: number, sessionId: string): ShellTranscriptSnapshot;
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
    workingDir: null,
    title: null,
    providerId: null,
    modelId: null,
    promptAttempt: null,
  };
}

function copyUpdate(update: ShellSessionUpdate): ShellSessionUpdate {
  return {
    ...update,
    ...(update.stream ? { stream: { ...update.stream } } : {}),
    ...(update.failure ? { failure: { ...update.failure } } : {}),
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

function assertPrompt(text: string, libraryItemIds: string[]): void {
  if (typeof text !== 'string' || Buffer.byteLength(text, 'utf8') > MAX_PROMPT_BYTES) {
    throw new Error('prompt text must be at most 64 KiB');
  }
  if (
    !Array.isArray(libraryItemIds) ||
    libraryItemIds.length > 16 ||
    libraryItemIds.some(
      (itemId) => typeof itemId !== 'string' || itemId.length === 0 || itemId.length > 128
    ) ||
    new Set(libraryItemIds).size !== libraryItemIds.length
  ) {
    throw new Error('prompt may contain up to 16 unique library items');
  }
  if (text.trim().length === 0 && libraryItemIds.length === 0) {
    throw new Error('prompt text must be non-empty unless library items are selected');
  }
}

export function createShellSessionController(input: {
  transport: ShellSessionTransport;
  generation: () => number;
  createAttemptId?: () => string;
}): ShellSessionController {
  let session = emptySession();
  let updateSeq = 0;
  let transcript: ShellSessionUpdate[] = [];
  let transcriptBytes = 0;
  let transcriptTruncated = false;
  let pendingHistory: ShellSessionStream[] = [];
  let pendingHistoryBytes = 0;
  // Bumped by every close, so an open that is still awaiting the transport cannot reinstate a
  // session the shell has already released.
  let openEpoch = 0;
  const listeners = new Set<(update: ShellSessionUpdate) => void>();
  const stateListeners = new Set<(session: ShellSessionRecord) => void>();
  const createAttemptId = input.createAttemptId ?? randomUUID;

  const publish = (
    kind: ShellSessionUpdate['kind'],
    promptAttemptId: string | null,
    delivery: ShellSessionUpdate['delivery'],
    stream?: ShellSessionStream,
    failure?: ShellOperationFailure
  ) => {
    updateSeq += 1;
    const update: ShellSessionUpdate = {
      generation: input.generation(),
      sessionId: session.sessionId,
      promptAttemptId,
      updateSeq,
      kind,
      delivery,
      ...(stream ? { stream } : {}),
      ...(failure ? { failure } : {}),
    };
    const stored = copyUpdate(update);
    const storedBytes = Buffer.byteLength(JSON.stringify(stored), 'utf8');
    if (storedBytes <= MAX_TRANSCRIPT_BYTES) {
      transcript.push(stored);
      transcriptBytes += storedBytes;
    } else {
      transcriptTruncated = true;
    }
    while (
      transcript.length > MAX_TRANSCRIPT_UPDATES ||
      (transcriptBytes > MAX_TRANSCRIPT_BYTES && transcript.length > 1)
    ) {
      const removed = transcript.shift();
      if (removed) transcriptBytes -= Buffer.byteLength(JSON.stringify(removed), 'utf8');
      transcriptTruncated = true;
    }
    for (const listener of listeners) listener(copyUpdate(update));
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
    expectedSessionId: string | null,
    operation: () => Promise<Awaited<ReturnType<ShellSessionTransport['resumeSession']>>>
  ) => {
    if (generation !== input.generation()) throw new Error('session request generation is stale');
    if (session.status !== 'none') throw new Error('an active session already exists');
    const epoch = openEpoch;
    updateSeq = 0;
    transcript = [];
    transcriptBytes = 0;
    transcriptTruncated = false;
    pendingHistory = [];
    pendingHistoryBytes = 0;
    session = {
      ...emptySession(),
      sessionId: expectedSessionId ?? '',
      status: kind,
      resumeKind: kind === 'resuming' ? 'resumed' : 'fresh',
      resumeIntegrity: kind === 'resuming' ? 'uncertain' : 'not_applicable',
    };
    publishState();
    try {
      const opened = await operation();
      if (generation !== input.generation()) throw new Error('session request generation is stale');
      if (epoch !== openEpoch) throw new Error('session was released while it was opening');
      assertSessionId(opened.sessionId);
      if (expectedSessionId && opened.sessionId !== expectedSessionId) {
        throw new Error('session transport returned a different sessionId');
      }
      session = {
        sessionId: opened.sessionId,
        status: 'active',
        resumeKind: kind === 'creating' ? 'fresh' : 'resumed',
        resumeIntegrity:
          kind === 'creating' ? 'not_applicable' : (opened.resumeIntegrity ?? 'uncertain'),
        workingDir: opened.workingDir ?? null,
        title: opened.title ?? null,
        providerId: opened.providerId ?? null,
        modelId: opened.modelId ?? null,
        promptAttempt: null,
      };
      publishState();
      for (const stream of pendingHistory) publish('stream', null, 'history', stream);
      pendingHistory = [];
      pendingHistoryBytes = 0;
      return session;
    } catch (error) {
      if (epoch === openEpoch) {
        session = emptySession();
        pendingHistory = [];
        pendingHistoryBytes = 0;
        transcript = [];
        transcriptBytes = 0;
        transcriptTruncated = false;
        publishState();
      }
      throw error;
    }
  };

  return {
    read: () => copySession(session),
    create: (generation) =>
      open(generation, 'creating', null, () => input.transport.createSession()),
    resume: (generation, sessionId) => {
      assertSessionId(sessionId);
      return open(generation, 'resuming', sessionId, () =>
        input.transport.resumeSession(sessionId)
      );
    },
    submit({ generation, sessionId, text, libraryItemIds = [] }) {
      assertCurrent(generation, sessionId);
      assertPrompt(text, libraryItemIds);
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
      publish('started', promptAttemptId, 'live');
      void input.transport
        .prompt({ sessionId, text, messageId: promptAttemptId, libraryItemIds })
        .then(() => {
          if (session.promptAttempt?.id !== promptAttemptId) return;
          const wasCancelling = session.promptAttempt.phase === 'cancelling';
          session = { ...session, promptAttempt: null };
          publishState();
          publish(wasCancelling ? 'cancelled' : 'completed', promptAttemptId, 'live');
        })
        .catch((error) => {
          if (session.promptAttempt?.id !== promptAttemptId) return;
          session = { ...session, promptAttempt: null };
          publishState();
          publish(
            'failed',
            promptAttemptId,
            'live',
            undefined,
            classifyShellOperationFailure('prompt.submit', error)
          );
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
    async setProviderModel({ generation, providerId, modelId }) {
      assertCurrent(generation, session.sessionId);
      if (session.promptAttempt) throw new Error('model changes require an idle session');
      if (!providerId || providerId.length > 256 || !modelId || modelId.length > 512) {
        throw new Error('provider and model must be bounded strings');
      }
      if (!input.transport.setProviderModel) throw new Error('model selection is unavailable');
      await input.transport.setProviderModel({
        sessionId: session.sessionId,
        providerId,
        modelId,
      });
      assertCurrent(generation, session.sessionId);
      session = { ...session, providerId, modelId };
      publishState();
      return copySession(session);
    },
    ingestUpdate(notification) {
      if (notification.sessionId !== session.sessionId) return;
      const stream = projectShellSessionUpdate(notification.update);
      if (!stream) return;
      if (session.status === 'resuming') {
        const bytes = Buffer.byteLength(JSON.stringify(stream), 'utf8');
        if (
          pendingHistory.length < MAX_TRANSCRIPT_UPDATES &&
          pendingHistoryBytes + bytes <= MAX_TRANSCRIPT_BYTES
        ) {
          pendingHistory.push(stream);
          pendingHistoryBytes += bytes;
        } else {
          transcriptTruncated = true;
        }
        return;
      }
      if (session.status === 'active' && session.promptAttempt) {
        publish('stream', session.promptAttempt.id, 'live', stream);
      }
    },
    readTranscript(generation, sessionId) {
      assertCurrent(generation, sessionId);
      const integrity = transcriptTruncated
        ? 'incomplete'
        : session.resumeIntegrity === 'uncertain'
          ? 'resume_uncertain'
          : 'complete';
      return {
        generation,
        sessionId,
        integrity,
        firstSeq: transcript[0]?.updateSeq ?? null,
        lastSeq: transcript[transcript.length - 1]?.updateSeq ?? null,
        truncated: transcriptTruncated,
        updates: transcript.map(copyUpdate),
      };
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
        publish(outcome, attempt.id, 'live');
      }
      session = emptySession();
      updateSeq = 0;
      transcript = [];
      transcriptBytes = 0;
      transcriptTruncated = false;
      pendingHistory = [];
      pendingHistoryBytes = 0;
      publishState();
    },
  };
}
