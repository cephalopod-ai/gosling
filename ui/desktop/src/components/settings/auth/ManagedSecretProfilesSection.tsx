import { useEffect, useMemo, useRef, useState } from 'react';
import { Eye, EyeOff, Globe, KeyRound, Plus, Server, ShieldEllipsis, Trash2 } from 'lucide-react';
import { toast } from 'react-toastify';
import { defineMessages, useIntl } from '../../../i18n';
import { Select } from '../../ui/Select';
import type {
  ManagedSecretEntry,
  ManagedSecretProfile,
  ManagedSecretProfileUse,
} from '../../../utils/settings';
import { errorMessage } from '../../../utils/conversionUtils';
import { Button } from '../../ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import { Input } from '../../ui/input';

const i18n = defineMessages({
  title: {
    id: 'managedSecrets.title',
    defaultMessage: 'Local Secret Profiles',
  },
  description: {
    id: 'managedSecrets.description',
    defaultMessage:
      'Store repo-style secrets locally for websites and projects you manage. These values stay on this device in gosling settings and are not synced.',
  },
  customTemplate: {
    id: 'managedSecrets.customTemplate',
    defaultMessage: 'Custom profile',
  },
  vpsTemplate: {
    id: 'managedSecrets.vpsTemplate',
    defaultMessage: 'VPS server',
  },
  supabaseTemplate: {
    id: 'managedSecrets.supabaseTemplate',
    defaultMessage: 'Supabase project',
  },
  addProfile: {
    id: 'managedSecrets.addProfile',
    defaultMessage: 'Add profile',
  },
  profiles: {
    id: 'managedSecrets.profiles',
    defaultMessage: 'Profiles',
  },
  profileName: {
    id: 'managedSecrets.profileName',
    defaultMessage: 'Profile name',
  },
  website: {
    id: 'managedSecrets.website',
    defaultMessage: 'Website or service',
  },
  useFor: {
    id: 'managedSecrets.useFor',
    defaultMessage: 'Use for',
  },
  useForAuthentication: {
    id: 'managedSecrets.useForAuthentication',
    defaultMessage: 'Authentication',
  },
  useForConfig: {
    id: 'managedSecrets.useForConfig',
    defaultMessage: 'Project config',
  },
  useForBoth: {
    id: 'managedSecrets.useForBoth',
    defaultMessage: 'Authentication + config',
  },
  note: {
    id: 'managedSecrets.note',
    defaultMessage: 'Note from user',
  },
  variableName: {
    id: 'managedSecrets.variableName',
    defaultMessage: 'Variable name',
  },
  variableValue: {
    id: 'managedSecrets.variableValue',
    defaultMessage: 'Secret value',
  },
  addVariable: {
    id: 'managedSecrets.addVariable',
    defaultMessage: 'Add variable',
  },
  deleteProfile: {
    id: 'managedSecrets.deleteProfile',
    defaultMessage: 'Delete profile',
  },
  empty: {
    id: 'managedSecrets.empty',
    defaultMessage:
      'No local secret profiles yet. Add a custom profile or start from a VPS or Supabase template.',
  },
  loading: {
    id: 'managedSecrets.loading',
    defaultMessage: 'Loading local secret profiles...',
  },
  saving: {
    id: 'managedSecrets.saving',
    defaultMessage: 'Saving changes...',
  },
  save: {
    id: 'managedSecrets.save',
    defaultMessage: 'Save',
  },
  saved: {
    id: 'managedSecrets.saved',
    defaultMessage: 'Saved',
  },
  unsaved: {
    id: 'managedSecrets.unsaved',
    defaultMessage: 'Unsaved changes',
  },
  failedToLoad: {
    id: 'managedSecrets.failedToLoad',
    defaultMessage: 'Failed to load local secret profiles',
  },
  failedToSave: {
    id: 'managedSecrets.failedToSave',
    defaultMessage: 'Failed to save local secret profiles: {error}',
  },
  profileSaved: {
    id: 'managedSecrets.profileSaved',
    defaultMessage: 'Local secret profiles saved',
  },
  customName: {
    id: 'managedSecrets.customName',
    defaultMessage: 'Custom Project',
  },
  customNote: {
    id: 'managedSecrets.customNote',
    defaultMessage: '',
  },
  vpsName: {
    id: 'managedSecrets.vpsName',
    defaultMessage: 'VPS Server',
  },
  vpsNote: {
    id: 'managedSecrets.vpsNote',
    defaultMessage: 'This is the password and login to manage the VPS server.',
  },
  supabaseName: {
    id: 'managedSecrets.supabaseName',
    defaultMessage: 'Supabase Project',
  },
  supabaseNote: {
    id: 'managedSecrets.supabaseNote',
    defaultMessage:
      'Project configuration and admin credentials for managing this Supabase project.',
  },
});

