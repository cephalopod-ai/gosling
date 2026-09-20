import { useEffect, useState } from 'react';
import {
  ARTIFACT_TIMESTAMPS_BATCH_LIMIT,
  ARTIFACT_TIMESTAMPS_REFRESH_EVENT,
  type ArtifactFileTimestampMap,
} from '../types/artifactFileTimestamps';

// A path's on-disk timestamps don't depend on which panel is asking, so this
// cache is shared across every mounted list. It only ever shortens a
// mount-time refresh with no reason to distrust the last read (switching
// back to a tab that was showing the same files moments ago) — a focus or
// explicit ARTIFACT_TIMESTAMPS_REFRESH_EVENT signal always bypasses it, so a
// real change is never hidden behind a stale cache entry for longer than
// this window. An entry is also only reused under the timestampRevision it
// was read for: a changed revision is the caller saying the file changed, and
// nothing else would schedule another read once the cached value was served.
const TIMESTAMP_CACHE_TTL_MS = 3000;
const timestampCache = new Map<
  string,
  { value: ArtifactFileTimestampMap[string]; fetchedAt: number; revision: string | null }
>();

// Exposed so tests can isolate themselves from other tests' cached paths;
// also a reasonable escape hatch if a caller ever needs to force a hard
// refresh regardless of how recently a path was read.
export function clearArtifactFileTimestampCache(): void {
  timestampCache.clear();
}

export function useArtifactFileTimestamps(
  files: Array<{ path: string; timestampRevision?: string }>
): ArtifactFileTimestampMap {
  const requestKey = JSON.stringify(files.map((file) => [file.path, file.timestampRevision]));
  const [snapshot, setSnapshot] = useState<{
    requestKey: string;
    timestamps: ArtifactFileTimestampMap;
  } | null>(null);

  useEffect(() => {
    const requests: Array<[string, string | null]> = JSON.parse(requestKey);
    const revisionByPath = new Map(requests);
    const paths = [...revisionByPath.keys()];
    let cancelled = false;
    let revision = 0;

    const refresh = async (bypassCache: boolean) => {
      const currentRevision = ++revision;
      const timestamps: ArtifactFileTimestampMap = {};
      const now = Date.now();
      const uncachedPaths = bypassCache
        ? paths
        : paths.filter((filePath) => {
            const cached = timestampCache.get(filePath);
            if (
              cached &&
              cached.revision === (revisionByPath.get(filePath) ?? null) &&
              now - cached.fetchedAt < TIMESTAMP_CACHE_TTL_MS
            ) {
              timestamps[filePath] = cached.value;
              return false;
            }
            return true;
          });
      for (
        let offset = 0;
        offset < uncachedPaths.length;
        offset += ARTIFACT_TIMESTAMPS_BATCH_LIMIT
      ) {
        const batch = uncachedPaths.slice(offset, offset + ARTIFACT_TIMESTAMPS_BATCH_LIMIT);
        try {
          const result = await window.electron.getArtifactFileTimestamps(batch);
          for (const filePath of batch) {
            const value = result[filePath] ?? null;
            timestamps[filePath] = value;
            timestampCache.set(filePath, {
              value,
              fetchedAt: Date.now(),
              revision: revisionByPath.get(filePath) ?? null,
            });
          }
        } catch {
          for (const filePath of batch) timestamps[filePath] = null;
        }
        if (cancelled || currentRevision !== revision) return;
      }
      if (!cancelled && currentRevision === revision) setSnapshot({ requestKey, timestamps });
    };

    const onSignal = () => void refresh(true);
    void refresh(false);
    window.addEventListener('focus', onSignal);
    window.addEventListener(ARTIFACT_TIMESTAMPS_REFRESH_EVENT, onSignal);
    return () => {
      cancelled = true;
      window.removeEventListener('focus', onSignal);
      window.removeEventListener(ARTIFACT_TIMESTAMPS_REFRESH_EVENT, onSignal);
    };
  }, [requestKey]);

  // A late response for another list must never supply this list's timestamps.
  return snapshot?.requestKey === requestKey ? snapshot.timestamps : {};
}
