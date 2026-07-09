import { useState, useEffect } from 'react';
import { Button } from './ui/button';
import type { Permission } from '../types/permissions';
import { resolveAcpPermissionRequest } from '../acp/permissionRequests';
import { listTools, setToolPermissions } from '../acp/permissions';
import { defineMessages, useIntl } from '../i18n';

const i18n = defineMessages({
  allowOnce: {
    id: 'toolApprovalButtons.allowOnce',
    defaultMessage: 'Allow Once',
  },
  alwaysAllow: {
    id: 'toolApprovalButtons.alwaysAllow',
    defaultMessage: 'Always Allow',
  },
  alwaysAllowExtension: {
    id: 'toolApprovalButtons.alwaysAllowExtension',
    defaultMessage: 'Always Allow all {extensionName} tools',
  },
  deny: {
    id: 'toolApprovalButtons.deny',
    defaultMessage: 'Deny',
  },
  allowedOnce: {
    id: 'toolApprovalButtons.allowedOnce',
    defaultMessage: 'Allowed once',
  },
  alwaysAllowed: {
    id: 'toolApprovalButtons.alwaysAllowed',
    defaultMessage: 'Always allowed',
  },
  alwaysAllowedExtension: {
    id: 'toolApprovalButtons.alwaysAllowedExtension',
    defaultMessage: 'Always allowed ({extensionName} tools)',
  },
  denied: {
    id: 'toolApprovalButtons.denied',
    defaultMessage: 'Denied',
  },
  deniedOnce: {
    id: 'toolApprovalButtons.deniedOnce',
    defaultMessage: 'Denied once',
  },
  cancelled: {
    id: 'toolApprovalButtons.cancelled',
    defaultMessage: 'Cancelled',
  },
  staleApprovalRequest: {
    id: 'toolApprovalButtons.staleApprovalRequest',
    defaultMessage: 'This approval request is no longer active.',
  },
  failedToAllowExtension: {
    id: 'toolApprovalButtons.failedToAllowExtension',
    defaultMessage: 'Failed to update permissions for this extension',
  },
});

function extensionNameFromToolName(toolName: string): string | undefined {
  const [extensionName, ...rest] = toolName.split('__');
  return rest.length > 0 && extensionName ? extensionName : undefined;
}

const globalApprovalState = new Map<
  string,
  {
    decision: Permission | null;
    isClicked: boolean;
  }
>();

// The map outlives sessions so decisions survive remounts, but agents issue
// many approvals per session — without a cap it grows for the window's
// lifetime. Oldest entries belong to long-resolved requests, so evict those.
const MAX_APPROVAL_STATES = 500;

function recordApprovalState(
  id: string,
  state: { decision: Permission | null; isClicked: boolean }
) {
  if (!globalApprovalState.has(id) && globalApprovalState.size >= MAX_APPROVAL_STATES) {
    const oldest = globalApprovalState.keys().next().value;
    if (oldest !== undefined) {
      globalApprovalState.delete(oldest);
    }
  }
  globalApprovalState.set(id, state);
}

export interface ToolApprovalData {
  id: string;
  toolName: string;
  prompt?: string;
  sessionId: string;
  isClicked?: boolean;
}

export default function ToolApprovalButtons({ data }: { data: ToolApprovalData }) {
  const intl = useIntl();
  const { id, toolName, prompt, sessionId, isClicked: initialIsClicked } = data;

  const storedState = globalApprovalState.get(id);
  const [decision, setDecision] = useState<Permission | null>(storedState?.decision ?? null);
  const [isClicked, setIsClicked] = useState(storedState?.isClicked ?? initialIsClicked ?? false);
  const [approvalError, setApprovalError] = useState<string | null>(null);
  const [isAllowingExtension, setIsAllowingExtension] = useState(false);
  const [bulkAllowedExtension, setBulkAllowedExtension] = useState<string | null>(null);

  const extensionName = extensionNameFromToolName(toolName);

  const setResolvedDecision = (action: Permission) => {
    setDecision(action);
    setIsClicked(true);
    setApprovalError(null);
  };

  useEffect(() => {
    const currentState = globalApprovalState.get(id);
    if (currentState) {
      setDecision(currentState.decision);
      setIsClicked(currentState.isClicked);
    }
    setApprovalError(null);
  }, [id]);

  useEffect(() => {
    recordApprovalState(id, { decision, isClicked });
  }, [id, decision, isClicked]);

  const handleAction = async (action: Permission) => {
    try {
      if (resolveAcpPermissionRequest(sessionId, id, action)) {
        setResolvedDecision(action);
      } else {
        setApprovalError(intl.formatMessage(i18n.staleApprovalRequest));
      }
    } catch (err) {
      console.error('Error confirming tool action:', err);
    }
  };

  const handleAlwaysAllowExtension = async () => {
    if (!extensionName) {
      await handleAction('always_allow');
      return;
    }

    setIsAllowingExtension(true);
    try {
      const tools = await listTools(sessionId, extensionName);
      const toolPermissions = (tools.length > 0 ? tools.map((t) => t.name) : [toolName]).map(
        (name) => ({ toolName: name, permission: 'always_allow' as const })
      );
      await setToolPermissions(toolPermissions);

      if (resolveAcpPermissionRequest(sessionId, id, 'always_allow')) {
        setBulkAllowedExtension(extensionName);
        setResolvedDecision('always_allow');
      } else {
        setApprovalError(intl.formatMessage(i18n.staleApprovalRequest));
      }
    } catch (err) {
      console.error('Error allowing extension tools:', err);
      setApprovalError(intl.formatMessage(i18n.failedToAllowExtension));
    } finally {
      setIsAllowingExtension(false);
    }
  };

  if (isClicked && decision) {
    const statusMessages: Record<Permission, string> = {
      allow_once: intl.formatMessage(i18n.allowedOnce),
      always_allow:
        bulkAllowedExtension && decision === 'always_allow'
          ? intl.formatMessage(i18n.alwaysAllowedExtension, {
              extensionName: bulkAllowedExtension,
            })
          : intl.formatMessage(i18n.alwaysAllowed),
      always_deny: intl.formatMessage(i18n.denied),
      deny_once: intl.formatMessage(i18n.deniedOnce),
      cancel: intl.formatMessage(i18n.cancelled),
    };
    return (
      <p className="text-sm text-muted-foreground mt-2">
        {toolName} - {statusMessages[decision]}
      </p>
    );
  }

  return (
    <>
      <div className="flex items-center gap-2 mt-2">
        <Button
          className="rounded-full"
          variant="secondary"
          onClick={() => handleAction('allow_once')}
        >
          {intl.formatMessage(i18n.allowOnce)}
        </Button>
        {!prompt && (
          <Button
            className="rounded-full"
            variant="secondary"
            onClick={() => handleAction('always_allow')}
          >
            {intl.formatMessage(i18n.alwaysAllow)}
          </Button>
        )}
        {!prompt && extensionName && (
          <Button
            className="rounded-full"
            variant="secondary"
            disabled={isAllowingExtension}
            onClick={() => void handleAlwaysAllowExtension()}
          >
            {intl.formatMessage(i18n.alwaysAllowExtension, { extensionName })}
          </Button>
        )}
        <Button className="rounded-full" variant="outline" onClick={() => handleAction('deny_once')}>
          {intl.formatMessage(i18n.deny)}
        </Button>
      </div>
      {approvalError && (
        <p className="text-sm text-red-500 mt-2" role="alert">
          {approvalError}
        </p>
      )}
    </>
  );
}
