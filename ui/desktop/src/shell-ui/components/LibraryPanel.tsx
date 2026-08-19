import { useState, type ClipboardEvent } from 'react';
import type { ShellLibraryItemSummary, ShellLibraryScope } from '@repo-makeover/gosling-sdk';
import { COPY } from '../copy';
import { ShellButton } from './primitives';

interface LibraryPanelProps {
  library: {
    status: 'idle' | 'loading' | 'loaded';
    items: ShellLibraryItemSummary[];
    selectedItemIds: string[];
    addScope: ShellLibraryScope;
  };
  canWrite: boolean;
  busy: boolean;
  onScopeChange(scope: ShellLibraryScope): void;
  onToggle(itemId: string): void;
  onAddText(name: string, text: string): Promise<void>;
  onAddImage(name: string, mimeType: string, data: string): Promise<void>;
  onLinkFile(): Promise<void>;
  onRemove(itemId: string): Promise<void>;
}

function readImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error('Could not read pasted image'));
    reader.onload = () => {
      const result = String(reader.result ?? '');
      const separator = result.indexOf(',');
      if (separator < 0) reject(new Error('Pasted image encoding is invalid'));
      else resolve(result.slice(separator + 1));
    };
    reader.readAsDataURL(file);
  });
}

export function LibraryPanel({
  library,
  canWrite,
  busy,
  onScopeChange,
  onToggle,
  onAddText,
  onAddImage,
  onLinkFile,
  onRemove,
}: LibraryPanelProps) {
  const [addingText, setAddingText] = useState(false);
  const [name, setName] = useState('Notes');
  const [text, setText] = useState('');

  const submitText = async () => {
    if (!name.trim() || !text.trim()) return;
    await onAddText(name.trim(), text);
    setText('');
    setAddingText(false);
  };

  const pasteImage = async (event: ClipboardEvent<HTMLElement>) => {
    if (!canWrite || busy) return;
    const file = [...event.clipboardData.items]
      .find((item) => item.kind === 'file' && item.type.startsWith('image/'))
      ?.getAsFile();
    if (!file) return;
    event.preventDefault();
    const data = await readImage(file);
    await onAddImage(file.name || 'Pasted image', file.type, data);
  };

  return (
    <section
      className="gsh-library"
      aria-labelledby="gsh-library-heading"
      onPaste={(event) => void pasteImage(event)}
    >
      <div className="gsh-library__head">
        <h2 id="gsh-library-heading" className="gsh-library__heading">
          {COPY.libraryHeading}
        </h2>
        <select
          aria-label="New library item scope"
          value={library.addScope}
          disabled={!canWrite || busy}
          onChange={(event) => onScopeChange(event.target.value as ShellLibraryScope)}
        >
          <option value="session">This task</option>
          <option value="project">This project</option>
        </select>
      </div>
      <p className="gsh-library__hint">{COPY.libraryHint}</p>
      {canWrite ? (
        <div className="gsh-library__actions">
          <ShellButton
            label={COPY.libraryAddFile}
            onClick={() => void onLinkFile()}
            disabled={busy}
          />
          <ShellButton
            label={COPY.libraryAddText}
            onClick={() => setAddingText((shown) => !shown)}
            disabled={busy}
          />
        </div>
      ) : null}
      {addingText ? (
        <div className="gsh-library__text-form">
          <input
            aria-label="Text item name"
            value={name}
            maxLength={128}
            onChange={(event) => setName(event.target.value)}
          />
          <textarea
            aria-label="Text item content"
            value={text}
            rows={4}
            maxLength={256 * 1024}
            onChange={(event) => setText(event.target.value)}
          />
          <ShellButton
            label={COPY.librarySaveText}
            emphasis="primary"
            onClick={() => void submitText()}
            disabled={busy || !name.trim() || !text.trim()}
          />
        </div>
      ) : null}
      {library.status !== 'loaded' ? (
        <p className="gsh-library__empty" role="status">
          Loading library…
        </p>
      ) : library.items.length === 0 ? (
        <p className="gsh-library__empty">{COPY.libraryEmpty}</p>
      ) : (
        <ul className="gsh-library__list">
          {library.items.map((item) => (
            <li className="gsh-library-item" key={item.id}>
              <label className="gsh-library-item__select">
                <input
                  type="checkbox"
                  checked={library.selectedItemIds.includes(item.id)}
                  disabled={item.status === 'missing' || busy}
                  onChange={() => onToggle(item.id)}
                />
                <span>
                  <span className="gsh-library-item__name">{item.name}</span>
                  <span className="gsh-library-item__meta">
                    {item.kind} · {item.scope}
                    {item.status === 'missing' ? ' · missing' : ''}
                  </span>
                </span>
              </label>
              {canWrite ? (
                <button
                  type="button"
                  className="gsh-library-item__remove"
                  aria-label={`Remove ${item.name}`}
                  disabled={busy}
                  onClick={() => void onRemove(item.id)}
                >
                  Remove
                </button>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
