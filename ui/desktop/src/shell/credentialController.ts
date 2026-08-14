import type {
  ShellCredentialListResponse_unstable,
  ShellCredentialSummary,
} from '@repo-makeover/gosling-sdk';
import type { ShellSettingsStore } from './localSettings';

const MAX_CREDENTIAL_PROFILES = 128;
const MAX_CREDENTIAL_FIELD_LENGTH = 256;

export type ShellCredentialCatalogStatus = 'available' | 'denied' | 'unavailable';
export type ShellCredentialSelectionStatus =
  | 'none'
  | 'configured'
  | 'relink_required'
  | 'missing';

export interface ShellCredentialSnapshot {
  catalogStatus: ShellCredentialCatalogStatus;
  profiles: ShellCredentialSummary[];
  selectedProfileId: string | null;
  selectionStatus: ShellCredentialSelectionStatus;
}

export interface ShellCredentialController {
  read(): ShellCredentialSnapshot;
  selected(): string | null;
  refresh(): Promise<ShellCredentialSnapshot>;
  select(generation: number, profileId: string | null): Promise<ShellCredentialSnapshot>;
  clear(): void;
  onChanged(listener: (snapshot: ShellCredentialSnapshot) => void): () => void;
}

export interface ShellCredentialDependencies {
  settings: ShellSettingsStore;
  list(): Promise<ShellCredentialListResponse_unstable>;
  generation(): number;
}

function isSafeSummary(value: unknown): value is ShellCredentialSummary {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (
    keys.length !== 4 ||
    keys[0] !== 'id' ||
    keys[1] !== 'name' ||
    keys[2] !== 'providerOrServiceId' ||
    keys[3] !== 'status'
  ) {
    return false;
  }
  return (
    ['id', 'name', 'providerOrServiceId'].every((key) => {
      const entry = record[key];
      return (
        typeof entry === 'string' && entry.length > 0 && entry.length <= MAX_CREDENTIAL_FIELD_LENGTH
      );
    }) &&
    (record.status === 'configured' || record.status === 'relink_required')
  );
}

function emptySnapshot(): ShellCredentialSnapshot {
  return {
    catalogStatus: 'denied',
    profiles: [],
    selectedProfileId: null,
    selectionStatus: 'none',
  };
}

function copySnapshot(snapshot: ShellCredentialSnapshot): ShellCredentialSnapshot {
  return {
    ...snapshot,
    profiles: snapshot.profiles.map((profile) => ({ ...profile })),
  };
}

/// Tracks which Gosling credential profile this shell prefers, without ever holding a credential.
///
/// The preference is one opaque profile ID; the backend re-resolves it at session creation, so a
/// deleted or revoked profile becomes visibly invalid rather than being silently replaced.
export function createShellCredentialController(
  dependencies: ShellCredentialDependencies
): ShellCredentialController {
  let snapshot = emptySnapshot();
  const listeners = new Set<(snapshot: ShellCredentialSnapshot) => void>();

  const selectionStatus = (
    profiles: ShellCredentialSummary[],
    selectedProfileId: string | null
  ): ShellCredentialSelectionStatus => {
    if (!selectedProfileId) return 'none';
    const profile = profiles.find((entry) => entry.id === selectedProfileId);
    if (!profile) return 'missing';
    return profile.status === 'configured' ? 'configured' : 'relink_required';
  };

  const publish = (next: ShellCredentialSnapshot) => {
    snapshot = next;
    for (const listener of listeners) listener(copySnapshot(snapshot));
    return copySnapshot(snapshot);
  };

  // Every publish carries the sequence of the request that produced it, so a slower earlier
  // selection can never overwrite the snapshot a later one already committed and persisted.
  let issued = 0;
  let published = 0;

  const load = async (selectedProfileId: string | null) => {
    issued += 1;
    const sequence = issued;
    const response = await dependencies.list();
    if (sequence < published) {
      return copySnapshot(snapshot);
    }
    published = sequence;
    const catalogStatus: ShellCredentialCatalogStatus =
      response.status === 'available'
        ? 'available'
        : response.status === 'unavailable'
          ? 'unavailable'
          : 'denied';
    const profiles =
      catalogStatus === 'available'
        ? (response.profiles ?? []).filter(isSafeSummary).slice(0, MAX_CREDENTIAL_PROFILES)
        : [];
    return publish({
      catalogStatus,
      profiles,
      selectedProfileId,
      selectionStatus: selectionStatus(profiles, selectedProfileId),
    });
  };

  return {
    read: () => copySnapshot(snapshot),
    selected: () => snapshot.selectedProfileId,
    refresh: () => load(dependencies.settings.read().workspace.preferredCredentialProfileId),
    async select(generation, profileId) {
      if (generation !== dependencies.generation()) {
        throw new Error('credential selection generation is stale');
      }
      if (snapshot.catalogStatus !== 'available') {
        throw new Error('credential selection is not permitted for this shell');
      }
      if (profileId !== null && !snapshot.profiles.some((profile) => profile.id === profileId)) {
        throw new Error('credential profile is not in the current safe catalog');
      }
      dependencies.settings.setPreferredCredentialProfileId(profileId);
      return load(profileId);
    },
    clear() {
      issued += 1;
      published = issued;
      publish(emptySnapshot());
    },
    onChanged(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
