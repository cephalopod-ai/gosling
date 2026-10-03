import { useEffect, useRef, useState } from 'react';
import type {
  AuthenticationResponse_unstable as AuthenticationResponse,
  AuthenticationTarget,
} from '@repo-makeover/gosling-sdk';
import {
  acpReadAuthentication,
  acpSetExtensionAuthentication,
  acpSetProviderAuthentication,
} from '../../acp/authentication';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { workspaceErrorMessage } from '../../utils/workspaceError';
import { Button } from '../ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog';
import { Input } from '../ui/input';
import { CredentialProfileManagerDialog } from './CredentialProfileManagerDialog';

interface AuthenticationDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  target: AuthenticationTarget;
  onChanged?(authentication: AuthenticationResponse): void;
}

export function AuthenticationDialog({
  open,
  onOpenChange,
  target,
  onChanged,
}: AuthenticationDialogProps) {
  const { credentialProfiles, refreshWorkspaces } = useWorkspace();
  const [authentication, setAuthentication] = useState<AuthenticationResponse | null>(null);
  const [profileId, setProfileId] = useState('');
  const [values, setValues] = useState<Record<string, Record<string, string>>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [managerOpen, setManagerOpen] = useState(false);
  const { id, type } = target;
  const generation = useRef(0);

  useEffect(() => {
    generation.current += 1;
    setBusy(false);
    setValues({});
    if (!open) return;
    let cancelled = false;
    setAuthentication(null);
    setValues({});
    setError(null);
    void acpReadAuthentication({ id, type })
      .then((result) => {
        if (cancelled) return;
        setAuthentication(result);
        setProfileId(result.credentialProfileId ?? '');
      })
      .catch((cause) => {
        if (!cancelled) setError(workspaceErrorMessage(cause, 'Unable to load authentication'));
      });
    return () => {
      cancelled = true;
    };
  }, [open, id, type]);

  const apply = async (action: () => Promise<AuthenticationResponse>) => {
    if (busy) return;
    const startedGeneration = generation.current;
    setBusy(true);
    setError(null);
    try {
      const result = await action();
      if (startedGeneration !== generation.current) return;
      setAuthentication(result);
      setProfileId(result.credentialProfileId ?? '');
      setValues({});
      onChanged?.(result);
      if (type === 'workspace') {
        await refreshWorkspaces();
        window.electron.broadcastWorkspaceChange();
      }
    } catch (cause) {
      if (startedGeneration !== generation.current) return;
      setError(workspaceErrorMessage(cause, 'Unable to update authentication'));
      try {
        const saved = await acpReadAuthentication(target);
        if (startedGeneration !== generation.current) return;
        setAuthentication(saved);
        onChanged?.(saved);
        if (type === 'workspace') {
          await refreshWorkspaces();
          window.electron.broadcastWorkspaceChange();
        }
      } catch {
        /* Keep the mutation error visible when the connection is unavailable. */
      }
    } finally {
      if (startedGeneration === generation.current) setBusy(false);
    }
  };

  const profiles = credentialProfiles.filter(
    (profile) =>
      profile.status === 'configured' &&
      (!authentication?.providerId || profile.providerOrServiceId === authentication.providerId)
  );

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(next) => {
          if (!busy) onOpenChange(next);
        }}
      >
        <DialogContent className="max-w-xl" aria-busy={busy}>
          <DialogHeader>
            <DialogTitle>
              {type === 'workspace' ? 'Workspace authentication' : 'Chat authentication'}
            </DialogTitle>
            <DialogDescription>
              {type === 'workspace'
                ? 'Changes apply to new chats in this workspace. Existing chats keep their authentication.'
                : 'Changes apply only to this chat.'}{' '}
              Disconnecting keeps saved accounts available for reconnection.
            </DialogDescription>
          </DialogHeader>
          <div className="max-h-[65vh] space-y-5 overflow-y-auto">
            {error && (
              <p role="alert" className="text-sm text-red-600">
                {error}
              </p>
            )}
            {!authentication && !error && <p role="status">Loading authentication…</p>}
            {authentication && (
              <>
                <section className="space-y-2" aria-label="Provider authentication">
                  <h3 className="text-sm font-semibold">Provider account</h3>
                  <p className="text-xs text-text-secondary">
                    {authentication.settings.providerDisconnected
                      ? 'Disconnected'
                      : (authentication.credentialProfileName ?? 'App default credentials')}
                    {authentication.providerId && ` · ${authentication.providerId}`}
                  </p>
                  <label className="block space-y-1 text-sm">
                    <span>Credential profile</span>
                    <select
                      className="w-full rounded-md border border-border-primary bg-background-primary px-3 py-2"
                      value={profileId}
                      onChange={(event) => setProfileId(event.target.value)}
                      disabled={busy}
                    >
                      <option value="">Choose a profile</option>
                      {profiles.map((profile) => (
                        <option key={profile.id} value={profile.id}>
                          {profile.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      disabled={busy || !profiles.some((profile) => profile.id === profileId)}
                      onClick={() =>
                        void apply(() => acpSetProviderAuthentication(target, profileId))
                      }
                    >
                      Connect profile
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={busy || authentication.settings.providerDisconnected}
                      onClick={() => void apply(() => acpSetProviderAuthentication(target, null))}
                    >
                      Disconnect provider
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={busy}
                      onClick={() => setManagerOpen(true)}
                    >
                      Manage profiles
                    </Button>
                  </div>
                </section>
                <section className="space-y-3" aria-label="Extension authentication">
                  <h3 className="text-sm font-semibold">MCP extensions</h3>
                  {authentication.extensions.length === 0 && (
                    <p className="text-xs text-text-secondary">No MCP extensions configured.</p>
                  )}
                  {authentication.extensions.map((extension) => {
                    const binding = authentication.settings.extensions?.[extension.key];
                    const fields = Object.entries(values[extension.name] ?? {})
                      .filter(([, value]) => value.length > 0)
                      .map(([key, value]) => ({ key, value }));
                    const update = (connected: boolean, signIn = false, supplyFields = false) =>
                      void apply(() =>
                        acpSetExtensionAuthentication({
                          target,
                          name: extension.name,
                          connected,
                          signIn,
                          secretFields: supplyFields ? fields : [],
                        })
                      );
                    return (
                      <div
                        key={extension.name}
                        className="space-y-2 rounded-lg border border-border-primary p-3"
                      >
                        <div className="flex items-center justify-between gap-2 text-sm">
                          <span className="font-medium">{extension.name}</span>
                          <span className="text-xs text-text-secondary">
                            {binding?.disconnected
                              ? 'Disconnected'
                              : binding?.credentialNamespace
                                ? 'Scoped account'
                                : 'App default credentials'}
                          </span>
                        </div>
                        {extension.secretFields.map((field) => (
                          <label key={field} className="block space-y-1 text-xs">
                            <span>{field}</span>
                            <Input
                              type="password"
                              autoComplete="off"
                              disabled={busy}
                              value={values[extension.name]?.[field] ?? ''}
                              placeholder={
                                binding?.secretFields?.includes(field)
                                  ? 'Saved — enter a replacement'
                                  : 'Enter credential'
                              }
                              onChange={(event) =>
                                setValues((current) => ({
                                  ...current,
                                  [extension.name]: {
                                    ...current[extension.name],
                                    [field]: event.target.value,
                                  },
                                }))
                              }
                            />
                          </label>
                        ))}
                        <div className="flex flex-wrap gap-2">
                          {fields.length > 0 && (
                            <Button
                              size="sm"
                              disabled={busy}
                              onClick={() => update(true, false, true)}
                            >
                              Save credentials
                            </Button>
                          )}
                          {extension.supportsOauth && (
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={busy}
                              onClick={() => update(true, true, true)}
                            >
                              Sign in to {extension.name}
                            </Button>
                          )}
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={busy}
                            onClick={() => update(true)}
                          >
                            Reconnect {extension.name}
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            disabled={busy || binding?.disconnected}
                            onClick={() => update(false)}
                          >
                            Disconnect {extension.name}
                          </Button>
                        </div>
                      </div>
                    );
                  })}
                </section>
              </>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" disabled={busy} onClick={() => onOpenChange(false)}>
              Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <CredentialProfileManagerDialog open={managerOpen} onOpenChange={setManagerOpen} />
    </>
  );
}
