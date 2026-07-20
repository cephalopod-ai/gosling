import { useCallback, useEffect, useMemo, useState } from 'react';
import { FolderOpen, KeyRound, Plus, Trash2 } from 'lucide-react';
import { v7 as uuidv7 } from 'uuid';
import { toast } from 'react-toastify';
import type {
  CredentialBinding,
  ProductOutputFolder,
  ProductType,
  Workspace,
  WorkspaceFolder,
  WorkspaceMutation,
  WorkspaceThinkingEffort,
  WorkspaceValidationReport,
} from '@repo-makeover/gosling-sdk';
import { acpListProviderDetails, acpListProviderModels } from '../../acp/providers';
import { acpCreateWorkspaceOutput, workspaceToMutation } from '../../acp/workspaces';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import type { ProviderDetails } from '../../types/providers';
import { getDefaultWorkspaceWorkingDir } from '../../utils/workingDir';
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

const PRODUCT_TYPES: ProductType[] = [
  'document',
  'spreadsheet',
  'presentation',
  'image',
  'video',
  'code',
  'data',
  'export',
  'other',
];

const DEFAULT_WORKSPACE_PROVIDER = 'chatgpt_codex';
const DEFAULT_WORKSPACE_MODEL = 'gpt-5.6-terra';
const DEFAULT_WORKSPACE_EFFORT: WorkspaceThinkingEffort = 'medium';

interface WorkspaceModelOption {
  id: string;
  thinkingEfforts?: WorkspaceThinkingEffort[];
}

interface WorkspaceEditorDialogProps {
  open: boolean;
  workspace?: Workspace | null;
  onOpenChange(open: boolean): void;
}

