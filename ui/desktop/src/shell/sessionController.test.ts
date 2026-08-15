import { describe, expect, it, vi } from 'vitest';
import { createShellSessionController } from './sessionController';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('shell session controller', () => {
  it('owns one fenced session and a bounded prompt attempt lifecycle', async () => {
    let generation = 3;
    const prompt = deferred<unknown>();
    const transport = {
      createSession: vi.fn().mockResolvedValue({ sessionId: 'session-a' }),
      resumeSession: vi.fn(),
      prompt: vi.fn().mockReturnValue(prompt.promise),
      cancel: vi.fn().mockResolvedValue(undefined),
    };
    const controller = createShellSessionController({
      transport,
      generation: () => generation,
      createAttemptId: () => 'attempt-a',
    });
    const updates: unknown[] = [];
    controller.onUpdate((update) => updates.push(update));

    await expect(controller.create(3)).resolves.toMatchObject({
      sessionId: 'session-a',
      status: 'active',
      resumeKind: 'fresh',
    });
    expect(controller.submit({ generation: 3, sessionId: 'session-a', text: 'hello' })).toEqual({
      promptAttemptId: 'attempt-a',
    });
    expect(transport.prompt).toHaveBeenCalledWith({
      sessionId: 'session-a',
      text: 'hello',
      messageId: 'attempt-a',
    });
    expect(() =>
      controller.submit({ generation: 3, sessionId: 'session-a', text: 'again' })
    ).toThrow('already active');
    await controller.cancel({
      generation: 3,
      sessionId: 'session-a',
      promptAttemptId: 'attempt-a',
    });
    expect(transport.cancel).toHaveBeenCalledWith({ sessionId: 'session-a' });
    prompt.resolve({ stopReason: 'cancelled' });
    await Promise.resolve();
    await Promise.resolve();
    expect(controller.read().promptAttempt).toBeNull();
    expect(updates).toEqual([
      expect.objectContaining({ kind: 'started', updateSeq: 1 }),
      expect.objectContaining({ kind: 'cancelled', updateSeq: 2 }),
    ]);
    generation = 4;
    await expect(controller.create(3)).rejects.toThrow('generation is stale');
  });

  it('rejects malformed, oversized, stale, and foreign prompt operations', async () => {
    const controller = createShellSessionController({
      generation: () => 1,
      transport: {
        createSession: vi.fn().mockResolvedValue({ sessionId: 'session-a' }),
        resumeSession: vi.fn(),
        prompt: vi.fn(),
        cancel: vi.fn(),
      },
    });
    await controller.create(1);
    expect(() => controller.submit({ generation: 1, sessionId: 'other', text: 'hello' })).toThrow(
      'not active'
    );
    expect(() => controller.submit({ generation: 1, sessionId: 'session-a', text: ' ' })).toThrow(
      'non-empty'
    );
    expect(() =>
      controller.submit({ generation: 1, sessionId: 'session-a', text: 'x'.repeat(64 * 1024 + 1) })
    ).toThrow('64 KiB');
    expect(() =>
      controller.submit({ generation: 2, sessionId: 'session-a', text: 'hello' })
    ).toThrow('generation is stale');
  });

  it('uses the server-derived compacted-resume integrity result', async () => {
    const controller = createShellSessionController({
      generation: () => 1,
      transport: {
        createSession: vi.fn(),
        resumeSession: vi.fn().mockResolvedValue({
          sessionId: 'session-a',
          resumeIntegrity: 'clean',
        }),
        prompt: vi.fn(),
        cancel: vi.fn(),
      },
    });
    await expect(controller.resume(1, 'session-a')).resolves.toMatchObject({
      resumeKind: 'resumed',
      resumeIntegrity: 'clean',
    });
  });

  it('preserves uncertainty when a resume transport omits a durable outcome', async () => {
    const controller = createShellSessionController({
      generation: () => 1,
      transport: {
        createSession: vi.fn(),
        resumeSession: vi.fn().mockResolvedValue({ sessionId: 'session-a' }),
        prompt: vi.fn(),
        cancel: vi.fn(),
      },
    });

    await expect(controller.resume(1, 'session-a')).resolves.toMatchObject({
      resumeIntegrity: 'uncertain',
    });
  });

  it('publishes a terminal failure before clearing an interrupted prompt attempt', async () => {
    const controller = createShellSessionController({
      generation: () => 1,
      createAttemptId: () => 'attempt-a',
      transport: {
        createSession: vi.fn().mockResolvedValue({ sessionId: 'session-a' }),
        resumeSession: vi.fn(),
        prompt: vi.fn().mockReturnValue(new Promise(() => {})),
        cancel: vi.fn(),
      },
    });
    const updates: unknown[] = [];
    controller.onUpdate((update) => updates.push(update));

    await controller.create(1);
    controller.submit({ generation: 1, sessionId: 'session-a', text: 'interrupt me' });
    controller.close('failed');

    expect(updates).toEqual([
      expect.objectContaining({ kind: 'started', promptAttemptId: 'attempt-a' }),
      expect.objectContaining({ kind: 'failed', promptAttemptId: 'attempt-a' }),
    ]);
    expect(controller.read()).toMatchObject({ status: 'none', promptAttempt: null });
  });
});

describe('shell session release', () => {
  it('never lets an in-flight open reinstate a session that was already released', async () => {
    let release!: (value: { sessionId: string }) => void;
    const transport = {
      createSession: vi.fn(
        () => new Promise<{ sessionId: string }>((resolve) => (release = resolve))
      ),
      resumeSession: vi.fn(),
      prompt: vi.fn(),
      cancel: vi.fn(),
    };
    const controller = createShellSessionController({ transport, generation: () => 1 });

    const opening = controller.create(1);
    expect(controller.read().status).toBe('creating');

    controller.close();
    release({ sessionId: 'session-a' });

    await expect(opening).rejects.toThrow('released while it was opening');
    expect(controller.read().status).toBe('none');
    expect(controller.read().sessionId).toBe('');
  });
});
