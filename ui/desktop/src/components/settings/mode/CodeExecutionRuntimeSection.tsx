import { useEffect, useState } from 'react';
import { defineMessages, useIntl } from '../../../i18n';
import { useConfig } from '../../ConfigContext';

const CONFIG_KEY = 'GOSLING_CODE_EXECUTION_RUNTIME';

type CodeExecutionRuntime = 'enabled' | 'disabled';

const i18n = defineMessages({
  title: {
    id: 'codeExecutionRuntime.title',
    defaultMessage: 'Code execution runtime',
  },
  description: {
    id: 'codeExecutionRuntime.description',
    defaultMessage: 'Allow or block Code Mode runtime loading for new Gosling processes.',
  },
  enabledLabel: {
    id: 'codeExecutionRuntime.enabledLabel',
    defaultMessage: 'Enabled',
  },
  enabledDescription: {
    id: 'codeExecutionRuntime.enabledDescription',
    defaultMessage: 'Allow Code Mode when the extension is enabled.',
  },
  disabledLabel: {
    id: 'codeExecutionRuntime.disabledLabel',
    defaultMessage: 'Disabled',
  },
  disabledDescription: {
    id: 'codeExecutionRuntime.disabledDescription',
    defaultMessage: 'Block Code Mode runtime loading.',
  },
  restartRequired: {
    id: 'codeExecutionRuntime.restartRequired',
    defaultMessage: 'Restart Gosling for this change to take effect.',
  },
  restartRequiredExternalBackend: {
    id: 'codeExecutionRuntime.restartRequiredExternalBackend',
    defaultMessage: 'Restart the external backend process for this change to take effect.',
  },
});

const runtimeFromConfig = (value: unknown): CodeExecutionRuntime => {
  if (value === undefined || value === null) return 'enabled';
  return value === 'enabled' ? 'enabled' : 'disabled';
};

export function CodeExecutionRuntimeSection() {
  const intl = useIntl();
  const { config, upsert } = useConfig();
  const [currentRuntime, setCurrentRuntime] = useState<CodeExecutionRuntime>('enabled');
  const [restartRequired, setRestartRequired] = useState(false);
  const [usesExternalBackend, setUsesExternalBackend] = useState(false);

  useEffect(() => {
    setCurrentRuntime(runtimeFromConfig(config[CONFIG_KEY]));
  }, [config]);

  useEffect(() => {
    const loadExternalBackendSetting = async () => {
      const externalGoslingd = await window.electron.getSetting('externalGoslingd');
      setUsesExternalBackend(Boolean(externalGoslingd?.enabled));
    };
    loadExternalBackendSetting();
  }, []);

  const handleRuntimeChange = async (runtime: CodeExecutionRuntime) => {
    await upsert(CONFIG_KEY, runtime, false);
    setCurrentRuntime(runtime);
    setRestartRequired(true);
  };

  const options: Array<{
    value: CodeExecutionRuntime;
    label: keyof typeof i18n;
    description: keyof typeof i18n;
  }> = [
    {
      value: 'enabled',
      label: 'enabledLabel',
      description: 'enabledDescription',
    },
    {
      value: 'disabled',
      label: 'disabledLabel',
      description: 'disabledDescription',
    },
  ];

  return (
    <div className="border-t border-border-subtle mt-3 pt-3 px-2">
      <div className="mb-2">
        <h3 className="text-sm text-text-primary">{intl.formatMessage(i18n.title)}</h3>
        <p className="text-sm text-text-secondary mt-[2px]">
          {intl.formatMessage(i18n.description)}
        </p>
      </div>
      <div className="space-y-1">
        {options.map((option) => {
          const checked = currentRuntime === option.value;
          return (
            <button
              key={option.value}
              type="button"
              className={`w-full flex items-center justify-between text-left text-sm py-2 px-2 rounded-lg transition-all ${
                checked
                  ? 'bg-background-secondary'
                  : 'bg-background-primary hover:bg-background-secondary'
              }`}
              aria-pressed={checked}
              onClick={() => handleRuntimeChange(option.value)}
            >
              <span>
                <span className="block text-text-primary">
                  {intl.formatMessage(i18n[option.label])}
                </span>
                <span className="block text-text-secondary mt-[2px]">
                  {intl.formatMessage(i18n[option.description])}
                </span>
              </span>
              <span
                className={`h-4 w-4 shrink-0 rounded-full border transition-all ${
                  checked
                    ? 'border-[6px] border-black bg-white dark:border-white dark:bg-black'
                    : 'border-border-primary'
                }`}
              />
            </button>
          );
        })}
      </div>
      {restartRequired && (
        <p className="text-xs text-text-secondary mt-2">
          {intl.formatMessage(
            usesExternalBackend ? i18n.restartRequiredExternalBackend : i18n.restartRequired
          )}
        </p>
      )}
    </div>
  );
}
