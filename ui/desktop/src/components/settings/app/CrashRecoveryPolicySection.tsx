import { useEffect, useRef, useState } from 'react';
import { defineMessages, useIntl } from '../../../i18n';
import type { CrashRecoveryPolicy } from '../../../utils/settings';
import { defaultSettings } from '../../../utils/settings';
import { errorMessage } from '../../../utils/conversionUtils';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';

const i18n = defineMessages({
  title: {
    id: 'settings.crashRecovery.title',
    defaultMessage: 'Crash recovery',
  },
  description: {
    id: 'settings.crashRecovery.description',
    defaultMessage:
      'Choose what Gosling does with a task that was running when Desktop stopped unexpectedly. Recovery starts a new model turn from saved history; it cannot restore the exact interrupted provider stream.',
  },
  manualLabel: {
    id: 'settings.crashRecovery.manual.label',
    defaultMessage: 'Manual',
  },
  manualDescription: {
    id: 'settings.crashRecovery.manual.description',
    defaultMessage: 'Reopen the interrupted chat and wait for you to review and resume it.',
  },
  safeLabel: {
    id: 'settings.crashRecovery.safe.label',
    defaultMessage: 'Safe (recommended)',
  },
  safeDescription: {
    id: 'settings.crashRecovery.safe.description',
    defaultMessage:
      'Automatically continue only when the backend confirms an interrupted turn and every recovered tool call has a saved successful result. Otherwise, wait for your review.',
  },
  alwaysLabel: {
    id: 'settings.crashRecovery.always.label',
    defaultMessage: 'Always',
  },
  alwaysDescription: {
    id: 'settings.crashRecovery.always.description',
    defaultMessage:
      'Automatically continue even when a tool result is missing or failed. This can repeat external side effects such as writes, messages, or deployments.',
  },
  safeguards: {
    id: 'settings.crashRecovery.safeguards',
    defaultMessage:
      'Permission prompts still apply in every mode. Closing Gosling normally clears pending recovery instead of restarting the task.',
  },
  saveError: {
    id: 'settings.crashRecovery.saveError',
    defaultMessage: 'Could not save the crash recovery policy: {error}',
  },
});

const OPTIONS: Array<{
  value: CrashRecoveryPolicy;
  label: keyof typeof i18n;
  description: keyof typeof i18n;
}> = [
  { value: 'manual', label: 'manualLabel', description: 'manualDescription' },
  { value: 'safe', label: 'safeLabel', description: 'safeDescription' },
  { value: 'always', label: 'alwaysLabel', description: 'alwaysDescription' },
];

export default function CrashRecoveryPolicySection() {
  const intl = useIntl();
  const [policy, setPolicy] = useState<CrashRecoveryPolicy>(defaultSettings.crashRecoveryPolicy);
  const [saveError, setSaveError] = useState<string | null>(null);
  const savedPolicyRef = useRef(policy);

  useEffect(() => {
    void window.electron.getSetting('crashRecoveryPolicy').then((storedPolicy) => {
      setPolicy(storedPolicy);
      savedPolicyRef.current = storedPolicy;
    });
  }, []);

  const handlePolicyChange = async (nextPolicy: CrashRecoveryPolicy) => {
    setPolicy(nextPolicy);
    try {
      await window.electron.setSetting('crashRecoveryPolicy', nextPolicy);
      savedPolicyRef.current = nextPolicy;
      setSaveError(null);
    } catch (error) {
      setPolicy(savedPolicyRef.current);
      setSaveError(
        intl.formatMessage(i18n.saveError, { error: errorMessage(error, 'Unknown error') })
      );
    }
  };

  return (
    <Card className="rounded-lg">
      <CardHeader className="pb-0">
        <CardTitle className="mb-1">{intl.formatMessage(i18n.title)}</CardTitle>
        <CardDescription>{intl.formatMessage(i18n.description)}</CardDescription>
      </CardHeader>
      <CardContent className="pt-4 space-y-2 px-4">
        {OPTIONS.map((option) => {
          const selected = policy === option.value;
          return (
            <button
              key={option.value}
              type="button"
              className={`w-full flex items-center justify-between gap-4 rounded-lg px-3 py-2 text-left transition-colors ${
                selected
                  ? 'bg-background-secondary'
                  : 'bg-background-primary hover:bg-background-secondary'
              }`}
              aria-pressed={selected}
              onClick={() => void handlePolicyChange(option.value)}
            >
              <span>
                <span className="block text-sm text-text-primary">
                  {intl.formatMessage(i18n[option.label])}
                </span>
                <span className="mt-1 block text-xs text-text-secondary">
                  {intl.formatMessage(i18n[option.description])}
                </span>
              </span>
              <span
                aria-hidden="true"
                className={`h-4 w-4 shrink-0 rounded-full border transition-all ${
                  selected
                    ? 'border-[6px] border-black bg-white dark:border-white dark:bg-black'
                    : 'border-border-primary'
                }`}
              />
            </button>
          );
        })}
        <p className="pt-1 text-xs text-text-secondary">{intl.formatMessage(i18n.safeguards)}</p>
        {saveError && (
          <p role="alert" className="text-xs text-red-500">
            {saveError}
          </p>
        )}
      </CardContent>
    </Card>
  );
}
