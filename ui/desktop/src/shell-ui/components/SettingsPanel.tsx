import { useEffect, useMemo, useState } from 'react';
import type { ShellSettingsSnapshot } from '../../shell/ipc';
import type { ShellCredentialSnapshot } from '../../shell/credentialController';
import type { ShellDirectorySnapshot } from '../../shell/directoryController';
import {
  MAX_SHELL_TEXT_SCALE,
  MIN_SHELL_TEXT_SCALE,
  SHELL_THEME_VALUES,
  type ShellTheme,
} from '../../shell/settingsSchema';
import { COPY, settingsRecoveryCopy } from '../copy';
import { ShellButton, ShellButtonRow, ShellNotice } from './primitives';

export interface SettingsPanelProps {
  settings: ShellSettingsSnapshot;
  directory: ShellDirectorySnapshot;
  credentials: ShellCredentialSnapshot;
  productName: string;
  onThemeChange: (theme: ShellTheme) => void;
  onTextScaleChange: (scale: number) => void;
  onModelSelect: (providerId: string, modelId: string) => void;
  onReset: () => void;
  onClose: () => void;
}

export const SettingsPanel = ({
  settings,
  directory,
  credentials,
  productName,
  onThemeChange,
  onTextScaleChange,
  onModelSelect,
  onReset,
  onClose,
}: SettingsPanelProps) => {
  const recoveryMessage = settingsRecoveryCopy(settings.recovery, productName);
  const readOnly = recoveryMessage !== null;
  const selectedAccount = credentials.profiles.find(
    (profile) => profile.id === credentials.selectedProfileId
  );
  const modelSelection = settings.modelSelection ?? {
    status: 'unavailable' as const,
    providerId: null,
    modelId: null,
    options: [],
  };
  const [providerId, setProviderId] = useState(modelSelection.providerId ?? '');
  const providerOptions = useMemo(
    () => [
      ...new Map(modelSelection.options.map((option) => [option.providerId, option])).values(),
    ],
    [modelSelection.options]
  );
  const modelOptions = modelSelection.options.filter((option) => option.providerId === providerId);
  const [modelId, setModelId] = useState(modelSelection.modelId ?? '');

  useEffect(() => {
    const nextProvider = modelSelection.options.some(
      (option) => option.providerId === modelSelection.providerId
    )
      ? (modelSelection.providerId ?? '')
      : (modelSelection.options[0]?.providerId ?? '');
    setProviderId(nextProvider);
    const nextModel = modelSelection.options.find(
      (option) => option.providerId === nextProvider && option.modelId === modelSelection.modelId
    );
    setModelId(
      nextModel?.modelId ??
        modelSelection.options.find((option) => option.providerId === nextProvider)?.modelId ??
        ''
    );
  }, [modelSelection]);

  const selectProvider = (nextProviderId: string) => {
    setProviderId(nextProviderId);
    setModelId(
      modelSelection.options.find((option) => option.providerId === nextProviderId)?.modelId ?? ''
    );
  };

  return (
    <section className="gsh-settings" aria-label={COPY.settings}>
      {recoveryMessage ? (
        <ShellNotice tone="warn" message={recoveryMessage} live>
          <span className="gsh-hint">{settings.recovery.status}</span>
        </ShellNotice>
      ) : null}
      <div className="gsh-settings__row">
        <label className="gsh-settings__label" htmlFor="gsh-theme">
          {COPY.theme}
        </label>
        {readOnly ? (
          <span className="gsh-hint">{COPY.settingsReadOnly}</span>
        ) : (
          <select
            id="gsh-theme"
            value={settings.appearance.theme}
            onChange={(event) => onThemeChange(event.target.value as ShellTheme)}
          >
            {SHELL_THEME_VALUES.map((theme) => (
              <option key={theme} value={theme}>
                {theme}
              </option>
            ))}
          </select>
        )}
      </div>
      <div className="gsh-settings__row">
        <label className="gsh-settings__label" htmlFor="gsh-provider">
          Provider
        </label>
        {modelSelection.status === 'unavailable' ? (
          <span className="gsh-hint">
            Model choices are unavailable while Gosling is reconnecting.
          </span>
        ) : (
          <select
            id="gsh-provider"
            value={providerId}
            onChange={(event) => selectProvider(event.target.value)}
            disabled={readOnly || providerOptions.length === 0}
          >
            {providerOptions.map((option) => (
              <option key={option.providerId} value={option.providerId}>
                {option.providerName}
              </option>
            ))}
          </select>
        )}
      </div>
      <div className="gsh-settings__row">
        <label className="gsh-settings__label" htmlFor="gsh-model">
          Model
        </label>
        {modelSelection.status === 'available' ? (
          <>
            <select
              id="gsh-model"
              value={modelId}
              onChange={(event) => setModelId(event.target.value)}
              disabled={readOnly || modelOptions.length === 0}
            >
              {modelOptions.map((option) => (
                <option key={option.modelId} value={option.modelId}>
                  {option.modelName}
                </option>
              ))}
            </select>
            <ShellButton
              label="Apply model"
              onClick={() => onModelSelect(providerId, modelId)}
              disabled={readOnly || !providerId || !modelId}
            />
          </>
        ) : null}
      </div>
      <div className="gsh-settings__row">
        <label className="gsh-settings__label" htmlFor="gsh-text-scale">
          {COPY.textSize}
        </label>
        {readOnly ? (
          <span className="gsh-hint">{COPY.settingsReadOnly}</span>
        ) : (
          <>
            <input
              id="gsh-text-scale"
              type="range"
              min={MIN_SHELL_TEXT_SCALE}
              max={MAX_SHELL_TEXT_SCALE}
              step={0.1}
              value={settings.appearance.textScale}
              onChange={(event) => onTextScaleChange(Number(event.target.value))}
            />
            <span className="gsh-hint">{settings.appearance.textScale.toFixed(1)}×</span>
          </>
        )}
      </div>
      <div className="gsh-settings__row">
        <span className="gsh-settings__label">{COPY.rememberedFolder}</span>
        <span className="gsh-hint">
          {directory.path ?? '—'}
          {directory.path && !directory.remembered ? ' (not remembered)' : ''}
        </span>
      </div>
      <div className="gsh-settings__row">
        <span className="gsh-settings__label">{COPY.preferredAccount}</span>
        <span className="gsh-hint">
          {selectedAccount?.name ?? credentials.selectedProfileId ?? '—'}
        </span>
      </div>
      <ShellButtonRow>
        <ShellButton label={COPY.close} onClick={onClose} emphasis="primary" />
        <ShellButton label={COPY.resetSettings} onClick={onReset} />
      </ShellButtonRow>
    </section>
  );
};
