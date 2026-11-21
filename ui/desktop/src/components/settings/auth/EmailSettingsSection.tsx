import { useEffect, useState } from 'react';
import type {
  EmailActivity,
  EmailEndpoint,
  EmailPreset,
  EmailProtocol,
  EmailRequest,
  EmailSettings,
  EmailSnapshot,
} from '../../../types/email';
import { applyEmailPreset } from '../../../types/email';
import { errorMessage } from '../../../utils/conversionUtils';
import type { View, ViewOptions } from '../../../utils/navigationUtils';
import { Button } from '../../ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../../ui/dialog';
import { Input } from '../../ui/input';
import { SwitchModelModal } from '../models/subcomponents/SwitchModelModal';

const MAX_INSTRUCTION_BYTES = 8_000;

const initialSettings: EmailSettings = {
  address: '',
  protocol: 'imap',
  incoming: { host: 'imap.purelymail.com', port: 993, startTls: false, username: '' },
  outgoing: { host: 'smtp.purelymail.com', port: 465, startTls: false, username: '' },
  provider: '',
  model: '',
  instructions: '',
  senders: [],
  recipients: [],
  folders: [],
  archive: '',
  trash: '',
  includeExisting: false,
  localArchive: false,
  captureNotes: false,
};

function inferPreset(settings: EmailSettings): EmailPreset {
  for (const preset of ['purelymail', 'gmail'] as const) {
    const defaults = applyEmailPreset(settings, preset, settings.protocol);
    if (
      settings.incoming.host === defaults.incoming.host &&
      settings.outgoing.host === defaults.outgoing.host
    ) {
      return preset;
    }
  }
  return 'custom';
}

const fieldsClass = 'grid gap-2 text-sm';
const selectClass = 'h-9 rounded-md border border-input bg-background px-2 text-sm';
const contentClass = 'max-h-56 overflow-auto whitespace-pre-wrap break-words rounded bg-muted p-2';

function EndpointFields({
  name,
  endpoint,
  update,
}: {
  name: string;
  endpoint: EmailEndpoint;
  update: (endpoint: EmailEndpoint) => void;
}) {
  return (
    <fieldset className="grid gap-2 rounded-md border p-3 sm:grid-cols-2">
      <legend className="px-1 text-sm font-medium">{name}</legend>
      <label className={fieldsClass}>
        Hostname
        <Input
          value={endpoint.host}
          onChange={(event) => update({ ...endpoint, host: event.target.value })}
        />
      </label>
      <label className={fieldsClass}>
        Port
        <Input
          type="number"
          min={1}
          max={65535}
          value={endpoint.port || ''}
          onChange={(event) => update({ ...endpoint, port: Number(event.target.value) })}
        />
      </label>
      <label className={fieldsClass}>
        Login
        <Input
          value={endpoint.username}
          placeholder="Use the mailbox address"
          onChange={(event) => update({ ...endpoint, username: event.target.value })}
        />
      </label>
      <label className={fieldsClass}>
        Encryption
        <select
          className={selectClass}
          value={endpoint.startTls ? 'startTls' : 'tls'}
          onChange={(event) => update({ ...endpoint, startTls: event.target.value === 'startTls' })}
        >
          <option value="tls">TLS</option>
          <option value="startTls">STARTTLS</option>
        </select>
      </label>
    </fieldset>
  );
}

