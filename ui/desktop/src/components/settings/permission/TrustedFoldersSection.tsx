import { useCallback, useEffect, useState } from 'react';
import { FolderPlus, X } from 'lucide-react';
import { defineMessages, useIntl } from '../../../i18n';
import { useConfig } from '../../ConfigContext';
import { Button } from '../../ui/button';

export const TRUSTED_DIRS_CONFIG_KEY = 'GOSLING_TRUSTED_DIRS';

const i18n = defineMessages({
  title: { id: 'settings.trustedFolders.title', defaultMessage: 'Trusted folders' },
  description: {
    id: 'settings.trustedFolders.description',
    defaultMessage:
      'Tools may read and write inside these folders without asking, in every session. A workspace folder marked read-only still blocks writes.',
  },
  add: { id: 'settings.trustedFolders.add', defaultMessage: 'Add folder' },
  empty: {
    id: 'settings.trustedFolders.empty',
    defaultMessage: 'No trusted folders. Work outside a session’s own folders asks for approval.',
  },
  remove: { id: 'settings.trustedFolders.remove', defaultMessage: 'Stop trusting {folder}' },
  loadFailed: {
    id: 'settings.trustedFolders.loadFailed',
    defaultMessage: 'Could not load your trusted folders.',
  },
  saveFailed: {
    id: 'settings.trustedFolders.saveFailed',
    defaultMessage: 'Could not save your trusted folders.',
  },
});

function asFolderList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string')
    : [];
}

export function TrustedFoldersSection() {
  const intl = useIntl();
  const { read, upsert } = useConfig();
  const [folders, setFolders] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void read(TRUSTED_DIRS_CONFIG_KEY, false)
      .then((value) => {
        if (!cancelled) setFolders(asFolderList(value));
      })
      .catch(() => {
        if (!cancelled) setError(intl.formatMessage(i18n.loadFailed));
      });
    return () => {
      cancelled = true;
    };
  }, [intl, read]);

  const save = useCallback(
    async (nextFolders: string[]) => {
      const previousFolders = folders;
      setFolders(nextFolders);
      setSaving(true);
      setError('');
      try {
        await upsert(TRUSTED_DIRS_CONFIG_KEY, nextFolders, false);
      } catch {
        setFolders(previousFolders);
        setError(intl.formatMessage(i18n.saveFailed));
      } finally {
        setSaving(false);
      }
    },
    [folders, intl, upsert]
  );

  const addFolder = async () => {
    const result = await window.electron.directoryChooser();
    const selected = result.canceled ? undefined : result.filePaths[0];
    if (!selected || folders.includes(selected)) return;
    await save([...folders, selected]);
  };

  return (
    <div className="space-y-3 px-2 py-2">
      <div>
        <h3 className="text-sm font-medium text-text-primary">{intl.formatMessage(i18n.title)}</h3>
        <p className="mt-1 text-xs text-text-secondary">{intl.formatMessage(i18n.description)}</p>
      </div>
      {folders.length === 0 ? (
        <p className="text-xs text-text-secondary">{intl.formatMessage(i18n.empty)}</p>
      ) : (
        <ul className="space-y-1" aria-live="polite">
          {folders.map((folder) => (
            <li
              key={folder}
              className="flex items-center gap-2 rounded-md border border-border-primary bg-background-secondary px-2 py-1.5"
            >
              <span className="min-w-0 flex-1 truncate font-mono text-xs text-text-primary">
                {folder}
              </span>
              <button
                type="button"
                className="rounded-sm text-text-secondary hover:text-text-primary disabled:opacity-50"
                aria-label={intl.formatMessage(i18n.remove, { folder })}
                disabled={saving}
                onClick={() => void save(folders.filter((candidate) => candidate !== folder))}
              >
                <X className="h-3 w-3" />
              </button>
            </li>
          ))}
        </ul>
      )}
      {error && <p className="text-xs text-red-500">{error}</p>}
      <Button
        type="button"
        variant="secondary"
        size="sm"
        className="gap-2"
        disabled={saving}
        onClick={() => void addFolder()}
      >
        <FolderPlus className="h-4 w-4" />
        {intl.formatMessage(i18n.add)}
      </Button>
    </div>
  );
}
