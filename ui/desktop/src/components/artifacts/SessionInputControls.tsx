import { useRef, useState } from 'react';
import { FilePlus, ClipboardPaste } from 'lucide-react';
import type { ShellLibraryItemSummary } from '@repo-makeover/gosling-sdk';
import {
  addSessionLibraryText,
  compileSessionLibraryInputs,
  linkSessionLibraryFile,
  sessionInputSelectionSize,
} from '../../acp/sessionLibraryInputs';
import {
  clearSelectedSessionInputs,
  setSessionInputSelected,
  useSelectedSessionInputs,
} from '../../acp/sessionInputSelection';
import { describeAcpError, parseAcpLibraryError } from '../../acp/errors';
import { acpChatSessionController } from '../../acp/chatSessionController';
import { acpChatSessionActions } from '../../acp/chatSessionStore';
import { acpListSessionArtifacts } from '../../acp/sessions';
import { useArtifactWorkbench } from '../../contexts/ArtifactWorkbenchContext';
import { defineMessages, useIntl } from '../../i18n';
import {
  MAX_RESEARCH_INITIAL_TEXT_BYTES,
  MAX_RESEARCH_INITIAL_INPUTS,
  MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES,
  MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES,
  RESEARCH_INITIAL_FILE_ACCEPT,
  researchInitialTextBytes,
} from '../../types/sessionExperience';
import { Button } from '../ui/button';

const i18n = defineMessages({
  addFile: { id: 'sessionInputs.addFile', defaultMessage: 'Add file' },
  pasteText: { id: 'sessionInputs.pasteText', defaultMessage: 'Paste text' },
  name: { id: 'sessionInputs.name', defaultMessage: 'Name (optional)' },
  content: { id: 'sessionInputs.content', defaultMessage: 'Text content' },
  placeholder: {
    id: 'sessionInputs.placeholder',
    defaultMessage: 'Paste notes, links, excerpts, or other source material…',
  },
  defaultName: { id: 'sessionInputs.defaultName', defaultMessage: 'Pasted text' },
  save: { id: 'sessionInputs.save', defaultMessage: 'Add text' },
  saving: { id: 'sessionInputs.saving', defaultMessage: 'Adding…' },
  cancel: { id: 'sessionInputs.cancel', defaultMessage: 'Cancel' },
  textLimit: {
    id: 'sessionInputs.textLimit',
    defaultMessage: 'Each pasted input must be no larger than 256 KB.',
  },
  failed: {
    id: 'sessionInputs.failed',
    defaultMessage: 'Unable to add input: {error}',
  },
  help: {
    id: 'sessionInputs.help',
    defaultMessage:
      'Selected inputs accompany your next message. Large selections are compiled into a source file for the agent to read in sections.',
  },
  selectAll: { id: 'sessionInputs.selectAll', defaultMessage: 'Select all' },
  clearSelection: { id: 'sessionInputs.clearSelection', defaultMessage: 'Clear selection' },
  compileAll: { id: 'sessionInputs.compileAll', defaultMessage: 'Compile all inputs' },
  compiling: { id: 'sessionInputs.compiling', defaultMessage: 'Compiling complete sources…' },
  compiled: {
    id: 'sessionInputs.compiled',
    defaultMessage: 'Compiled {count} inputs into one source file.',
  },
  openCompilation: { id: 'sessionInputs.openCompilation', defaultMessage: 'Open compilation' },
  selectionBudget: {
    id: 'sessionInputs.selectionBudget',
    defaultMessage: '{count} selected · {text} / 512 KiB text · {images} / 10 MiB images',
  },
  linkedBudget: {
    id: 'sessionInputs.linkedBudget',
    defaultMessage: 'Linked-file content is checked when sending.',
  },
  compilationNeeded: {
    id: 'sessionInputs.compilationNeeded',
    defaultMessage:
      'This selection exceeds the inline limit. Sending will save a complete source compilation in the chat folder and attach its location.',
  },
  compilationHelp: {
    id: 'sessionInputs.compilationHelp',
    defaultMessage:
      'Compile all inputs saves every listed source, including unchecked inputs. It preserves source text and citations; ask the agent to write the synthesized report.',
  },
  compileFailed: {
    id: 'sessionInputs.compileFailed',
    defaultMessage: 'Unable to compile inputs: {error}',
  },
  outputRefreshFailed: {
    id: 'sessionInputs.outputRefreshFailed',
    defaultMessage: 'Compilation saved, but Outputs could not be refreshed: {error}',
  },
  noSession: {
    id: 'sessionInputs.noSession',
    defaultMessage: 'Open a chat to add inputs.',
  },
});