function ActivityList({
  activity,
  busy,
  canSend,
  onAction,
  onOpenTask,
  onOpenSession,
}: {
  activity: EmailActivity[];
  busy: boolean;
  canSend: boolean;
  onOpenTask: (item: EmailActivity) => void;
  onOpenSession: (id: string) => void;
  onAction: (action: 'send' | 'restore' | 'discard' | 'retryResearch', id: string) => void;
}) {
  return (
    <details className="rounded-md border p-3 text-sm">
      <summary className="cursor-pointer font-medium">Mail activity ({activity.length})</summary>
      {activity.length === 0 && <p className="mt-2 text-muted-foreground">No mail activity yet.</p>}
      <ul className="mt-2 max-h-[32rem] divide-y overflow-auto">
        {activity.map((item) => (
          <li key={item.id} className="space-y-2 py-2">
            <div className="break-words font-medium">{item.subject}</div>
            <div className="text-muted-foreground">
              {item.status} · <time dateTime={item.at}>{new Date(item.at).toLocaleString()}</time>
            </div>
            {item.taskResult && (
              <details>
                <summary className="cursor-pointer">Read task response</summary>
                <p className={contentClass}>{item.taskResult}</p>
              </details>
            )}
            {item.paper && (
              <details>
                <summary className="cursor-pointer">Research intake</summary>
                {item.paper.summary && <p className={contentClass}>{item.paper.summary}</p>}
                <ul>
                  {(
                    [
                      ['zotero', 'Zotero'],
                      ['math', 'Math MCP'],
                      ['physics', 'Alfred Physics'],
                    ] as const
                  ).map(([key, label]) => {
                    if (item.paper?.scope && item.paper.scope !== key) return null;
                    const outcome = item.paper?.destinations[key];
                    return (
                      <li key={key}>
                        {label} ·{' '}
                        {outcome?.status === 'saved'
                          ? outcome.detail
                          : outcome?.status === 'inProgress'
                            ? 'Outcome unknown'
                            : outcome?.status === 'failed'
                              ? 'Import could not be verified'
                              : 'Waiting for import'}
                      </li>
                    );
                  })}
                </ul>
                {item.localBody !== null &&
                  Object.values(item.paper.destinations).some(
                    (outcome) => outcome.status !== 'saved'
                  ) && (
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy}
                      onClick={() => onAction('retryResearch', item.id)}
                    >
                      Retry unverified imports
                    </Button>
                  )}
              </details>
            )}
            {item.draft && (
              <details>
                <summary className="cursor-pointer">Review reply to {item.draft.recipient}</summary>
                <p className={contentClass}>{item.draft.text}</p>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy || !canSend}
                  onClick={() => onAction('send', item.id)}
                >
                  Send reply
                </Button>
              </details>
            )}
            {item.localBody !== null && (
              <details>
                <summary className="cursor-pointer">Read local copy in {item.localFolder}</summary>
                <p className={contentClass}>{item.localBody}</p>
              </details>
            )}
            <div className="flex flex-wrap gap-2">
              {item.taskSessionId && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => onOpenSession(item.taskSessionId!)}
                >
                  Open task session
                </Button>
              )}
              {item.taskRequest && item.localBody !== null && (
                <Button size="sm" variant="outline" onClick={() => onOpenTask(item)}>
                  Open as chat draft
                </Button>
              )}
              {(item.trashed || item.localFolder === 'Trash') && (
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy}
                  onClick={() => onAction('restore', item.id)}
                >
                  Restore
                </Button>
              )}
              {(item.draft || item.localBody !== null || item.trashed) && (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => onAction('discard', item.id)}
                >
                  {item.trashed ? 'Clear restore record' : 'Discard local copy'}
                </Button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </details>
  );
}

