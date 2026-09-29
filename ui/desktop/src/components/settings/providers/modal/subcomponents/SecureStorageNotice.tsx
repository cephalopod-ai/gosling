import { Lock } from 'lucide-react';
import { defineMessages, useIntl } from '../../../../../i18n';

const i18n = defineMessages({
  defaultMessage: {
    id: 'secureStorageNotice.configuredStorage',
    defaultMessage:
      'Keys use the backend’s configured credential storage. When keyring is disabled, secrets are stored in a plaintext file.',
  },
});

export function SecureStorageNotice({
  className = '',
  message,
}: {
  className?: string;
  message?: string;
}) {
  const intl = useIntl();
  const displayMessage = message ?? intl.formatMessage(i18n.defaultMessage);
  return (
    <div className={`flex items-center mt-2 text-gray-600 dark:text-gray-300 ${className}`}>
      <Lock className="w-5 h-5" />
      <span className="text-sm font-light ml-2">{displayMessage}</span>
    </div>
  );
}