export function SessionInputControls({
  sessionId,
  items,
  onAdded,
}: {
  sessionId: string | null;
  items: ShellLibraryItemSummary[];
  onAdded: (item: ShellLibraryItemSummary) => void;
}) {
  const intl = useIntl();
  const fileInput = useRef<HTMLInputElement>(null);
  const textInput = useRef<HTMLTextAreaElement>(null);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState('');
  const [text, setText] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [compiling, setCompiling] = useState(false);
  const [compiled, setCompiled] = useState<{ filePath: string; sourceCount: number } | null>(null);
  const selected = useSelectedSessionInputs(sessionId);
  const size = sessionInputSelectionSize(items, selected);
  const needsCompilation =
    selected.length > MAX_RESEARCH_INITIAL_INPUTS ||
    size.textBytes > MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES ||
    size.imageBytes > MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES;
  const { openFile } = useArtifactWorkbench();
  const textTooLarge = researchInitialTextBytes(text) > MAX_RESEARCH_INITIAL_TEXT_BYTES;

  const add = async (operation: () => Promise<ShellLibraryItemSummary>) => {
    if (!sessionId || saving) return;
    setSaving(true);
    setError(null);
    try {
      if (!(await acpChatSessionController.loadSession(sessionId))) {
        throw new Error('The session could not be loaded. Retry after reconnecting.');
      }
      const item = await operation();
      setSessionInputSelected(sessionId, item.id, true);
      onAdded(item);
      return item;
    } catch (cause) {
      setError(intl.formatMessage(i18n.failed, { error: describeAcpError(cause) }));
      return undefined;
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="no-drag shrink-0 space-y-2 border-b border-border-primary p-3">
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={!sessionId || saving}
          onClick={() => fileInput.current?.click()}
        >
          <FilePlus className="mr-1.5 h-4 w-4" />
          {intl.formatMessage(i18n.addFile)}
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={!sessionId || saving}
          onClick={() => {
            setEditing(true);
            setError(null);
            window.requestAnimationFrame(() => textInput.current?.focus());
          }}
        >
          <ClipboardPaste className="mr-1.5 h-4 w-4" />
          {intl.formatMessage(i18n.pasteText)}
        </Button>
        <input
          ref={fileInput}
          type="file"
          className="hidden"
          aria-label={intl.formatMessage(i18n.addFile)}
          accept={RESEARCH_INITIAL_FILE_ACCEPT}
          disabled={!sessionId || saving}
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = '';
            if (file && sessionId) void add(() => linkSessionLibraryFile(sessionId, file));
          }}
        />
      </div>
      <p className="text-xs text-text-secondary">
        {intl.formatMessage(sessionId ? i18n.help : i18n.noSession)}
      </p>
      {sessionId && items.length > 0 && (
        <div className="space-y-2">
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={compiling}
              onClick={() =>
                items.forEach((item) => setSessionInputSelected(sessionId, item.id, true))
              }
            >
              {intl.formatMessage(i18n.selectAll)}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={compiling || selected.length === 0}
              onClick={() => clearSelectedSessionInputs(sessionId, selected)}
            >
              {intl.formatMessage(i18n.clearSelection)}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={compiling || saving}
              onClick={async () => {
                setCompiling(true);
                setCompiled(null);
                setError(null);
                try {
                  if (!(await acpChatSessionController.loadSession(sessionId)))
                    throw new Error('The session could not be loaded. Retry after reconnecting.');
                  const result = await compileSessionLibraryInputs(
                    sessionId,
                    items.map((item) => item.id)
                  );
                  setCompiled(result);
                  try {
                    const artifacts = await acpListSessionArtifacts(sessionId);
                    acpChatSessionActions.setArtifacts(sessionId, artifacts);
                  } catch (cause) {
                    setError(
                      intl.formatMessage(i18n.outputRefreshFailed, {
                        error: describeAcpError(cause),
                      })
                    );
                  }
                } catch (cause) {
                  setError(
                    intl.formatMessage(i18n.compileFailed, {
                      error: parseAcpLibraryError(cause)?.message ?? describeAcpError(cause),
                    })
                  );
                } finally {
                  setCompiling(false);
                }
              }}
            >
              {intl.formatMessage(i18n.compileAll)}
            </Button>
          </div>
          <p className="text-xs text-text-secondary" aria-live="polite">
            {intl.formatMessage(i18n.selectionBudget, {
              count: selected.length,
              text: Math.ceil(size.textBytes / 1024),
              images: (size.imageBytes / (1024 * 1024)).toFixed(1),
            })}
            {size.linkedFiles > 0 && <> {intl.formatMessage(i18n.linkedBudget)}</>}
          </p>
          {needsCompilation && (
            <p role="status" className="text-xs text-text-secondary">
              {intl.formatMessage(i18n.compilationNeeded)}
            </p>
          )}
          <p className="text-xs text-text-secondary">{intl.formatMessage(i18n.compilationHelp)}</p>
          {compiling && (
            <p role="status" className="text-xs text-text-secondary">
              {intl.formatMessage(i18n.compiling)}
            </p>
          )}
          {compiled && (
            <div className="space-y-1 text-xs text-text-secondary" role="status">
              <p>{intl.formatMessage(i18n.compiled, { count: compiled.sourceCount })}</p>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => openFile(compiled.filePath)}
              >
                {intl.formatMessage(i18n.openCompilation)}
              </Button>
            </div>
          )}
        </div>
      )}
      {editing && sessionId && (
        <form
          className="min-w-0 space-y-2"
          onSubmit={async (event) => {
            event.preventDefault();
            if (!text.trim() || textTooLarge) return;
            const item = await add(() =>
              addSessionLibraryText(
                sessionId,
                name.trim() || intl.formatMessage(i18n.defaultName),
                text
              )
            );
            if (item) {
              setName('');
              setText('');
              setEditing(false);
            }
          }}
        >
          <label className="block text-xs">
            {intl.formatMessage(i18n.name)}
            <input
              value={name}
              maxLength={100}
              disabled={saving}
              className="mt-1 w-full rounded-md border border-border-primary bg-background-secondary px-2 py-1.5 text-sm"
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <label className="block text-xs">
            {intl.formatMessage(i18n.content)}
            <textarea
              ref={textInput}
              value={text}
              rows={6}
              disabled={saving}
              aria-invalid={textTooLarge}
              placeholder={intl.formatMessage(i18n.placeholder)}
              className="mt-1 block w-full min-w-0 resize-y whitespace-pre-wrap break-words [overflow-wrap:anywhere] rounded-md border border-border-primary bg-background-secondary px-2 py-1.5 text-sm"
              onChange={(event) => setText(event.target.value)}
            />
          </label>
          {textTooLarge && (
            <p role="alert" className="text-xs text-text-secondary">
              {intl.formatMessage(i18n.textLimit)}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={saving}
              onClick={() => setEditing(false)}
            >
              {intl.formatMessage(i18n.cancel)}
            </Button>
            <Button type="submit" size="sm" disabled={saving || !text.trim() || textTooLarge}>
              {intl.formatMessage(saving ? i18n.saving : i18n.save)}
            </Button>
          </div>
        </form>
      )}
      {saving && (
        <p role="status" className="text-xs text-text-secondary">
          {intl.formatMessage(i18n.saving)}
        </p>
      )}
      {error && (
        <p role="alert" className="break-words text-xs text-text-secondary">
          {error}
        </p>
      )}
    </div>
  );
}
