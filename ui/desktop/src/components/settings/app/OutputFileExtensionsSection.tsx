import { useEffect, useState, type FormEvent } from 'react';
import { RotateCcw, X } from 'lucide-react';
import { defineMessages, useIntl } from '../../../i18n';
import {
  defaultOutputFileExtensions,
  defaultSettings,
  isSettingValue,
  normalizeOutputFileExtension,
  OUTPUT_FILE_EXTENSIONS_CHANGED_EVENT,
} from '../../../utils/settings';
import { Button } from '../../ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import { Input } from '../../ui/input';

const i18n = defineMessages({
  title: { id: 'settings.outputFileExtensions.title', defaultMessage: 'Output files' },
  description: {
    id: 'settings.outputFileExtensions.description',
    defaultMessage: 'Choose which file extensions are listed in the Outputs side panel.',
  },
  inputLabel: {
    id: 'settings.outputFileExtensions.inputLabel',
    defaultMessage: 'Add file extensions',
  },
  inputPlaceholder: {
    id: 'settings.outputFileExtensions.inputPlaceholder',
    defaultMessage: '.csv, .html',
  },
  inputHint: {
    id: 'settings.outputFileExtensions.inputHint',
    defaultMessage: 'Separate multiple extensions with commas or spaces.',
  },
  add: { id: 'settings.outputFileExtensions.add', defaultMessage: 'Add' },
  reset: { id: 'settings.outputFileExtensions.reset', defaultMessage: 'Reset defaults' },
  remove: {
    id: 'settings.outputFileExtensions.remove',
    defaultMessage: 'Remove .{extension}',
  },
  invalid: {
    id: 'settings.outputFileExtensions.invalid',
    defaultMessage: 'Enter a valid extension, such as .csv or .tar.gz.',
  },
  saveFailed: {
    id: 'settings.outputFileExtensions.saveFailed',
    defaultMessage: 'Could not save output file extensions.',
  },
});

function notifyOutputFileExtensionsChanged(extensions: string[]) {
  window.dispatchEvent(
    new CustomEvent(OUTPUT_FILE_EXTENSIONS_CHANGED_EVENT, { detail: extensions })
  );
}

export default function OutputFileExtensionsSection() {
  const intl = useIntl();
  const [extensions, setExtensions] = useState<string[]>(defaultSettings.outputFileExtensions);
  const [input, setInput] = useState('');
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void window.electron.getSetting('outputFileExtensions').then((storedExtensions) => {
      if (!cancelled && isSettingValue('outputFileExtensions', storedExtensions)) {
        setExtensions(storedExtensions);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const save = async (nextExtensions: string[]) => {
    const previousExtensions = extensions;
    setExtensions(nextExtensions);
    setSaving(true);
    setError('');
    try {
      await window.electron.setSetting('outputFileExtensions', nextExtensions);
      notifyOutputFileExtensionsChanged(nextExtensions);
      return true;
    } catch {
      setExtensions(previousExtensions);
      setError(intl.formatMessage(i18n.saveFailed));
      return false;
    } finally {
      setSaving(false);
    }
  };

  const addExtensions = async (event: FormEvent) => {
    event.preventDefault();
    const candidates = input.split(/[,\s]+/).filter(Boolean);
    const normalized = candidates.map(normalizeOutputFileExtension);
    if (normalized.length === 0 || normalized.some((extension) => extension === null)) {
      setError(intl.formatMessage(i18n.invalid));
      return;
    }

    const nextExtensions = [...new Set([...extensions, ...(normalized as string[])])];
    if (await save(nextExtensions)) setInput('');
  };

  const removeExtension = (extension: string) => {
    void save(extensions.filter((candidate) => candidate !== extension));
  };

  const resetDefaults = () => {
    void save([...defaultOutputFileExtensions]);
  };

  return (
    <Card className="rounded-lg">
      <CardHeader className="pb-0">
        <CardTitle className="mb-1">{intl.formatMessage(i18n.title)}</CardTitle>
        <CardDescription>{intl.formatMessage(i18n.description)}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4 px-4 pt-4">
        <form className="flex items-end gap-2" onSubmit={(event) => void addExtensions(event)}>
          <div className="min-w-0 flex-1">
            <label
              className="mb-1.5 block text-xs font-medium text-text-primary"
              htmlFor="output-file-extension-input"
            >
              {intl.formatMessage(i18n.inputLabel)}
            </label>
            <Input
              id="output-file-extension-input"
              value={input}
              onChange={(event) => setInput(event.target.value)}
              placeholder={intl.formatMessage(i18n.inputPlaceholder)}
              disabled={saving}
            />
          </div>
          <Button type="submit" variant="secondary" size="sm" disabled={saving || !input.trim()}>
            {intl.formatMessage(i18n.add)}
          </Button>
        </form>
        <p className="text-xs text-text-secondary">{intl.formatMessage(i18n.inputHint)}</p>
        {error && <p className="text-xs text-red-500">{error}</p>}
        <div className="flex flex-wrap gap-2" aria-live="polite">
          {extensions.map((extension) => (
            <span
              key={extension}
              className="inline-flex items-center gap-1 rounded-md border border-border-primary bg-background-secondary px-2 py-1 font-mono text-xs text-text-primary"
            >
              .{extension}
              <button
                type="button"
                className="rounded-sm text-text-secondary hover:text-text-primary disabled:opacity-50"
                aria-label={intl.formatMessage(i18n.remove, { extension })}
                onClick={() => removeExtension(extension)}
                disabled={saving}
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          ))}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="gap-2"
          onClick={resetDefaults}
          disabled={saving}
        >
          <RotateCcw className="h-4 w-4" />
          {intl.formatMessage(i18n.reset)}
        </Button>
      </CardContent>
    </Card>
  );
}
