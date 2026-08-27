import { useRef, useState } from 'react';
import { ChevronRight, FileText, Paperclip, Trash2 } from 'lucide-react';
import { defineMessages, useIntl } from '../../i18n';
import {
  isResearchInitialImageFile,
  MAX_RESEARCH_INITIAL_FILE_BYTES,
  MAX_RESEARCH_INITIAL_IMAGE_BYTES,
  MAX_RESEARCH_INITIAL_INPUTS,
  MAX_RESEARCH_INITIAL_TEXT_BYTES,
  MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES,
  MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES,
  researchInitialInputCount,
  researchInitialTextBytes,
  type ResearchInitialInputFile,
  type ResearchInitialInputs,
} from '../../types/sessionExperience';
import { Button } from '../ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog';

const supportedFileTypes = [
  '.pdf',
  '.png',
  '.jpg',
  '.jpeg',
  '.webp',
  '.gif',
  '.txt',
  '.md',
  '.csv',
  '.tsv',
  '.json',
  '.yaml',
  '.yml',
  '.xml',
].join(',');

const i18n = defineMessages({
  button: { id: 'researchInitialInputs.button', defaultMessage: 'Initial Inputs' },
  title: { id: 'researchInitialInputs.title', defaultMessage: 'Initial Inputs' },
  description: {
    id: 'researchInitialInputs.description',
    defaultMessage:
      'Paste reports, links, notes, or prompts. Use Next to keep each pasted item separate, and add any files the research session should begin with.',
  },
  pasteLabel: {
    id: 'researchInitialInputs.pasteLabel',
    defaultMessage: 'Paste content',
  },
  pastePlaceholder: {
    id: 'researchInitialInputs.pastePlaceholder',
    defaultMessage: 'Paste links, prompts, report excerpts, notes, or other source material…',
  },
  next: { id: 'researchInitialInputs.next', defaultMessage: 'Next' },
  pastedHeading: {
    id: 'researchInitialInputs.pastedHeading',
    defaultMessage: 'Pasted inputs',
  },
  pastedInput: {
    id: 'researchInitialInputs.pastedInput',
    defaultMessage: 'Pasted input {number}',
  },
  removePastedInput: {
    id: 'researchInitialInputs.removePastedInput',
    defaultMessage: 'Remove pasted input {number}',
  },
  browse: { id: 'researchInitialInputs.browse', defaultMessage: 'Browse files' },
  filePicker: {
    id: 'researchInitialInputs.filePicker',
    defaultMessage: 'Choose initial research files',
  },
  filesHeading: {
    id: 'researchInitialInputs.filesHeading',
    defaultMessage: 'Selected files',
  },
  noFiles: { id: 'researchInitialInputs.noFiles', defaultMessage: 'No files selected.' },
  fileHelp: {
    id: 'researchInitialInputs.fileHelp',
    defaultMessage:
      'Files up to 20 MB each; images up to 5 MB each and 10 MB total. Pasted text is limited to 256 KB each and 512 KB total.',
  },
  removeFile: {
    id: 'researchInitialInputs.removeFile',
    defaultMessage: 'Remove {name}',
  },
  cancel: { id: 'researchInitialInputs.cancel', defaultMessage: 'Cancel' },
  done: { id: 'researchInitialInputs.done', defaultMessage: 'Done' },
  inputCount: {
    id: 'researchInitialInputs.inputCount',
    defaultMessage: '{count, plural, one {# input} other {# inputs}}',
  },
  tooMany: {
    id: 'researchInitialInputs.tooMany',
    defaultMessage: 'Add up to {count} initial inputs in total.',
  },
  fileTooLarge: {
    id: 'researchInitialInputs.fileTooLarge',
    defaultMessage: '{name} is larger than 20 MB.',
  },
  imageTooLarge: {
    id: 'researchInitialInputs.imageTooLarge',
    defaultMessage: '{name} is larger than the 5 MB image limit.',
  },
  imagesTooLarge: {
    id: 'researchInitialInputs.imagesTooLarge',
    defaultMessage: 'Selected images exceed the 10 MB total limit.',
  },
  textTooLarge: {
    id: 'researchInitialInputs.textTooLarge',
    defaultMessage: 'Each pasted input must be no larger than 256 KB.',
  },
  textTotalTooLarge: {
    id: 'researchInitialInputs.textTotalTooLarge',
    defaultMessage: 'Pasted inputs exceed the 512 KB total limit.',
  },
  fileUnavailable: {
    id: 'researchInitialInputs.fileUnavailable',
    defaultMessage: 'Gosling could not access {name}.',
  },
});