type Template = ManagedSecretProfile['template'];

function normalizeProfile(profile: ManagedSecretProfile): ManagedSecretProfile {
  return {
    ...profile,
    useFor: profile.useFor ?? 'both',
  };
}

function createId() {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function createEntry(key = '', value = ''): ManagedSecretEntry {
  return {
    id: createId(),
    key,
    value,
  };
}

function createProfile(template: Template, intl: ReturnType<typeof useIntl>): ManagedSecretProfile {
  switch (template) {
    case 'vps':
      return {
        id: createId(),
        name: intl.formatMessage(i18n.vpsName),
        website: '',
        note: intl.formatMessage(i18n.vpsNote),
        template,
        useFor: 'both',
        entries: [
          createEntry('VPS_SERVER_URL'),
          createEntry('VPS_SERVER_LOGIN'),
          createEntry('VPS_SERVER_PASSWORD'),
        ],
      };
    case 'supabase':
      return {
        id: createId(),
        name: intl.formatMessage(i18n.supabaseName),
        website: '',
        note: intl.formatMessage(i18n.supabaseNote),
        template,
        useFor: 'both',
        entries: [
          createEntry('SUPABASE_PROJECT_URL'),
          createEntry('SUPABASE_PROJECT_REF'),
          createEntry('SUPABASE_ANON_KEY'),
          createEntry('SUPABASE_SERVICE_ROLE_KEY'),
          createEntry('SUPABASE_DB_PASSWORD'),
        ],
      };
    case 'custom':
    default:
      return {
        id: createId(),
        name: intl.formatMessage(i18n.customName),
        website: '',
        note: intl.formatMessage(i18n.customNote),
        template: 'custom',
        useFor: 'both',
        entries: [createEntry()],
      };
  }
}

function templateLabel(template: Template, intl: ReturnType<typeof useIntl>) {
  switch (template) {
    case 'vps':
      return intl.formatMessage(i18n.vpsTemplate);
    case 'supabase':
      return intl.formatMessage(i18n.supabaseTemplate);
    case 'custom':
    default:
      return intl.formatMessage(i18n.customTemplate);
  }
}

function templateIcon(template: Template) {
  switch (template) {
    case 'vps':
      return Server;
    case 'supabase':
      return ShieldEllipsis;
    case 'custom':
    default:
      return KeyRound;
  }
}

function getUseForLabel(useFor: ManagedSecretProfileUse, intl: ReturnType<typeof useIntl>) {
  switch (useFor) {
    case 'authentication':
      return intl.formatMessage(i18n.useForAuthentication);
    case 'config':
      return intl.formatMessage(i18n.useForConfig);
    case 'both':
    default:
      return intl.formatMessage(i18n.useForBoth);
  }
}

export default function ManagedSecretProfilesSection() {
  const intl = useIntl();
  const [profiles, setProfiles] = useState<ManagedSecretProfile[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [revealedValues, setRevealedValues] = useState<Record<string, boolean>>({});
  const hasLoaded = useRef(false);

  useEffect(() => {
    let cancelled = false;

    window.electron
      .getSetting('managedSecretProfiles')
      .then((storedProfiles) => {
        if (cancelled) {
          return;
        }
        const normalizedProfiles = (storedProfiles ?? []).map(normalizeProfile);
        setProfiles(normalizedProfiles);
        setSelectedProfileId(normalizedProfiles[0]?.id ?? null);
        hasLoaded.current = true;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        toast.error(intl.formatMessage(i18n.failedToLoad));
        hasLoaded.current = true;
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [intl]);

  const profileCountLabel = useMemo(() => {
    if (saving) {
      return intl.formatMessage(i18n.saving);
    }
    if (dirty) {
      return intl.formatMessage(i18n.unsaved);
    }
    if (!loading && profiles.length > 0 && hasLoaded.current) {
      return intl.formatMessage(i18n.saved);
    }
    return null;
  }, [dirty, intl, loading, profiles.length, saving]);

  const updateProfiles = (updater: (current: ManagedSecretProfile[]) => ManagedSecretProfile[]) => {
    setProfiles((current) => {
      const next = updater(current);
      if (hasLoaded.current) {
        setDirty(true);
      }
      return next;
    });
  };

  const updateProfile = (
    profileId: string,
    updater: (profile: ManagedSecretProfile) => ManagedSecretProfile
  ) => {
    updateProfiles((current) =>
      current.map((profile) => (profile.id === profileId ? updater(profile) : profile))
    );
  };

  const updateEntry = (
    profileId: string,
    entryId: string,
    field: keyof Pick<ManagedSecretEntry, 'key' | 'value'>,
    value: string
  ) => {
    updateProfile(profileId, (profile) => ({
      ...profile,
      entries: profile.entries.map((entry) =>
        entry.id === entryId ? { ...entry, [field]: value } : entry
      ),
    }));
  };

  const addProfile = (template: Template) => {
    const nextProfile = createProfile(template, intl);
    updateProfiles((current) => [...current, nextProfile]);
    setSelectedProfileId(nextProfile.id);
  };

  const addEntry = (profileId: string) => {
    updateProfile(profileId, (profile) => ({
      ...profile,
      entries: [...profile.entries, createEntry()],
    }));
  };

  const removeProfile = (profileId: string) => {
    updateProfiles((current) => {
      const nextProfiles = current.filter((profile) => profile.id !== profileId);
      setSelectedProfileId((currentSelected) => {
        if (currentSelected !== profileId) {
          return currentSelected;
        }
        return nextProfiles[0]?.id ?? null;
      });
      return nextProfiles;
    });
  };

  const removeEntry = (profileId: string, entryId: string) => {
    updateProfile(profileId, (profile) => ({
      ...profile,
      entries:
        profile.entries.length > 1
          ? profile.entries.filter((entry) => entry.id !== entryId)
          : [createEntry()],
    }));
    setRevealedValues((current) => {
      const next = { ...current };
      delete next[entryId];
      return next;
    });
  };

  const selectedProfile =
    profiles.find((profile) => profile.id === selectedProfileId) ?? profiles[0] ?? null;

  useEffect(() => {
    if (!selectedProfile && profiles.length > 0) {
      setSelectedProfileId(profiles[0].id);
    }
  }, [profiles, selectedProfile]);

  const saveProfiles = async () => {
    setSaving(true);
    try {
      await window.electron.setSetting('managedSecretProfiles', profiles);
      setDirty(false);
      toast.success(intl.formatMessage(i18n.profileSaved));
    } catch (error) {
      toast.error(
        intl.formatMessage(i18n.failedToSave, {
          error: errorMessage(error, 'Unknown error'),
        })
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card className="pb-2">
      <CardHeader className="pb-0">
        <CardTitle className="flex items-center gap-2">
          <KeyRound className="h-4 w-4" />
          {intl.formatMessage(i18n.title)}
        </CardTitle>
        <CardDescription>{intl.formatMessage(i18n.description)}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4 px-4 py-4">
        <div className="flex flex-wrap items-center gap-2">
          <Button type="button" variant="outline" size="sm" onClick={() => addProfile('custom')}>
            <Plus className="h-4 w-4" />
            {intl.formatMessage(i18n.customTemplate)}
          </Button>
          <Button type="button" variant="outline" size="sm" onClick={() => addProfile('vps')}>
            <Server className="h-4 w-4" />
            {intl.formatMessage(i18n.vpsTemplate)}
          </Button>
          <Button type="button" variant="outline" size="sm" onClick={() => addProfile('supabase')}>
            <ShieldEllipsis className="h-4 w-4" />
            {intl.formatMessage(i18n.supabaseTemplate)}
          </Button>
          <Button type="button" size="sm" onClick={saveProfiles} disabled={saving || !dirty}>
            {intl.formatMessage(i18n.save)}
          </Button>
          {profileCountLabel && (
            <span className="text-xs text-text-secondary">{profileCountLabel}</span>
          )}
        </div>

        {loading ? (
          <div className="py-2 text-sm text-text-secondary">{intl.formatMessage(i18n.loading)}</div>
        ) : profiles.length === 0 ? (
          <div className="rounded-md border border-dashed border-border-primary p-4 text-sm text-text-secondary">
            {intl.formatMessage(i18n.empty)}
          </div>
        ) : selectedProfile ? (
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-xs font-medium uppercase tracking-wide text-text-secondary">
                {intl.formatMessage(i18n.profiles)}
              </label>
              <div className="flex flex-wrap gap-2">
                {profiles.map((profile) => {
                  const ProfileIcon = templateIcon(profile.template);
                  const isSelected = profile.id === selectedProfile.id;

                  return (
                    <Button
                      key={profile.id}
                      type="button"
                      variant={isSelected ? 'default' : 'outline'}
                      size="sm"
                      onClick={() => setSelectedProfileId(profile.id)}
                    >
                      <ProfileIcon className="h-4 w-4" />
                      {profile.name}
                    </Button>
                  );
                })}
              </div>
            </div>

            <div
              className="space-y-4 rounded-lg border border-border-primary bg-background-secondary/40 p-4"
              data-testid="managed-secret-profile"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  {(() => {
                    const ProfileIcon = templateIcon(selectedProfile.template);
                    return <ProfileIcon className="h-4 w-4 text-text-secondary" />;
                  })()}
                  <span className="rounded border border-border-primary bg-background-primary px-2 py-0.5 text-xs text-text-secondary">
                    {templateLabel(selectedProfile.template, intl)}
                  </span>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="text-text-secondary hover:text-text-primary"
                  onClick={() => removeProfile(selectedProfile.id)}
                >
                  <Trash2 className="h-4 w-4" />
                  {intl.formatMessage(i18n.deleteProfile)}
                </Button>
              </div>

              <div className="grid gap-3 md:grid-cols-3">
                <div className="space-y-2">
                  <label className="text-xs font-medium uppercase tracking-wide text-text-secondary">
                    {intl.formatMessage(i18n.profileName)}
                  </label>
                  <Input
                    value={selectedProfile.name}
                    onChange={(event) =>
                      updateProfile(selectedProfile.id, (current) => ({
                        ...current,
                        name: event.target.value,
                      }))
                    }
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium uppercase tracking-wide text-text-secondary">
                    {intl.formatMessage(i18n.website)}
                  </label>
                  <div className="relative">
                    <Globe className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-secondary" />
                    <Input
                      value={selectedProfile.website}
                      onChange={(event) =>
                        updateProfile(selectedProfile.id, (current) => ({
                          ...current,
                          website: event.target.value,
                        }))
                      }
                      className="pl-9"
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium uppercase tracking-wide text-text-secondary">
                    {intl.formatMessage(i18n.useFor)}
                  </label>
                  <Select
                    value={{
                      value: selectedProfile.useFor,
                      label: getUseForLabel(selectedProfile.useFor, intl),
                    }}
                    onChange={(option: unknown) => {
                      const selectedOption = option as {
                        value: ManagedSecretProfileUse;
                        label: string;
                      } | null;
                      if (!selectedOption) {
                        return;
                      }
                      updateProfile(selectedProfile.id, (current) => ({
                        ...current,
                        useFor: selectedOption.value,
                      }));
                    }}
                    options={[
                      {
                        value: 'authentication',
                        label: intl.formatMessage(i18n.useForAuthentication),
                      },
                      {
                        value: 'config',
                        label: intl.formatMessage(i18n.useForConfig),
                      },
                      {
                        value: 'both',
                        label: intl.formatMessage(i18n.useForBoth),
                      },
                    ]}
                    isSearchable={false}
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-xs font-medium uppercase tracking-wide text-text-secondary">
                  {intl.formatMessage(i18n.note)}
                </label>
                <textarea
                  value={selectedProfile.note}
                  onChange={(event) =>
                    updateProfile(selectedProfile.id, (current) => ({
                      ...current,
                      note: event.target.value,
                    }))
                  }
                  rows={3}
                  className="flex w-full rounded-md border border-border-primary bg-background-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-secondary focus:border-border-secondary focus-visible:outline-none"
                />
              </div>

              <div className="space-y-3">
                {selectedProfile.entries.map((entry) => {
                  const isRevealed = !!revealedValues[entry.id];

                  return (
                    <div
                      key={entry.id}
                      className="grid gap-2 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto]"
                    >
                      <Input
                        value={entry.key}
                        onChange={(event) =>
                          updateEntry(selectedProfile.id, entry.id, 'key', event.target.value)
                        }
                        placeholder={intl.formatMessage(i18n.variableName)}
                      />
                      <Input
                        value={entry.value}
                        onChange={(event) =>
                          updateEntry(selectedProfile.id, entry.id, 'value', event.target.value)
                        }
                        placeholder={intl.formatMessage(i18n.variableValue)}
                        type={isRevealed ? 'text' : 'password'}
                      />
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        shape="round"
                        onClick={() =>
                          setRevealedValues((current) => ({
                            ...current,
                            [entry.id]: !current[entry.id],
                          }))
                        }
                        aria-label={isRevealed ? 'Hide secret value' : 'Show secret value'}
                      >
                        {isRevealed ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        shape="round"
                        onClick={() => removeEntry(selectedProfile.id, entry.id)}
                        aria-label="Remove secret variable"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  );
                })}

                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => addEntry(selectedProfile.id)}
                  className="w-fit"
                >
                  <Plus className="h-4 w-4" />
                  {intl.formatMessage(i18n.addVariable)}
                </Button>
              </div>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
