// @vitest-environment node
import { beforeEach, expect, it, vi } from 'vitest';
import { getAcpClient } from './acpConnection';
import { getLatestOutputRevision } from './outputRevisions';

vi.mock('./acpConnection', () => ({ getAcpClient: vi.fn() }));

const latestBatch = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getAcpClient).mockResolvedValue({
    gosling: { sessionOutputsLatestBatch_unstable: latestBatch },
  } as never);
});

it('coalesces same-tick requests for one session into a single batch call', async () => {
  latestBatch.mockResolvedValue({
    revisions: [
      { path: '/Outputs/a.md', revision: { version: 1 } },
      { path: '/Outputs/b.md', revision: null },
    ],
  });
  const results = await Promise.all([
    getLatestOutputRevision('chat', '/Outputs/a.md', new AbortController().signal),
    getLatestOutputRevision('chat', '/Outputs/b.md', new AbortController().signal),
  ]);
  expect(latestBatch).toHaveBeenCalledTimes(1);
  expect(latestBatch).toHaveBeenCalledWith({
    sessionId: 'chat',
    paths: ['/Outputs/a.md', '/Outputs/b.md'],
  });
  expect(results).toEqual([{ version: 1 }, null]);
});

it('sends one batch per distinct session rather than merging across sessions', async () => {
  latestBatch.mockResolvedValue({ revisions: [] });
  await Promise.all([
    getLatestOutputRevision('chat-1', '/Outputs/a.md', new AbortController().signal).catch(() => null),
    getLatestOutputRevision('chat-2', '/Outputs/a.md', new AbortController().signal).catch(() => null),
  ]);
  expect(latestBatch).toHaveBeenCalledTimes(2);
  const sessionIds = latestBatch.mock.calls.map(([request]) => request.sessionId).sort();
  expect(sessionIds).toEqual(['chat-1', 'chat-2']);
});

it('rejects only the paths a batch response omits, and resolves the rest', async () => {
  latestBatch.mockResolvedValue({ revisions: [{ path: '/Outputs/a.md', revision: null }] });
  const dropped = getLatestOutputRevision('chat', '/Outputs/dropped.md', new AbortController().signal);
  const kept = getLatestOutputRevision('chat', '/Outputs/a.md', new AbortController().signal);
  await expect(dropped).rejects.toThrow('History unavailable');
  await expect(kept).resolves.toBeNull();
});

it('rejects every waiter in a batch when the request itself fails', async () => {
  latestBatch.mockRejectedValue(new Error('backend unreachable'));
  const requests = Promise.all([
    getLatestOutputRevision('chat', '/Outputs/a.md', new AbortController().signal).catch((e) => e.message),
    getLatestOutputRevision('chat', '/Outputs/b.md', new AbortController().signal).catch((e) => e.message),
  ]);
  expect(await requests).toEqual(['backend unreachable', 'backend unreachable']);
});

it('rejects immediately on an already-aborted signal without joining a batch', async () => {
  const controller = new AbortController();
  controller.abort();
  await expect(getLatestOutputRevision('chat', '/Outputs/a.md', controller.signal)).rejects.toBeTruthy();
  expect(latestBatch).not.toHaveBeenCalled();
});

it('a later request for a session starts a fresh batch after the first flushes', async () => {
  latestBatch.mockResolvedValue({ revisions: [{ path: '/Outputs/a.md', revision: null }] });
  await getLatestOutputRevision('chat', '/Outputs/a.md', new AbortController().signal);
  await getLatestOutputRevision('chat', '/Outputs/a.md', new AbortController().signal);
  expect(latestBatch).toHaveBeenCalledTimes(2);
});

it('splits a tick larger than the backend batch cap instead of losing the overflow', async () => {
  latestBatch.mockImplementation(async ({ paths }: { paths: string[] }) => ({
    revisions: paths.map((path) => ({ path, revision: null })),
  }));
  const paths = Array.from({ length: 501 }, (_, index) => `/Outputs/${index}.md`);
  const results = await Promise.all(
    paths.map((path) => getLatestOutputRevision('chat', path, new AbortController().signal))
  );
  expect(latestBatch.mock.calls.map(([request]) => request.paths.length)).toEqual([500, 1]);
  expect(results).toHaveLength(501);
  expect(results.every((revision) => revision === null)).toBe(true);
});