function formatFileSize(sizeBytes: number): string {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${Math.ceil(sizeBytes / 1024)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function ResearchInitialInputsDialog({
  value,
  onApply,
}: {
  value: ResearchInitialInputs;
  onApply: (inputs: ResearchInitialInputs) => void;
}) {
  const intl = useIntl();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textInputRef = useRef<HTMLTextAreaElement>(null);
  const [open, setOpen] = useState(false);
  const [draftTexts, setDraftTexts] = useState<string[]>(value.texts);
  const [draftText, setDraftText] = useState('');
  const [draftFiles, setDraftFiles] = useState<ResearchInitialInputFile[]>(value.files);
  const [fileError, setFileError] = useState<string | null>(null);
  const appliedCount = researchInitialInputCount(value);
  const pendingText = draftText.trim();
  const draftCount = researchInitialInputCount({
    texts: [...draftTexts, ...(pendingText ? [pendingText] : [])],
    files: draftFiles,
  });
  const isOverLimit = draftCount > MAX_RESEARCH_INITIAL_INPUTS;
  const pendingTextBytes = researchInitialTextBytes(pendingText);
  const totalTextBytes = [...draftTexts, ...(pendingText ? [pendingText] : [])].reduce(
    (total, text) => total + researchInitialTextBytes(text),
    0
  );
  const isTextTooLarge = pendingTextBytes > MAX_RESEARCH_INITIAL_TEXT_BYTES;
  const isTextTotalTooLarge = totalTextBytes > MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES;
  const canQueueText =
    pendingText.length > 0 &&
    !isTextTooLarge &&
    !isTextTotalTooLarge &&
    draftTexts.length + draftFiles.length < MAX_RESEARCH_INITIAL_INPUTS;

  const handleOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (nextOpen) {
      setDraftTexts(value.texts);
      setDraftText('');
      setDraftFiles(value.files);
      setFileError(null);
    }
  };

  const queueDraftText = () => {
    if (!canQueueText) return;
    setDraftTexts((texts) => [...texts, pendingText]);
    setDraftText('');
    window.requestAnimationFrame(() => textInputRef.current?.focus());
  };

  const handleFilesSelected = (event: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFiles = Array.from(event.target.files ?? []);
    event.target.value = '';
    if (selectedFiles.length === 0) return;

    const nextFiles = [...draftFiles];
    let selectedImageBytes = nextFiles
      .filter((file) => isResearchInitialImageFile(file.name))
      .reduce((total, file) => total + file.sizeBytes, 0);
    let nextError: string | null = null;

    for (const file of selectedFiles) {
      if (file.size > MAX_RESEARCH_INITIAL_FILE_BYTES) {
        nextError = intl.formatMessage(i18n.fileTooLarge, { name: file.name });
        continue;
      }
      if (isResearchInitialImageFile(file.name)) {
        if (file.size > MAX_RESEARCH_INITIAL_IMAGE_BYTES) {
          nextError = intl.formatMessage(i18n.imageTooLarge, { name: file.name });
          continue;
        }
        if (selectedImageBytes + file.size > MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES) {
          nextError = intl.formatMessage(i18n.imagesTooLarge);
          continue;
        }
      }

      try {
        const path = window.electron.getPathForFile(file);
        if (nextFiles.some((candidate) => candidate.path === path)) continue;
        nextFiles.push({
          id: `${path}:${file.size}`,
          name: file.name,
          path,
          sizeBytes: file.size,
        });
        if (isResearchInitialImageFile(file.name)) selectedImageBytes += file.size;
      } catch {
        nextError = intl.formatMessage(i18n.fileUnavailable, { name: file.name });
      }
    }

    setDraftFiles(nextFiles);
    setFileError(nextError);
  };

  const applyInputs = () => {
    if (isOverLimit || isTextTooLarge || isTextTotalTooLarge) return;
    onApply({
      texts: [...draftTexts, ...(pendingText ? [pendingText] : [])],
      files: draftFiles,
    });
    setOpen(false);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="mt-3 gap-2"
        onClick={() => handleOpenChange(true)}
      >
        <Paperclip className="h-4 w-4" />
        {intl.formatMessage(i18n.button)}
        {appliedCount > 0 && (
          <span className="rounded-full bg-background-tertiary px-2 py-0.5 text-xs">
            {intl.formatMessage(i18n.inputCount, { count: appliedCount })}
          </span>
        )}
      </Button>

      <DialogContent className="min-w-0 max-h-[85vh] overflow-x-hidden overflow-y-auto sm:max-w-2xl">
        <DialogHeader className="min-w-0">
          <DialogTitle>{intl.formatMessage(i18n.title)}</DialogTitle>
          <DialogDescription className="break-words">
            {intl.formatMessage(i18n.description)}
          </DialogDescription>
        </DialogHeader>

        <div className="min-w-0 space-y-4">
          <div className="min-w-0 space-y-2">
            <label htmlFor="research-initial-input-text" className="text-sm font-medium">
              {intl.formatMessage(i18n.pasteLabel)}
            </label>
            <textarea
              ref={textInputRef}
              id="research-initial-input-text"
              value={draftText}
              rows={7}
              wrap="soft"
              placeholder={intl.formatMessage(i18n.pastePlaceholder)}
              className="block min-w-0 max-w-full w-full resize-y whitespace-pre-wrap break-words [overflow-wrap:anywhere] rounded-lg border border-border-primary bg-background-secondary px-3 py-2 text-sm text-text-primary outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onChange={(event) => setDraftText(event.target.value)}
            />
            <div className="flex justify-end">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="gap-1"
                disabled={!canQueueText}
                onClick={queueDraftText}
              >
                {intl.formatMessage(i18n.next)}
                <ChevronRight className="h-4 w-4" />
              </Button>
            </div>

            {draftTexts.length > 0 && (
              <div className="min-w-0 space-y-2">
                <h3 className="text-sm font-medium">{intl.formatMessage(i18n.pastedHeading)}</h3>
                <ul
                  className="min-w-0 max-w-full max-h-40 space-y-2 overflow-x-hidden overflow-y-auto"
                  aria-label="Pasted inputs"
                >
                  {draftTexts.map((text, index) => (
                    <li
                      key={`${index}:${text.slice(0, 32)}`}
                      className="flex w-full min-w-0 items-start gap-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-2"
                    >
                      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-background-tertiary text-xs text-text-secondary">
                        {index + 1}
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="text-xs font-medium text-text-secondary">
                          {intl.formatMessage(i18n.pastedInput, { number: index + 1 })}
                        </p>
                        <p className="max-h-24 overflow-y-auto whitespace-pre-wrap break-words [overflow-wrap:anywhere] text-sm text-text-primary">
                          {text}
                        </p>
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="xs"
                        shape="round"
                        className="shrink-0"
                        aria-label={intl.formatMessage(i18n.removePastedInput, {
                          number: index + 1,
                        })}
                        onClick={() =>
                          setDraftTexts((texts) =>
                            texts.filter((_, candidateIndex) => candidateIndex !== index)
                          )
                        }
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>

          <div className="min-w-0 space-y-2">
            <div className="flex min-w-0 items-center justify-between gap-3">
              <div className="min-w-0">
                <h3 className="text-sm font-medium">{intl.formatMessage(i18n.filesHeading)}</h3>
                <p className="break-words text-xs text-text-secondary">
                  {intl.formatMessage(i18n.fileHelp)}
                </p>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={() => fileInputRef.current?.click()}
              >
                {intl.formatMessage(i18n.browse)}
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                accept={supportedFileTypes}
                aria-label={intl.formatMessage(i18n.filePicker)}
                className="hidden"
                onChange={handleFilesSelected}
              />
            </div>

            {draftFiles.length === 0 ? (
              <p className="rounded-lg border border-dashed border-border-primary px-3 py-4 text-center text-sm text-text-secondary">
                {intl.formatMessage(i18n.noFiles)}
              </p>
            ) : (
              <ul
                className="min-w-0 max-w-full max-h-52 space-y-2 overflow-x-hidden overflow-y-auto"
                aria-label="Selected files"
              >
                {draftFiles.map((file) => (
                  <li
                    key={file.id}
                    className="flex w-full min-w-0 items-center gap-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-2"
                  >
                    <FileText className="h-4 w-4 shrink-0 text-text-secondary" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm text-text-primary">{file.name}</p>
                      <p className="text-xs text-text-secondary">
                        {formatFileSize(file.sizeBytes)}
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="ghost"
                      size="xs"
                      shape="round"
                      className="shrink-0"
                      aria-label={intl.formatMessage(i18n.removeFile, { name: file.name })}
                      onClick={() =>
                        setDraftFiles((files) =>
                          files.filter((candidate) => candidate.id !== file.id)
                        )
                      }
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {(isOverLimit || isTextTooLarge || isTextTotalTooLarge || fileError) && (
            <p role="alert" className="text-sm text-red-500">
              {isOverLimit
                ? intl.formatMessage(i18n.tooMany, { count: MAX_RESEARCH_INITIAL_INPUTS })
                : isTextTooLarge
                  ? intl.formatMessage(i18n.textTooLarge)
                  : isTextTotalTooLarge
                    ? intl.formatMessage(i18n.textTotalTooLarge)
                    : fileError}
            </p>
          )}
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="outline">
              {intl.formatMessage(i18n.cancel)}
            </Button>
          </DialogClose>
          <Button
            type="button"
            disabled={isOverLimit || isTextTooLarge || isTextTotalTooLarge}
            onClick={applyInputs}
          >
            {intl.formatMessage(i18n.done)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