export function WorkspaceEditorDialog({
  open,
  workspace,
  onOpenChange,
}: WorkspaceEditorDialogProps) {
  const { credentialProfiles, createWorkspace, updateWorkspace, validateWorkspace } =
    useWorkspace();
  const [draft, setDraft] = useState<WorkspaceMutation>(() => createDraft(workspace));
  const [providers, setProviders] = useState<ProviderDetails[]>([]);
  const [models, setModels] = useState<WorkspaceModelOption[]>([]);
  const [modelsProviderId, setModelsProviderId] = useState<string | null>(null);
  const [providerCatalogError, setProviderCatalogError] = useState<string | null>(null);
  const [modelCatalogError, setModelCatalogError] = useState<string | null>(null);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [validation, setValidation] = useState<WorkspaceValidationReport | null>(null);
  const [profileManagerOpen, setProfileManagerOpen] = useState(false);
  const [validating, setValidating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setDraft(createDraft(workspace));
      setValidation(null);
      setValidating(false);
      setError(null);
    }
  }, [open, workspace]);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setProviderCatalogError(null);
    void acpListProviderDetails()
      .then((items) => {
        if (cancelled) return;
        setProviders(
          [...items].sort(
            (left, right) =>
              Number(right.is_configured) - Number(left.is_configured) ||
              left.metadata.display_name.localeCompare(right.metadata.display_name)
          )
        );
      })
      .catch((cause) => {
        if (!cancelled) {
          setProviderCatalogError(workspaceErrorMessage(cause, 'Unable to load providers'));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    const providerId = draft.defaultProvider;
    if (!open || !providerId) {
      setModels([]);
      setModelsProviderId(null);
      setModelsLoading(false);
      setModelCatalogError(null);
      return;
    }
    let cancelled = false;
    setModels([]);
    setModelsProviderId(null);
    setModelsLoading(true);
    setModelCatalogError(null);
    void acpListProviderModels(providerId)
      .then((items) => {
        if (!cancelled) {
          setModels(
            items.map((item) => ({
              id: item.id,
              thinkingEfforts: item.thinkingEfforts,
            }))
          );
          setModelsProviderId(providerId);
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setModels([]);
          setModelCatalogError(workspaceErrorMessage(cause, 'Unable to load provider models'));
        }
      })
      .finally(() => {
        if (!cancelled) setModelsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [draft.defaultProvider, open]);

  const modelsMatchSelectedProvider =
    Boolean(draft.defaultProvider) && modelsProviderId === draft.defaultProvider;

  const effortOptions = useMemo(() => {
    if (!modelsMatchSelectedProvider || !draft.defaultModel) return [];
    return models.find((model) => model.id === draft.defaultModel)?.thinkingEfforts ?? [];
  }, [draft.defaultModel, models, modelsMatchSelectedProvider]);

  useEffect(() => {
    if (
      modelsLoading ||
      !modelsMatchSelectedProvider ||
      !draft.defaultProvider ||
      !draft.defaultModel ||
      models.length === 0
    ) {
      return;
    }
    setDraft((current) => {
      const selected = models.find((model) => model.id === current.defaultModel);
      if (!selected) return current;
      const supported = selected.thinkingEfforts ?? [];
      const nextEffort =
        supported.length === 0
          ? null
          : current.defaultThinkingEffort && supported.includes(current.defaultThinkingEffort)
            ? current.defaultThinkingEffort
            : supported.includes(DEFAULT_WORKSPACE_EFFORT)
              ? DEFAULT_WORKSPACE_EFFORT
              : supported[0];
      return nextEffort === current.defaultThinkingEffort
        ? current
        : { ...current, defaultThinkingEffort: nextEffort };
    });
  }, [
    draft.defaultModel,
    draft.defaultProvider,
    models,
    modelsLoading,
    modelsMatchSelectedProvider,
  ]);

  const requiredIssue = useMemo(
    () => validation?.issues?.find((issue) => issue.severity === 'error'),
    [validation]
  );

  const chooseDirectory = useCallback(async (onPath: (path: string) => void) => {
    try {
      const result = await window.electron.directoryChooser();
      const path = result.canceled ? null : result.filePaths[0];
      if (path) {
        await window.electron.addRecentDir(path);
        onPath(path);
      }
    } catch (cause) {
      toast.error(workspaceErrorMessage(cause, 'Unable to choose a folder'));
    }
  }, []);

  const reveal = useCallback(async (path: string) => {
    try {
      await window.electron.addRecentDir(path);
      const opened = await window.electron.openDirectoryInExplorer(path);
      if (!opened) throw new Error('The folder is unavailable. Choose a replacement path.');
    } catch (cause) {
      toast.error(workspaceErrorMessage(cause, 'Unable to reveal the folder'));
    }
  }, []);

  const updateWorkingFolder = useCallback((workingFolder: string) => {
    setDraft((current) => {
      const derivedOutputPath = joinPath(current.workingFolder, 'Outputs');
      return {
        ...current,
        workingFolder,
        productOutputFolders: current.productOutputFolders.map((output) =>
          output.path === derivedOutputPath
            ? { ...output, path: joinPath(workingFolder, 'Outputs') }
            : output
        ),
      };
    });
  }, []);

  const runValidation = useCallback(async (): Promise<WorkspaceValidationReport | null> => {
    if (!draft.name.trim() || validating) return null;
    setValidating(true);
    setError(null);
    try {
      const report = await validateWorkspace(draft, workspace?.id);
      setValidation(report);
      return report;
    } catch (cause) {
      setError(workspaceErrorMessage(cause, 'Unable to validate workspace'));
      return null;
    } finally {
      setValidating(false);
    }
  }, [draft, validateWorkspace, validating, workspace?.id]);

  const save = useCallback(async () => {
    if (!draft.name.trim() || saving || validating) return;
    setSaving(true);
    setError(null);
    try {
      const report = await runValidation();
      if (!report) return;
      if (workspace) {
        await updateWorkspace(workspace.id, draft);
      } else {
        await createWorkspace(draft);
      }
      if (report.validForSession) {
        toast.success(workspace ? 'Workspace saved.' : 'Workspace created.');
      } else {
        const warnings = (report.issues ?? []).filter((issue) => issue.severity === 'warning');
        const errors = (report.issues ?? []).filter((issue) => issue.severity === 'error');
        const issuesLabel = errors.length > 0 ? 'issues' : 'warnings';
        const detailLabel = errors.length > 0 ? 'issues' : 'them';

        toast.warning(
          <div className="space-y-1">
            <p>Workspace saved with {issuesLabel}. Resolve {detailLabel} before starting a new chat.</p>
            {warnings.length > 0 ? (
              <>
                <p className="font-medium">Warnings:</p>
                <ul className="list-disc space-y-1 pl-4">
                  {warnings.map((issue, index) => (
                    <li
                      key={`${issue.code}-${issue.targetId ?? issue.path ?? 'warning'}-${index}`}
                    >
                      {issue.message}
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
            {errors.length > 0 ? (
              <>
                <p className="font-medium">Errors:</p>
                <ul className="list-disc space-y-1 pl-4">
                  {errors.map((issue, index) => (
                    <li key={`${issue.code}-${issue.targetId ?? issue.path ?? 'error'}-${index}`}>
                      {issue.message}
                    </li>
                  ))}
                </ul>
              </>
            ) : null}
          </div>
        );
      }
      onOpenChange(false);
    } catch (cause) {
      setError(workspaceErrorMessage(cause, 'Unable to save workspace'));
    } finally {
      setSaving(false);
    }
  }, [
    createWorkspace,
    draft,
    onOpenChange,
    runValidation,
    saving,
    updateWorkspace,
    validating,
    workspace,
  ]);

  const updateFolder = useCallback((id: string, patch: Partial<WorkspaceFolder>) => {
    setDraft((current) => ({
      ...current,
      folders: (current.folders ?? []).map((folder) =>
        folder.id === id ? { ...folder, ...patch } : folder
      ),
    }));
  }, []);

  const updateOutput = useCallback((id: string, patch: Partial<ProductOutputFolder>) => {
    setDraft((current) => ({
      ...current,
      productOutputFolders: current.productOutputFolders.map((output) =>
        output.id === id ? { ...output, ...patch } : output
      ),
    }));
  }, []);

  const setDefaultOutput = useCallback((id: string) => {
    setDraft((current) => ({
      ...current,
      productOutputFolders: current.productOutputFolders.map((output) => ({
        ...output,
        isDefault: output.id === id,
      })),
    }));
  }, []);

  const removeOutput = useCallback((id: string) => {
    setDraft((current) => {
      const removed = current.productOutputFolders.find((output) => output.id === id);
      const remaining = current.productOutputFolders.filter((output) => output.id !== id);
      if (removed?.isDefault && remaining[0]) {
        remaining[0] = { ...remaining[0], isDefault: true };
      }
      return { ...current, productOutputFolders: remaining };
    });
  }, []);

  const updateBinding = useCallback((id: string, patch: Partial<CredentialBinding>) => {
    setDraft((current) => ({
      ...current,
      credentialBindings: (current.credentialBindings ?? []).map((binding) =>
        binding.id === id ? { ...binding, ...patch } : binding
      ),
    }));
  }, []);

  const addFolder = useCallback(
    (kind: WorkspaceFolder['kind']) => {
      void chooseDirectory((path) => {
        setDraft((current) => ({
          ...current,
          folders: [
            ...(current.folders ?? []),
            {
              id: uuidv7(),
              label: kind === 'working' ? 'Working folder' : 'Reference',
              path,
              kind,
              access: kind === 'working' ? 'read_write' : 'read',
            },
          ],
        }));
      });
    },
    [chooseDirectory]
  );

  const setDefaultBinding = useCallback((id: string) => {
    setDraft((current) => ({
      ...current,
      defaultCredentialBindingId: id,
      credentialBindings: (current.credentialBindings ?? []).map((binding) => ({
        ...binding,
        isDefault: binding.id === id,
      })),
    }));
  }, []);

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="grid max-h-[90vh] grid-rows-[auto_minmax(0,1fr)_auto] sm:max-w-5xl">
          <DialogHeader>
            <DialogTitle>{workspace ? 'Edit workspace' : 'Create workspace'}</DialogTitle>
            <DialogDescription>
              Define repeatable folders, output destinations, and secure credential-profile
              references for new chats.
            </DialogDescription>
          </DialogHeader>

          <div className="min-h-0 space-y-6 overflow-y-auto pr-2">
            <Section title="General">
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="Name">
                  <Input
                    value={draft.name}
                    onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                    maxLength={100}
                    autoFocus
                  />
                </Field>
                <Field label="Icon label (optional)">
                  <Input
                    value={draft.icon ?? ''}
                    onChange={(event) => setDraft({ ...draft, icon: event.target.value || null })}
                    placeholder="project"
                  />
                </Field>
              </div>
              <Field label="Description (optional)">
                <textarea
                  value={draft.description ?? ''}
                  onChange={(event) =>
                    setDraft({ ...draft, description: event.target.value || null })
                  }
                  className="min-h-20 w-full rounded-md border border-border-primary bg-background-primary p-3 text-sm outline-none focus:border-border-secondary"
                />
              </Field>
              <div className="grid gap-3 md:grid-cols-2">
                <Field label="Default provider (optional)">
                  <select
                    aria-label="Default provider (optional)"
                    value={draft.defaultProvider ?? ''}
                    onChange={(event) => {
                      setModels([]);
                      setModelsProviderId(null);
                      setModelsLoading(Boolean(event.target.value));
                      setModelCatalogError(null);
                      setDraft((current) => {
                        const providerId = event.target.value || null;
                        const selected = providers.find((item) => item.name === providerId);
                        return {
                          ...current,
                          defaultProvider: providerId,
                          defaultModel: providerId
                            ? (selected?.metadata.default_model ?? null)
                            : null,
                          defaultThinkingEffort: providerId ? DEFAULT_WORKSPACE_EFFORT : null,
                        };
                      });
                    }}
                    className="h-9 rounded-md border border-border-primary bg-background-primary px-3 text-sm"
                  >
                    <option value="">Use app default</option>
                    {draft.defaultProvider &&
                      !providers.some((provider) => provider.name === draft.defaultProvider) && (
                        <option value={draft.defaultProvider}>{draft.defaultProvider}</option>
                      )}
                    {providers.map((provider) => (
                      <option key={provider.name} value={provider.name}>
                        {provider.metadata.display_name}
                        {provider.is_configured ? '' : ' — setup required'}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label="Default model (optional)">
                  <select
                    aria-label="Default model (optional)"
                    value={draft.defaultModel ?? ''}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        defaultModel: event.target.value || null,
                        defaultThinkingEffort: event.target.value ? DEFAULT_WORKSPACE_EFFORT : null,
                      }))
                    }
                    disabled={
                      !draft.defaultProvider || modelsLoading || !modelsMatchSelectedProvider
                    }
                    className="h-9 rounded-md border border-border-primary bg-background-primary px-3 text-sm disabled:opacity-60"
                  >
                    <option value="">
                      {modelsLoading ? 'Loading models…' : 'Use provider default'}
                    </option>
                    {modelsMatchSelectedProvider &&
                      draft.defaultModel &&
                      !models.some((model) => model.id === draft.defaultModel) && (
                        <option value={draft.defaultModel}>{draft.defaultModel}</option>
                      )}
                    {models.map((model) => (
                      <option key={model.id} value={model.id}>
                        {model.id}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
              <Field label="Default reasoning effort (optional)">
                <select
                  aria-label="Default reasoning effort (optional)"
                  value={draft.defaultThinkingEffort ?? ''}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      defaultThinkingEffort:
                        (event.target.value as WorkspaceThinkingEffort) || null,
                    }))
                  }
                  disabled={
                    !draft.defaultProvider ||
                    !draft.defaultModel ||
                    modelsLoading ||
                    effortOptions.length === 0
                  }
                  className="h-9 w-full rounded-md border border-border-primary bg-background-primary px-3 text-sm disabled:opacity-60"
                >
                  <option value="">
                    {effortOptions.length === 0
                      ? 'Not available for this model'
                      : 'Use app default'}
                  </option>
                  {effortOptions.map((effort) => (
                    <option key={effort} value={effort}>
                      {formatEffort(effort)}
                    </option>
                  ))}
                </select>
              </Field>
              {providerCatalogError && (
                <p role="status" className="text-xs text-amber-600">
                  {providerCatalogError}
                </p>
              )}
              {modelCatalogError && (
                <p role="status" className="text-xs text-amber-600">
                  {modelCatalogError}
                </p>
              )}
            </Section>

            <Section title="Credentials">
              <div className="flex justify-between gap-3">
                <p className="text-sm text-text-secondary">
                  Bind secure profiles by reference. Secret values are never stored in this
                  workspace.
                </p>
                <Button variant="outline" size="sm" onClick={() => setProfileManagerOpen(true)}>
                  <KeyRound className="mr-1 size-4" /> Manage profiles
                </Button>
              </div>
              {(draft.credentialBindings ?? []).map((binding) => {
                const profile = credentialProfiles.find(
                  (item) => item.id === binding.credentialProfileId
                );
                return (
                  <div
                    key={binding.id}
                    className="grid gap-2 rounded-lg border border-border-primary p-3 md:grid-cols-[1fr_1fr_auto_auto]"
                  >
                    <Input
                      value={binding.label}
                      onChange={(event) => updateBinding(binding.id, { label: event.target.value })}
                      aria-label="Credential binding label"
                    />
                    <select
                      value={binding.credentialProfileId}
                      onChange={(event) => {
                        const selected = credentialProfiles.find(
                          (item) => item.id === event.target.value
                        );
                        updateBinding(binding.id, {
                          credentialProfileId: event.target.value,
                          targetId: selected?.providerOrServiceId ?? '',
                        });
                      }}
                      className="h-9 rounded-md border border-border-primary bg-background-primary px-3 text-sm"
                      aria-label="Credential profile"
                    >
                      <option value="">Select a profile</option>
                      {credentialProfiles.map((item) => (
                        <option key={item.id} value={item.id}>
                          {item.name} ({item.status.replace(/_/g, ' ')})
                        </option>
                      ))}
                    </select>
                    <label className="flex items-center gap-2 whitespace-nowrap text-xs">
                      <input
                        type="radio"
                        name="default-credential"
                        checked={draft.defaultCredentialBindingId === binding.id}
                        onChange={() => setDefaultBinding(binding.id)}
                      />
                      Default
                    </label>
                    <Button
                      variant="ghost"
                      size="xs"
                      aria-label={`Remove ${binding.label || 'credential binding'}`}
                      onClick={() =>
                        setDraft((current) => ({
                          ...current,
                          credentialBindings: (current.credentialBindings ?? []).filter(
                            (item) => item.id !== binding.id
                          ),
                          defaultCredentialBindingId:
                            current.defaultCredentialBindingId === binding.id
                              ? null
                              : current.defaultCredentialBindingId,
                        }))
                      }
                    >
                      <Trash2 className="size-4" />
                    </Button>
                    {!profile && binding.credentialProfileId && (
                      <p className="text-xs text-amber-600 md:col-span-4">
                        This credential profile is missing and must be relinked.
                      </p>
                    )}
                  </div>
                );
              })}
              <Button
                variant="outline"
                size="sm"
                disabled={credentialProfiles.length === 0}
                onClick={() => {
                  const profile = credentialProfiles[0];
                  if (!profile) return;
                  const id = uuidv7();
                  setDraft((current) => ({
                    ...current,
                    credentialBindings: [
                      ...(current.credentialBindings ?? []),
                      {
                        id,
                        label: profile.name,
                        credentialProfileId: profile.id,
                        targetKind: 'provider',
                        targetId: profile.providerOrServiceId,
                        isDefault: (current.credentialBindings ?? []).length === 0,
                      },
                    ],
                    defaultCredentialBindingId: current.defaultCredentialBindingId ?? id,
                  }));
                }}
              >
                <Plus className="mr-1 size-4" /> Add credential binding
              </Button>
            </Section>

            <Section title="Folders">
              <PathField
                label="Primary working folder"
                path={draft.workingFolder}
                onPath={updateWorkingFolder}
                onChoose={() => void chooseDirectory(updateWorkingFolder)}
                onReveal={() => void reveal(draft.workingFolder)}
              />
              {(draft.folders ?? []).map((folder) => (
                <div
                  key={folder.id}
                  className="space-y-2 rounded-lg border border-border-primary p-3"
                >
                  <div className="grid gap-2 md:grid-cols-[1fr_auto_auto_auto]">
                    <Input
                      value={folder.label}
                      onChange={(event) => updateFolder(folder.id, { label: event.target.value })}
                      aria-label="Folder label"
                    />
                    <select
                      value={folder.kind}
                      onChange={(event) =>
                        updateFolder(folder.id, {
                          kind: event.target.value as WorkspaceFolder['kind'],
                        })
                      }
                      className="h-9 rounded-md border border-border-primary bg-background-primary px-2 text-sm"
                      aria-label="Folder kind"
                    >
                      <option value="source">Source</option>
                      <option value="reference">Reference</option>
                      <option value="working">Working</option>
                    </select>
                    <select
                      value={folder.access}
                      onChange={(event) =>
                        updateFolder(folder.id, {
                          access: event.target.value as WorkspaceFolder['access'],
                        })
                      }
                      className="h-9 rounded-md border border-border-primary bg-background-primary px-2 text-sm"
                      aria-label="Folder access"
                    >
                      <option value="read">Read only</option>
                      <option value="read_write">Read/write</option>
                    </select>
                    <Button
                      variant="ghost"
                      size="xs"
                      onClick={() =>
                        setDraft((current) => ({
                          ...current,
                          folders: (current.folders ?? []).filter((item) => item.id !== folder.id),
                        }))
                      }
                      aria-label={`Remove ${folder.label || 'folder'}`}
                    >
                      <Trash2 className="size-4" />
                    </Button>
                  </div>
                  <PathField
                    label="Folder path"
                    path={folder.path}
                    onPath={(path) => updateFolder(folder.id, { path })}
                    onChoose={() =>
                      void chooseDirectory((path) => updateFolder(folder.id, { path }))
                    }
                    onReveal={() => void reveal(folder.path)}
                  />
                </div>
              ))}
              <div className="flex flex-wrap gap-2">
                <Button variant="outline" size="sm" onClick={() => addFolder('working')}>
                  <Plus className="mr-1 size-4" /> Add working folder
                </Button>
                <Button variant="outline" size="sm" onClick={() => addFolder('reference')}>
                  <Plus className="mr-1 size-4" /> Add source/reference folder
                </Button>
              </div>
            </Section>

            <Section title="Product outputs">
              {draft.productOutputFolders.map((output) => (
                <div
                  key={output.id}
                  className="space-y-3 rounded-lg border border-border-primary p-3"
                >
                  <div className="grid gap-2 md:grid-cols-[1fr_auto_auto]">
                    <Input
                      value={output.label}
                      onChange={(event) => updateOutput(output.id, { label: event.target.value })}
                      aria-label="Output label"
                    />
                    <label className="flex items-center gap-2 whitespace-nowrap text-xs">
                      <input
                        type="radio"
                        name="default-output"
                        checked={output.isDefault}
                        onChange={() => setDefaultOutput(output.id)}
                      />
                      Default output
                    </label>
                    <Button
                      variant="ghost"
                      size="xs"
                      disabled={draft.productOutputFolders.length === 1}
                      onClick={() => removeOutput(output.id)}
                      aria-label={`Remove ${output.label || 'output folder'}`}
                    >
                      <Trash2 className="size-4" />
                    </Button>
                  </div>
                  <PathField
                    label="Output path"
                    path={output.path}
                    onPath={(path) => updateOutput(output.id, { path })}
                    onChoose={() =>
                      void chooseDirectory((path) => updateOutput(output.id, { path }))
                    }
                    onReveal={() => void reveal(output.path)}
                  />
                  <div className="flex flex-wrap gap-x-4 gap-y-2">
                    {PRODUCT_TYPES.map((productType) => (
                      <label key={productType} className="flex items-center gap-1.5 text-xs">
                        <input
                          type="checkbox"
                          checked={output.productTypes.includes(productType)}
                          onChange={(event) =>
                            updateOutput(output.id, {
                              productTypes: event.target.checked
                                ? [...output.productTypes, productType]
                                : output.productTypes.filter((item) => item !== productType),
                            })
                          }
                        />
                        {productType}
                      </label>
                    ))}
                  </div>
                  <div className="flex flex-wrap items-center gap-3">
                    <label className="flex items-center gap-2 text-xs">
                      <input
                        type="checkbox"
                        checked={output.createIfMissing}
                        onChange={(event) =>
                          updateOutput(output.id, { createIfMissing: event.target.checked })
                        }
                      />
                      Allow explicit creation if missing
                    </label>
                    {workspace && output.createIfMissing && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={async () => {
                          const response = await window.electron.showMessageBox({
                            type: 'question',
                            buttons: ['Cancel', 'Create folder'],
                            defaultId: 0,
                            message: `Create ${output.path}?`,
                          });
                          if (response.response === 1) {
                            try {
                              const result = await acpCreateWorkspaceOutput(
                                workspace.id,
                                output.id
                              );
                              setValidation(result.validation);
                              toast.success(`Created “${output.label}”.`);
                            } catch (cause) {
                              toast.error(
                                workspaceErrorMessage(cause, 'Unable to create the output folder')
                              );
                            }
                          }
                        }}
                      >
                        Create now
                      </Button>
                    )}
                  </div>
                </div>
              ))}
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  setDraft((current) => ({
                    ...current,
                    productOutputFolders: [
                      ...current.productOutputFolders,
                      {
                        id: uuidv7(),
                        label: 'Output',
                        path: joinPath(current.workingFolder, 'Outputs'),
                        productTypes: ['other'],
                        isDefault: false,
                        createIfMissing: true,
                      },
                    ],
                  }))
                }
              >
                <Plus className="mr-1 size-4" /> Add output destination
              </Button>
            </Section>

            {validation && (
              <div
                role={requiredIssue ? 'alert' : 'status'}
                className={`rounded-lg border p-3 text-sm ${
                  requiredIssue
                    ? 'border-red-500/40 bg-red-500/5 text-red-600'
                    : 'border-amber-500/40 bg-amber-500/5 text-amber-700'
                }`}
              >
                {(validation.issues ?? []).length === 0
                  ? 'Workspace validation passed.'
                  : (validation.issues ?? []).map((issue) => (
                      <div key={`${issue.code}-${issue.targetId ?? issue.path}`}>
                        {issue.message}
                      </div>
                    ))}
              </div>
            )}
            {error && (
              <p role="alert" className="text-sm text-red-600">
                {error}
              </p>
            )}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => void runValidation()}
              disabled={saving || validating || !draft.name.trim()}
            >
              {validating ? 'Validating…' : 'Validate'}
            </Button>
            <Button
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={saving || validating}
            >
              Cancel
            </Button>
            <Button
              onClick={() => void save()}
              disabled={saving || validating || !draft.name.trim()}
            >
              {saving ? 'Saving…' : 'Save workspace'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <CredentialProfileManagerDialog
        open={profileManagerOpen}
        onOpenChange={setProfileManagerOpen}
      />
    </>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-3">
      <h3 className="border-b border-border-secondary pb-2 text-sm font-semibold">{title}</h3>
      {children}
    </section>
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

function PathField({
  label,
  path,
  onPath,
  onChoose,
  onReveal,
}: {
  label: string;
  path: string;
  onPath(path: string): void;
  onChoose(): void;
  onReveal(): void;
}) {
  return (
    <Field label={label}>
      <div className="flex gap-2">
        <Input value={path} onChange={(event) => onPath(event.target.value)} />
        <Button variant="outline" size="sm" onClick={onChoose} aria-label={`Choose ${label}`}>
          Choose
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={onReveal}
          disabled={!path}
          aria-label={`Reveal ${label}`}
        >
          <FolderOpen className="size-4" />
        </Button>
      </div>
    </Field>
  );
}

function createDraft(workspace?: Workspace | null): WorkspaceMutation {
  if (workspace) return workspaceToMutation(workspace);
  const workingFolder = getDefaultWorkspaceWorkingDir();
  return {
    name: '',
    description: null,
    icon: null,
    workingFolder,
    folders: [],
    productOutputFolders: [
      {
        id: uuidv7(),
        label: 'Outputs',
        path: joinPath(workingFolder, 'Outputs'),
        productTypes: [...PRODUCT_TYPES],
        isDefault: true,
        createIfMissing: true,
      },
    ],
    credentialBindings: [],
    defaultCredentialBindingId: null,
    defaultProvider: DEFAULT_WORKSPACE_PROVIDER,
    defaultModel: DEFAULT_WORKSPACE_MODEL,
    defaultThinkingEffort: DEFAULT_WORKSPACE_EFFORT,
  };
}

function formatEffort(effort: WorkspaceThinkingEffort): string {
  if (effort === 'off') return 'Off';
  return effort.charAt(0).toUpperCase() + effort.slice(1);
}

function joinPath(root: string, child: string): string {
  const separator = root.includes('\\') ? '\\' : '/';
  return `${root.replace(/[\\/]$/, '')}${separator}${child}`;
}