export default function EmailSettingsSection({
  setView,
}: {
  setView: (view: View, options?: ViewOptions) => void;
}) {
  const [snapshot, setSnapshot] = useState<EmailSnapshot>();
  const [settings, setSettings] = useState(initialSettings);
  const [preset, setPreset] = useState<EmailPreset>('purelymail');
  const [senders, setSenders] = useState('');
  const [recipients, setRecipients] = useState('');
  const [folders, setFolders] = useState('');
  const [incomingPassword, setIncomingPassword] = useState('');
  const [outgoingPassword, setOutgoingPassword] = useState('');
  const [separatePassword, setSeparatePassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState('');
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [instructionsDraft, setInstructionsDraft] = useState<string | null>(null);

  function applySaved(next: EmailSnapshot) {
    setSnapshot(next);
    const saved = next.account?.settings ?? initialSettings;
    setSettings({ ...saved, instructions: saved.instructions ?? '' });
    setSenders((saved.senders ?? []).join('\n'));
    setRecipients(saved.recipients.join('\n'));
    setFolders(saved.folders.join('\n'));
    setPreset(inferPreset(saved));
    setDirty(false);
  }

  useEffect(() => {
    let live = true;
    void window.electron.emailRequest({ op: 'status' }).then(
      (next) => {
        if (live) applySaved(next);
      },
      (error: unknown) => {
        if (live) setError(errorMessage(error));
      }
    );
    return () => {
      live = false;
    };
  }, []);

  async function run(request: EmailRequest) {
    setBusy(true);
    setError('');
    try {
      const next = await window.electron.emailRequest(request);
      if (request.op === 'save' || request.op === 'status' || request.op === 'reset') {
        applySaved(next);
      } else {
        setSnapshot(next);
      }
      if (request.op === 'save') {
        setIncomingPassword('');
        setOutgoingPassword('');
      }
    } catch (error) {
      setError(errorMessage(error));
      if (request.op === 'poll' || request.op === 'send' || request.op === 'restore') {
        const next = await window.electron.emailRequest({ op: 'status' }).catch(() => null);
        if (next) setSnapshot(next);
      }
    } finally {
      setBusy(false);
    }
  }

  function update(change: Partial<EmailSettings>) {
    setSettings((previous) => ({ ...previous, ...change }));
    setDirty(true);
  }

  const account = snapshot?.account;
  const hasAllowedSenders = Boolean(account?.settings.senders?.length);
  const smtpTested = account?.smtpTested ?? account?.tested ?? false;
  const canSend = Boolean(account?.enabled && account.tested && smtpTested);
  const revision = snapshot?.revision ?? 0;
  const disabled = busy || !snapshot;
  const instructionsTooLong =
    instructionsDraft !== null &&
    new TextEncoder().encode(instructionsDraft).length > MAX_INSTRUCTION_BYTES;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Email</CardTitle>
        <CardDescription>
          Give Gosling its own inbox to organize mail and draft replies while Desktop is running.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <form
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            const address = settings.address.trim();
            const password =
              preset === 'gmail' ? incomingPassword.replace(/\s/g, '') : incomingPassword;
            const lines = (value: string) =>
              value
                .split(/\r?\n/)
                .map((line) => line.trim())
                .filter(Boolean);
            void run({
              op: 'save',
              revision,
              incoming_password: password,
              outgoing_password: separatePassword ? outgoingPassword : password,
              settings: {
                ...settings,
                address,
                incoming: {
                  ...settings.incoming,
                  username: settings.incoming.username.trim() || address,
                },
                outgoing: {
                  ...settings.outgoing,
                  username: settings.outgoing.username.trim() || address,
                },
                senders: lines(senders),
                recipients: lines(recipients),
                folders: lines(folders),
              },
            });
          }}
        >
          <fieldset disabled={disabled} className="space-y-4">
            <div className="grid gap-3 sm:grid-cols-2">
              <label className={fieldsClass}>
                Provider
                <select
                  className={selectClass}
                  value={preset}
                  onChange={(event) => {
                    const next = event.target.value as EmailPreset;
                    setPreset(next);
                    update(applyEmailPreset(settings, next, settings.protocol));
                  }}
                >
                  <option value="purelymail">Purelymail</option>
                  <option value="gmail">Gmail</option>
                  <option value="custom">Custom</option>
                </select>
              </label>
              <label className={fieldsClass}>
                Incoming protocol
                <select
                  className={selectClass}
                  value={settings.protocol}
                  onChange={(event) =>
                    update(applyEmailPreset(settings, preset, event.target.value as EmailProtocol))
                  }
                >
                  <option value="imap">IMAP</option>
                  <option value="pop">POP3</option>
                </select>
              </label>
              <label className={fieldsClass}>
                Dedicated mailbox address
                <Input
                  type="email"
                  required
                  value={settings.address}
                  onChange={(event) => {
                    const address = event.target.value;
                    update({
                      address,
                      incoming: {
                        ...settings.incoming,
                        username:
                          settings.incoming.username === settings.address
                            ? address
                            : settings.incoming.username,
                      },
                      outgoing: {
                        ...settings.outgoing,
                        username:
                          settings.outgoing.username === settings.address
                            ? address
                            : settings.outgoing.username,
                      },
                    });
                  }}
                />
              </label>
              <label className={fieldsClass}>
                {preset === 'gmail' ? 'Google app password' : 'Mailbox or app password'}
                <Input
                  type="password"
                  autoComplete="new-password"
                  placeholder={
                    account?.hasCredentials ? 'Leave blank to keep saved credentials' : undefined
                  }
                  value={incomingPassword}
                  onChange={(event) => {
                    setIncomingPassword(event.target.value);
                    setDirty(true);
                  }}
                />
              </label>
            </div>
            {preset === 'gmail' && (
              <p className="text-sm text-muted-foreground">
                Use a dedicated Gmail account eligible for app passwords. Enable POP in Gmail if you
                choose POP3.
              </p>
            )}
            {settings.protocol === 'pop' && (
              <p className="text-sm text-muted-foreground">
                POP folders and Trash are local. Check the provider’s settings for what happens to
                downloaded copies.
              </p>
            )}
            <details
              open={preset === 'custom' || undefined}
              className="space-y-3 rounded-md border p-3"
            >
              <summary className="cursor-pointer text-sm font-medium">Server settings</summary>
              <EndpointFields
                name="Incoming server"
                endpoint={settings.incoming}
                update={(incoming) => update({ incoming })}
              />
              <EndpointFields
                name="SMTP server"
                endpoint={settings.outgoing}
                update={(outgoing) => update({ outgoing })}
              />
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={separatePassword}
                  onChange={(event) => setSeparatePassword(event.target.checked)}
                />
                Use a separate SMTP password
              </label>
              {separatePassword && (
                <label className={fieldsClass}>
                  SMTP password
                  <Input
                    type="password"
                    autoComplete="new-password"
                    value={outgoingPassword}
                    onChange={(event) => {
                      setOutgoingPassword(event.target.value);
                      setDirty(true);
                    }}
                  />
                </label>
              )}
            </details>
            <div className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="outline" onClick={() => setModelPickerOpen(true)}>
                {settings.model ? 'Change email model' : 'Choose email model'}
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={() => setInstructionsDraft(settings.instructions)}
              >
                Instructions
              </Button>
              {settings.model && (
                <span className="break-all text-sm text-muted-foreground">
                  {settings.provider} / {settings.model}
                </span>
              )}
            </div>
            <details className="space-y-3 rounded-md border p-3">
              <summary className="cursor-pointer text-sm font-medium">Receive allowlist</summary>
              <label className={fieldsClass}>
                Allowed senders
                <textarea
                  className="min-h-20 rounded-md border border-input bg-background p-2"
                  placeholder="One address per line"
                  value={senders}
                  onChange={(event) => {
                    setSenders(event.target.value);
                    setDirty(true);
                  }}
                />
              </label>
              <p className="text-sm text-muted-foreground">
                Gosling processes mail only from these addresses. An empty list stops processing.
              </p>
            </details>
            <details className="space-y-3 rounded-md border p-3">
              <summary className="cursor-pointer text-sm font-medium">Send allowlist</summary>
              <label className={fieldsClass}>
                Allowed recipients
                <textarea
                  className="min-h-20 rounded-md border border-input bg-background p-2"
                  placeholder="One address per line"
                  value={recipients}
                  onChange={(event) => {
                    setRecipients(event.target.value);
                    setDirty(true);
                  }}
                />
              </label>
              <p className="text-sm text-muted-foreground">
                Gosling can send email only to these addresses. An empty list blocks all sends.
              </p>
            </details>
            <details className="space-y-3 rounded-md border p-3">
              <summary className="cursor-pointer text-sm font-medium">Advanced</summary>
              {settings.protocol === 'imap' && (
                <div className="grid gap-3 sm:grid-cols-2">
                  <label className={fieldsClass}>
                    Archive folder
                    <Input
                      value={settings.archive}
                      placeholder="Detect from server"
                      onChange={(event) => update({ archive: event.target.value })}
                    />
                  </label>
                  <label className={fieldsClass}>
                    Trash folder
                    <Input
                      value={settings.trash}
                      placeholder="Detect from server"
                      onChange={(event) => update({ trash: event.target.value })}
                    />
                  </label>
                </div>
              )}
              <label className={fieldsClass}>
                Filing folders
                <textarea
                  className="min-h-16 rounded-md border border-input bg-background p-2"
                  placeholder="One folder name per line"
                  value={folders}
                  onChange={(event) => {
                    setFolders(event.target.value);
                    setDirty(true);
                  }}
                />
              </label>
              {!account && (
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={settings.includeExisting}
                    onChange={(event) => update({ includeExisting: event.target.checked })}
                  />
                  Process existing mail on the first inbox check
                </label>
              )}
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={settings.captureNotes ?? false}
                  onChange={(event) => update({ captureNotes: event.target.checked })}
                />
                Save notes and task requests locally
              </label>
              {settings.protocol === 'imap' && !settings.captureNotes && (
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={settings.localArchive}
                    onChange={(event) => update({ localArchive: event.target.checked })}
                  />
                  Store archives locally
                </label>
              )}
            </details>
            <Button
              type="submit"
              disabled={
                !settings.provider ||
                !settings.model ||
                (!dirty && Boolean(account?.hasCredentials))
              }
            >
              Save email settings
            </Button>
          </fieldset>
        </form>
        {modelPickerOpen && (
          <SwitchModelModal
            forEmail
            sessionId={null}
            titleOverride="Choose email model"
            descriptionOverride="Select a direct model provider and model to process email."
            initialProvider={settings.provider}
            initialModel={settings.model}
            onModelSelected={(model, provider) => update({ model, provider })}
            onClose={() => setModelPickerOpen(false)}
            setView={setView}
          />
        )}
        {instructionsDraft !== null && (
          <Dialog open onOpenChange={(open) => !open && setInstructionsDraft(null)}>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Email instructions</DialogTitle>
                <DialogDescription>
                  Tell Gosling how to sort mail and draft replies for this mailbox.
                </DialogDescription>
              </DialogHeader>
              <textarea
                aria-label="Email instructions"
                aria-invalid={instructionsTooLong}
                aria-describedby={instructionsTooLong ? 'email-instructions-error' : undefined}
                className="min-h-40 rounded-md border border-input bg-background p-3 text-sm"
                value={instructionsDraft}
                onChange={(event) => setInstructionsDraft(event.target.value)}
              />
              {instructionsTooLong && (
                <p id="email-instructions-error" role="alert" className="text-sm text-destructive">
                  Email instructions must be 8,000 bytes or fewer.
                </p>
              )}
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setInstructionsDraft(null)}>
                  Cancel
                </Button>
                <Button
                  type="button"
                  disabled={instructionsTooLong}
                  onClick={() => {
                    update({ instructions: instructionsDraft });
                    setInstructionsDraft(null);
                  }}
                >
                  Apply
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        )}
        {account && (
          <>
            <p className="text-sm">
              {account.enabled ? 'Monitoring inbox' : 'Paused'} · {account.settings.provider} /{' '}
              {account.settings.model}
            </p>
            <p className="text-sm text-muted-foreground">
              Gosling shares accepted message text with the model above. When SMTP is verified,
              replies to addresses on both allowlists can be sent automatically.
            </p>
            {!hasAllowedSenders && (
              <p className="text-sm text-muted-foreground">
                Add a sender to the Receive allowlist to start processing mail.
              </p>
            )}
            {account.tested && !smtpTested && (
              <p className="text-sm text-muted-foreground">
                Sending is unavailable until SMTP passes its connection test.
              </p>
            )}
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                disabled={disabled || dirty || !account.hasCredentials}
                onClick={() => void run({ op: 'test' })}
              >
                Test connection
              </Button>
              <Button
                variant="outline"
                disabled={disabled || (!account.enabled && (dirty || !account.tested))}
                onClick={() => void run({ op: account.enabled ? 'pause' : 'enable', revision })}
              >
                {account.enabled ? 'Disable email' : 'Enable email'}
              </Button>
              <Button
                variant="outline"
                disabled={disabled || !account.enabled || dirty || !hasAllowedSenders}
                onClick={() => void run({ op: 'poll' })}
              >
                Process now
              </Button>
              <Button
                variant="ghost"
                disabled={busy || dirty}
                onClick={() => void run({ op: 'status' })}
              >
                Refresh activity
              </Button>
            </div>
            <details className="space-y-2 text-sm">
              <summary className="cursor-pointer font-medium">Mailbox maintenance</summary>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  disabled={disabled || (!account.tested && !smtpTested)}
                  onClick={() => void run({ op: 'disconnect', revision })}
                >
                  Disconnect
                </Button>
                <Button
                  variant="outline"
                  disabled={disabled || account.enabled || dirty}
                  onClick={() => void run({ op: 'reset', revision })}
                >
                  Reset inbox checkpoint
                </Button>
                <Button
                  variant="outline"
                  disabled={disabled || !account.hasCredentials}
                  onClick={() => void run({ op: 'forget', revision })}
                >
                  Forget credentials
                </Button>
              </div>
            </details>
            <ActivityList
              activity={snapshot?.activity ?? []}
              busy={disabled || dirty}
              canSend={canSend}
              onAction={(op, id) => void run({ op, id, revision })}
              onOpenSession={(resumeSessionId) => setView('pair', { resumeSessionId })}
              onOpenTask={(item) =>
                setView('chat', {
                  initialMessage: {
                    msg: [item.subject, item.localBody].filter(Boolean).join('\n\n'),
                    images: [],
                  },
                })
              }
            />
          </>
        )}
        {snapshot?.lastStatus && (
          <p role="status" className="text-sm text-muted-foreground">
            {snapshot.lastStatus}
          </p>
        )}
        {error && (
          <div role="alert" className="text-sm text-destructive">
            {error}
            {!snapshot && (
              <Button variant="ghost" disabled={busy} onClick={() => void run({ op: 'status' })}>
                Retry
              </Button>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
