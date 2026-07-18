import { useCallback, useEffect, useMemo, useState } from 'react';
import { KeyRound, Pencil, Plus, Trash2 } from 'lucide-react';
import type {
  CredentialAuthKind,
  CredentialProfile,
  ProviderConfigKey,
} from '@repo-makeover/gosling-sdk';
import { acpCredentialProfileUsage, acpTestCredentialProfile } from '../../acp/workspaces';
import { acpListProviderDetails } from '../../acp/providers';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import type { ProviderDetails } from '../../types/providers';
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

interface CredentialProfileManagerDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
}

interface ProfileDraft {
  id?: string;
  name: string;
  providerId: string;
  authKind: CredentialAuthKind;
  values: Record<string, string>;
  clearFields: string[];
}

const emptyDraft = (): ProfileDraft => ({
  name: '',
  providerId: '',
  authKind: 'config_fields',
  values: {},
  clearFields: [],
});

export function CredentialProfileManagerDialog({
  open,
  onOpenChange,
}: CredentialProfileManagerDialogProps) {
  const {
    credentialProfiles,
    createCredentialProfile,
    updateCredentialProfile,
    deleteCredentialProfile,
  } = useWorkspace();
  const [providers, setProviders] = useState<ProviderDetails[]>([]);
  const [draft, setDraft] = useState<ProfileDraft | null>(null);
  const [profileStatus, setProfileStatus] = useState<string | null>(null);
  const [testingProfileId, setTestingProfileId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setDraft(null);
      setError(null);
      setProfileStatus(null);
      return;
    }
    void acpListProviderDetails()
      .then(setProviders)
      .catch((cause) => setError(workspaceErrorMessage(cause, 'Unable to load providers')));
  }, [open]);

  const selectedProvider = useMemo(
    () => providers.find((provider) => provider.name === draft?.providerId),
    [draft?.providerId, providers]
  );
  const configKeys = selectedProvider?.metadata.config_keys ?? [];

  const beginEdit = useCallback((profile: CredentialProfile) => {
    setDraft({
      id: profile.id,
      name: profile.name,
      providerId: profile.providerOrServiceId,
      authKind: profile.authKind,
      values: { ...(profile.nonSecretFields ?? {}) },
      clearFields: [],
    });
    setError(null);
    setProfileStatus(null);
  }, []);

  const closeForm = useCallback(() => {
    setDraft(null);
    setError(null);
    setProfileStatus(null);
  }, []);

  const testProfile = useCallback(async (profile: CredentialProfile) => {
    setTestingProfileId(profile.id);
    setError(null);
    setProfileStatus(null);
    try {
      const result = await acpTestCredentialProfile(profile.id);
      setProfileStatus(
        result.supported
          ? `Credential test result: ${result.status.replace(/_/g, ' ')}.`
          : `Live credential testing is not supported for ${profile.providerOrServiceId}. Secure profile status: ${result.status.replace(/_/g, ' ')}.`
      );
    } catch (cause) {
      setError(workspaceErrorMessage(cause, 'Unable to test credential profile'));
    } finally {
      setTestingProfileId(null);
    }
  }, []);

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen) {
        closeForm();
      }
      onOpenChange(nextOpen);
    },
    [closeForm, onOpenChange]
  );

  const save = useCallback(async () => {
    if (!draft?.name.trim() || !draft.providerId || saving) return;
    const provider = providers.find((item) => item.name === draft.providerId);
    const keys = provider?.metadata.config_keys ?? [];
    const secretFields = keys
      .filter((key) => key.secret && draft.values[key.name])
      .map((key) => ({ key: key.name, value: draft.values[key.name] }));
    const nonSecretFields = Object.fromEntries(
      keys
        .filter((key) => !key.secret && draft.values[key.name])
        .map((key) => [key.name, draft.values[key.name]])
    );
    setSaving(true);
    setError(null);
    try {
      if (draft.id) {
        await updateCredentialProfile({
          profileId: draft.id,
          name: draft.name.trim(),
          authKind: draft.authKind,
          nonSecretFields,
          secretFields,
          clearSecretFields: draft.clearFields,
        });
      } else {
        await createCredentialProfile({
          name: draft.name.trim(),
          providerOrServiceId: draft.providerId,
          authKind: draft.authKind,
          nonSecretFields,
          secretFields,
        });
      }
      closeForm();
    } catch (cause) {
      setError(workspaceErrorMessage(cause, 'Unable to save credential profile'));
    } finally {
      setDraft((current) =>
        current ? { ...current, values: clearSecretValues(current.values, keys) } : current
      );
      setSaving(false);
    }
  }, [closeForm, createCredentialProfile, draft, providers, saving, updateCredentialProfile]);

  const remove = useCallback(
    async (profile: CredentialProfile) => {
      try {
        const usage = await acpCredentialProfileUsage(profile.id);
        const names = usage.workspaces.map((item) => item.workspaceName).join(', ');
        const response = await window.electron.showMessageBox({
          type: 'warning',
          buttons: ['Cancel', 'Delete profile'],
          defaultId: 0,
          title: 'Delete credential profile',
          message:
            usage.workspaces.length > 0
              ? `This profile is used by: ${names}. Those workspaces will require relinking.`
              : `Delete “${profile.name}”?`,
        });
        if (response.response !== 1) return;
        await deleteCredentialProfile(profile.id, usage.workspaces.length > 0);
      } catch (cause) {
        setError(workspaceErrorMessage(cause, 'Unable to delete credential profile'));
      }
    },
    [deleteCredentialProfile]
  );

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="grid max-h-[85vh] grid-rows-[auto_minmax(0,1fr)_auto] sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Credential profiles</DialogTitle>
          <DialogDescription>
            Profiles store metadata here and secret values in Gosling secure storage. Stored values
            are never displayed again.
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 overflow-y-auto pr-1">
          {draft ? (
            <div className="space-y-4">
              <Field label="Profile name">
                <Input
                  value={draft.name}
                  onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                  autoFocus
                />
              </Field>
              <Field label="Provider or service">
                <select
                  value={draft.providerId}
                  onChange={(event) =>
                    setDraft({ ...draft, providerId: event.target.value, values: {} })
                  }
                  disabled={Boolean(draft.id)}
                  className="h-9 w-full rounded-md border border-border-primary bg-background-primary px-3 text-sm"
                >
                  <option value="">Select a provider</option>
                  {providers.map((provider) => (
                    <option key={provider.name} value={provider.name}>
                      {provider.metadata.display_name || provider.name}
                    </option>
                  ))}
                </select>
              </Field>
              {configKeys.map((key) => (
                <CredentialField
                  key={key.name}
                  configKey={key}
                  profile={
                    draft.id ? credentialProfiles.find((item) => item.id === draft.id) : null
                  }
                  value={draft.values[key.name] ?? ''}
                  clear={draft.clearFields.includes(key.name)}
                  onValue={(value) =>
                    setDraft({ ...draft, values: { ...draft.values, [key.name]: value } })
                  }
                  onClear={(clear) =>
                    setDraft({
                      ...draft,
                      clearFields: clear
                        ? [...draft.clearFields, key.name]
                        : draft.clearFields.filter((item) => item !== key.name),
                    })
                  }
                />
              ))}
              {error && (
                <p role="alert" className="text-sm text-red-600">
                  {error}
                </p>
              )}
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={closeForm} disabled={saving}>
                  Cancel
                </Button>
                <Button
                  onClick={() => void save()}
                  disabled={saving || !draft.name.trim() || !draft.providerId}
                >
                  {saving ? 'Saving…' : 'Save profile'}
                </Button>
              </div>
            </div>
          ) : (
            <div className="space-y-2">
              {credentialProfiles.length === 0 ? (
                <div className="rounded-lg border border-dashed border-border-primary p-4 text-sm text-text-secondary">
                  No credential profiles yet.
                </div>
              ) : (
                credentialProfiles.map((profile) => (
                  <div
                    key={profile.id}
                    className="flex items-center gap-3 rounded-lg border border-border-primary p-3"
                  >
                    <KeyRound className="size-4 text-text-secondary" />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-medium">{profile.name}</div>
                      <div className="truncate text-xs text-text-secondary">
                        {profile.providerOrServiceId} · {profile.status.replace(/_/g, ' ')}
                      </div>
                    </div>
                    <Button
                      variant="outline"
                      size="xs"
                      onClick={() => void testProfile(profile)}
                      disabled={testingProfileId !== null}
                      aria-label={`Test ${profile.name}`}
                    >
                      {testingProfileId === profile.id ? 'Testing…' : 'Test'}
                    </Button>
                    <Button
                      variant="ghost"
                      size="xs"
                      onClick={() => beginEdit(profile)}
                      disabled={profile.source !== 'workspace_secure_storage'}
                      aria-label={`Edit ${profile.name}`}
                    >
                      <Pencil className="size-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="xs"
                      onClick={() => void remove(profile)}
                      disabled={profile.source !== 'workspace_secure_storage'}
                      aria-label={`Delete ${profile.name}`}
                    >
                      <Trash2 className="size-4" />
                    </Button>
                  </div>
                ))
              )}
              {error && (
                <p role="alert" className="text-sm text-red-600">
                  {error}
                </p>
              )}
              {profileStatus && (
                <p role="status" className="text-sm text-text-secondary">
                  {profileStatus}
                </p>
              )}
            </div>
          )}
        </div>

        <DialogFooter>
          {!draft && (
            <>
              <Button variant="outline" onClick={() => handleOpenChange(false)}>
                Close
              </Button>
              <Button onClick={() => setDraft(emptyDraft())}>
                <Plus className="mr-1 size-4" /> New profile
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1.5 text-sm font-medium">
      <span>{label}</span>
      {children}
    </label>
  );
}

