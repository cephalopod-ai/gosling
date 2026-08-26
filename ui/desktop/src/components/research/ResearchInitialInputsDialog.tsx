import { useRef, useState } from 'react';
import { FileText, Paperclip, Trash2 } from 'lucide-react';
import { defineMessages, useIntl } from '../../i18n';
import {
  MAX_RESEARCH_INITIAL_FILE_BYTES,
  MAX_RESEARCH_INITIAL_INPUTS,
  researchInitialInputCount,
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
      'Paste reports, links, notes, or prompts, and add any files the research session should begin with.',
  },
  pasteLabel: {
    id: 'researchInitialInputs.pasteLabel',
    defaultMessage: 'Paste content',
  },
  pastePlaceholder: {
    id: 'researchInitialInputs.pastePlaceholder',
    defaultMessage: 'Paste links, prompts, report excerpts, notes, or other source material…',
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
    defaultMessage: 'PDF, text, data, and image files up to 20 MB each.',
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
  const [open, setOpen] = useState(false);
  const [draftText, setDraftText] = useState(value.text);
  const [draftFiles, setDraftFiles] = useState<ResearchInitialInputFile[]>(value.files);
  const [fileError, setFileError] = useState<string | null>(null);
  const appliedCount = researchInitialInputCount(value);
  const draftCount = researchInitialInputCount({ text: draftText, files: draftFiles });
  const isOverLimit = draftCount > MAX_RESEARCH_INITIAL_INPUTS;

  const handleOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (nextOpen) {
      setDraftText(value.text);
      setDraftFiles(value.files);
      setFileError(null);
    }
  };

  const handleFilesSelected = (event: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFiles = Array.from(event.target.files ?? []);
    event.target.value = '';
    if (selectedFiles.length === 0) return;

    const nextFiles = [...draftFiles];
    let nextError: string | null = null;

    for (const file of selectedFiles) {
      if (file.size > MAX_RESEARCH_INITIAL_FILE_BYTES) {
        nextError = intl.formatMessage(i18n.fileTooLarge, { name: file.name });
        continue;
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
      } catch {
        nextError = intl.formatMessage(i18n.fileUnavailable, { name: file.name });
      }
    }

    setDraftFiles(nextFiles);
    setFileError(nextError);
  };

  const applyInputs = () => {
    if (isOverLimit) return;
    onApply({ text: draftText, files: draftFiles });
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

      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{intl.formatMessage(i18n.title)}</DialogTitle>
          <DialogDescription>{intl.formatMessage(i18n.description)}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <label htmlFor="research-initial-input-text" className="text-sm font-medium">
              {intl.formatMessage(i18n.pasteLabel)}
            </label>
            <textarea
              id="research-initial-input-text"
              value={draftText}
              rows={7}
              maxLength={256 * 1024}
              placeholder={intl.formatMessage(i18n.pastePlaceholder)}
              className="w-full resize-y rounded-lg border border-border-primary bg-background-secondary px-3 py-2 text-sm text-text-primary outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onChange={(event) => setDraftText(event.target.value)}
            />
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h3 className="text-sm font-medium">{intl.formatMessage(i18n.filesHeading)}</h3>
                <p className="text-xs text-text-secondary">{intl.formatMessage(i18n.fileHelp)}</p>
              </div>
              <Button
                type="button"
                variant="outline"
                size="sm"
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
              <ul className="max-h-52 space-y-2 overflow-y-auto" aria-label="Selected files">
                {draftFiles.map((file) => (
                  <li
                    key={file.id}
                    className="flex items-center gap-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-2"
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

          {(isOverLimit || fileError) && (
            <p role="alert" className="text-sm text-red-500">
              {isOverLimit
                ? intl.formatMessage(i18n.tooMany, { count: MAX_RESEARCH_INITIAL_INPUTS })
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
          <Button type="button" disabled={isOverLimit} onClick={applyInputs}>
            {intl.formatMessage(i18n.done)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
