import type { GoslingExtension } from '@repo-makeover/gosling-sdk';
import { COPY } from '../copy';

interface ExtensionsPanelProps {
  extensions: {
    status: 'idle' | 'loading' | 'loaded';
    available: GoslingExtension[];
    selected: GoslingExtension[];
  };
  canWrite: boolean;
  busy: boolean;
  onAdd(extension: GoslingExtension): Promise<void>;
  onRemove(name: string): Promise<void>;
}

function extensionName(extension: GoslingExtension): string {
  return extension.type === 'mcp' ? extension.server.name : extension.name;
}

function extensionDisplayName(extension: GoslingExtension): string {
  return extension.type === 'mcp'
    ? extension.server.name
    : (extension.display_name ?? extension.name);
}

export function ExtensionsPanel({
  extensions,
  canWrite,
  busy,
  onAdd,
  onRemove,
}: ExtensionsPanelProps) {
  const selectedNames = new Set(extensions.selected.map(extensionName));

  return (
    <section className="gsh-extensions" aria-labelledby="gsh-extensions-heading">
      <div className="gsh-extensions__head">
        <h2 id="gsh-extensions-heading" className="gsh-extensions__heading">
          {COPY.extensionsHeading}
        </h2>
      </div>
      <p className="gsh-extensions__hint">{COPY.extensionsHint}</p>
      {extensions.status !== 'loaded' ? (
        <p className="gsh-extensions__empty" role="status">
          Loading extensions…
        </p>
      ) : extensions.available.length === 0 ? (
        <p className="gsh-extensions__empty">{COPY.extensionsEmpty}</p>
      ) : (
        <ul className="gsh-extensions__list">
          {extensions.available.map((extension) => {
            const name = extensionName(extension);
            const checked = selectedNames.has(name);
            return (
              <li className="gsh-extension-item" key={name}>
                <label className="gsh-extension-item__select">
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={!canWrite || busy}
                    onChange={() => void (checked ? onRemove(name) : onAdd(extension))}
                  />
                  <span>
                    <span className="gsh-extension-item__name">
                      {extensionDisplayName(extension)}
                    </span>
                    <span className="gsh-extension-item__meta">
                      {extension.type}
                      {extension.description ? ` · ${extension.description}` : ''}
                    </span>
                  </span>
                </label>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