function CredentialField({
  configKey,
  profile,
  value,
  clear,
  onValue,
  onClear,
}: {
  configKey: ProviderConfigKey;
  profile?: CredentialProfile | null;
  value: string;
  clear: boolean;
  onValue(value: string): void;
  onClear(clear: boolean): void;
}) {
  const configured = profile?.configuredSecretFields?.includes(configKey.name) ?? false;
  return (
    <Field label={`${configKey.name}${configKey.required ? ' (required)' : ''}`}>
      <Input
        type={configKey.secret ? 'password' : 'text'}
        value={value}
        onChange={(event) => onValue(event.target.value)}
        placeholder={configKey.secret && configured ? 'Configured — enter a replacement' : ''}
        autoComplete="off"
        disabled={clear}
      />
      {configKey.secret && configured && (
        <label className="flex items-center gap-2 text-xs font-normal text-text-secondary">
          <input
            type="checkbox"
            checked={clear}
            onChange={(event) => onClear(event.target.checked)}
          />
          Remove the stored value
        </label>
      )}
    </Field>
  );
}

function clearSecretValues(
  values: Record<string, string>,
  keys: ProviderConfigKey[]
): Record<string, string> {
  const secretNames = new Set(keys.filter((key) => key.secret).map((key) => key.name));
  return Object.fromEntries(
    Object.entries(values).map(([key, value]) => [key, secretNames.has(key) ? '' : value])
  );
}
