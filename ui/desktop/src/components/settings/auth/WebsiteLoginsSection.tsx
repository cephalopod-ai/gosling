import { useCallback, useEffect, useState } from 'react';
import { Globe, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import { toast } from 'react-toastify';
import {
  deleteWebsiteLogin,
  listWebsiteLogins,
  saveWebsiteLogin,
  type WebsiteLoginDto,
} from '../../../acp/websiteLogins';
import { errorMessage } from '../../../utils/conversionUtils';
import { Button } from '../../ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import { ConfirmationModal } from '../../ui/ConfirmationModal';
import { Input } from '../../ui/input';
import { defineMessages, useIntl } from '../../../i18n';

const i18n = defineMessages({
  title: {
    id: 'websiteLogins.title',
    defaultMessage: 'Website Logins',
  },
  description: {
    id: 'websiteLogins.description',
    defaultMessage:
      'Save a website with its username and password so gosling can sign in for you. The agent sees the website and username, never the password: you approve each sign-in, and gosling fills the password in itself.',
  },
  add: {
    id: 'websiteLogins.add',
    defaultMessage: 'Add login',
  },
  loading: {
    id: 'websiteLogins.loading',
    defaultMessage: 'Loading website logins...',
  },
  empty: {
    id: 'websiteLogins.empty',
    defaultMessage: 'No website logins saved yet.',
  },
  failedToLoad: {
    id: 'websiteLogins.failedToLoad',
    defaultMessage: 'Failed to load website logins',
  },
  nameLabel: {
    id: 'websiteLogins.nameLabel',
    defaultMessage: 'Name',
  },
  namePlaceholder: {
    id: 'websiteLogins.namePlaceholder',
    defaultMessage: 'Work GitHub',
  },
  urlLabel: {
    id: 'websiteLogins.urlLabel',
    defaultMessage: 'Website',
  },
  urlPlaceholder: {
    id: 'websiteLogins.urlPlaceholder',
    defaultMessage: 'https://github.com/login',
  },
  usernameLabel: {
    id: 'websiteLogins.usernameLabel',
    defaultMessage: 'Username or email',
  },
  passwordLabel: {
    id: 'websiteLogins.passwordLabel',
    defaultMessage: 'Password',
  },
  passwordKeepPlaceholder: {
    id: 'websiteLogins.passwordKeepPlaceholder',
    defaultMessage: 'Leave blank to keep the saved password',
  },
  passwordMissing: {
    id: 'websiteLogins.passwordMissing',
    defaultMessage: 'No password saved',
  },
  agentUses: {
    id: 'websiteLogins.agentUses',
    defaultMessage: 'Agent uses',
  },
  save: {
    id: 'websiteLogins.save',
    defaultMessage: 'Save',
  },
  cancel: {
    id: 'websiteLogins.cancel',
    defaultMessage: 'Cancel',
  },
  saved: {
    id: 'websiteLogins.saved',
    defaultMessage: 'Website login saved',
  },
  failedToSave: {
    id: 'websiteLogins.failedToSave',
    defaultMessage: 'Failed to save website login: {error}',
  },
  edit: {
    id: 'websiteLogins.edit',
    defaultMessage: 'Edit {name}',
  },
  delete: {
    id: 'websiteLogins.delete',
    defaultMessage: 'Delete {name}',
  },
  deleteTitle: {
    id: 'websiteLogins.deleteTitle',
    defaultMessage: 'Delete website login',
  },
  deleteMessage: {
    id: 'websiteLogins.deleteMessage',
    defaultMessage: 'Delete the {name} login and its saved password?',
  },
  deleteConfirm: {
    id: 'websiteLogins.deleteConfirm',
    defaultMessage: 'Delete',
  },
  deleted: {
    id: 'websiteLogins.deleted',
    defaultMessage: 'Website login deleted',
  },
  failedToDelete: {
    id: 'websiteLogins.failedToDelete',
    defaultMessage: 'Failed to delete website login: {error}',
  },
});

interface LoginFormState {
  id?: string;
  name: string;
  url: string;
  username: string;
  password: string;
}

const EMPTY_FORM: LoginFormState = { name: '', url: '', username: '', password: '' };

interface LoginFormProps {
  form: LoginFormState;
  isSubmitting: boolean;
  onChange: (form: LoginFormState) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

function LoginForm({ form, isSubmitting, onChange, onSubmit, onCancel }: LoginFormProps) {
  const intl = useIntl();
  const isEditing = form.id !== undefined;
  const fieldId = (field: string) => `website-login-${form.id ?? 'new'}-${field}`;
  const canSubmit =
    form.name.trim() !== '' &&
    form.url.trim() !== '' &&
    form.username.trim() !== '' &&
    (isEditing || form.password !== '');

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      className="space-y-3 rounded-lg border border-border-primary bg-background-secondary p-3"
      data-testid="website-login-form"
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-1">
          <label htmlFor={fieldId('name')} className="text-xs font-medium text-text-primary">
            {intl.formatMessage(i18n.nameLabel)}
          </label>
          <Input
            id={fieldId('name')}
            value={form.name}
            onChange={(event) => onChange({ ...form, name: event.target.value })}
            placeholder={intl.formatMessage(i18n.namePlaceholder)}
            autoFocus
          />
        </div>
        <div className="space-y-1">
          <label htmlFor={fieldId('url')} className="text-xs font-medium text-text-primary">
            {intl.formatMessage(i18n.urlLabel)}
          </label>
          <Input
            id={fieldId('url')}
            type="url"
            value={form.url}
            onChange={(event) => onChange({ ...form, url: event.target.value })}
            placeholder={intl.formatMessage(i18n.urlPlaceholder)}
          />
        </div>
        <div className="space-y-1">
          <label htmlFor={fieldId('username')} className="text-xs font-medium text-text-primary">
            {intl.formatMessage(i18n.usernameLabel)}
          </label>
          <Input
            id={fieldId('username')}
            value={form.username}
            onChange={(event) => onChange({ ...form, username: event.target.value })}
            autoComplete="off"
          />
        </div>
        <div className="space-y-1">
          <label htmlFor={fieldId('password')} className="text-xs font-medium text-text-primary">
            {intl.formatMessage(i18n.passwordLabel)}
          </label>
          <Input
            id={fieldId('password')}
            type="password"
            value={form.password}
            onChange={(event) => onChange({ ...form, password: event.target.value })}
            placeholder={isEditing ? intl.formatMessage(i18n.passwordKeepPlaceholder) : undefined}
            autoComplete="new-password"
          />
        </div>
      </div>
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={isSubmitting || !canSubmit}>
          {isSubmitting ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            intl.formatMessage(i18n.save)
          )}
        </Button>
        <Button type="button" variant="outline" size="sm" onClick={onCancel} disabled={isSubmitting}>
          {intl.formatMessage(i18n.cancel)}
        </Button>
      </div>
    </form>
  );
}

export default function WebsiteLoginsSection() {
  const intl = useIntl();
  const [logins, setLogins] = useState<WebsiteLoginDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [form, setForm] = useState<LoginFormState | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [loginToDelete, setLoginToDelete] = useState<WebsiteLoginDto | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  const loadLogins = useCallback(async () => {
    setLoading(true);
    try {
      setLogins(await listWebsiteLogins());
    } catch {
      toast.error(intl.formatMessage(i18n.failedToLoad));
      setLogins([]);
    } finally {
      setLoading(false);
    }
  }, [intl]);

  useEffect(() => {
    loadLogins();
  }, [loadLogins]);

  const submitForm = async () => {
    if (!form) {
      return;
    }
    setIsSubmitting(true);
    try {
      await saveWebsiteLogin({
        id: form.id,
        name: form.name.trim(),
        url: form.url.trim(),
        username: form.username.trim(),
        password: form.password || undefined,
      });
      toast.success(intl.formatMessage(i18n.saved));
      setForm(null);
      await loadLogins();
    } catch (error) {
      toast.error(
        intl.formatMessage(i18n.failedToSave, { error: errorMessage(error, 'Unknown error') })
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const confirmDelete = async () => {
    if (!loginToDelete) {
      return;
    }
    setIsDeleting(true);
    try {
      await deleteWebsiteLogin(loginToDelete.id);
      toast.success(intl.formatMessage(i18n.deleted));
      setLoginToDelete(null);
      await loadLogins();
    } catch (error) {
      toast.error(
        intl.formatMessage(i18n.failedToDelete, { error: errorMessage(error, 'Unknown error') })
      );
    } finally {
      setIsDeleting(false);
    }
  };

  const isAdding = form !== null && form.id === undefined;

  return (
    <section id="website-logins" className="space-y-4 pr-4 mt-1">
      <Card className="pb-2">
        <CardHeader className="pb-0">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle className="flex items-center gap-2">
                <Globe className="h-4 w-4" />
                {intl.formatMessage(i18n.title)}
              </CardTitle>
              <CardDescription>{intl.formatMessage(i18n.description)}</CardDescription>
            </div>
            <Button
              size="sm"
              className="gap-2 self-start"
              onClick={() => setForm({ ...EMPTY_FORM })}
              disabled={form !== null}
            >
              <Plus className="h-4 w-4" />
              {intl.formatMessage(i18n.add)}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-3 px-4 py-2">
          {form && isAdding && (
            <LoginForm
              form={form}
              isSubmitting={isSubmitting}
              onChange={setForm}
              onSubmit={submitForm}
              onCancel={() => setForm(null)}
            />
          )}
          {loading ? (
            <div className="flex items-center gap-2 py-6 text-sm text-text-secondary">
              <Loader2 className="h-4 w-4 animate-spin" />
              {intl.formatMessage(i18n.loading)}
            </div>
          ) : logins.length === 0 ? (
            !isAdding && (
              <div className="py-6 text-sm text-text-secondary">
                {intl.formatMessage(i18n.empty)}
              </div>
            )
          ) : (
            logins.map((login) =>
              form?.id === login.id ? (
                <LoginForm
                  key={login.id}
                  form={form}
                  isSubmitting={isSubmitting}
                  onChange={setForm}
                  onSubmit={submitForm}
                  onCancel={() => setForm(null)}
                />
              ) : (
                <div
                  key={login.id}
                  className="rounded-lg border border-border-primary p-3"
                  data-testid="website-login-card"
                >
                  <div className="flex items-start justify-between gap-3">
                    <h3 className="min-w-0 truncate text-sm font-medium text-text-primary">
                      {login.name}
                    </h3>
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        shape="round"
                        className="text-text-secondary hover:text-text-primary"
                        disabled={form !== null}
                        onClick={() =>
                          setForm({
                            id: login.id,
                            name: login.name,
                            url: login.url,
                            username: login.username,
                            password: '',
                          })
                        }
                        aria-label={intl.formatMessage(i18n.edit, { name: login.name })}
                        title={intl.formatMessage(i18n.edit, { name: login.name })}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        shape="round"
                        className="text-text-secondary hover:text-text-primary"
                        onClick={() => setLoginToDelete(login)}
                        aria-label={intl.formatMessage(i18n.delete, { name: login.name })}
                        title={intl.formatMessage(i18n.delete, { name: login.name })}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                  <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
                    <dt className="text-text-secondary">{intl.formatMessage(i18n.urlLabel)}</dt>
                    <dd className="min-w-0 break-all text-text-primary">{login.url}</dd>
                    <dt className="text-text-secondary">
                      {intl.formatMessage(i18n.usernameLabel)}
                    </dt>
                    <dd className="min-w-0 break-all text-text-primary">{login.username}</dd>
                    <dt className="text-text-secondary">
                      {intl.formatMessage(i18n.passwordLabel)}
                    </dt>
                    <dd className="text-text-primary">
                      {login.hasPassword ? (
                        '••••••••'
                      ) : (
                        <span className="text-red-600 dark:text-red-400">
                          {intl.formatMessage(i18n.passwordMissing)}
                        </span>
                      )}
                    </dd>
                    <dt className="text-text-secondary">{intl.formatMessage(i18n.agentUses)}</dt>
                    <dd className="min-w-0 break-all font-mono text-text-secondary">
                      {login.placeholder}
                    </dd>
                  </dl>
                </div>
              )
            )
          )}
        </CardContent>
      </Card>

      <ConfirmationModal
        isOpen={!!loginToDelete}
        title={intl.formatMessage(i18n.deleteTitle)}
        message={
          loginToDelete ? intl.formatMessage(i18n.deleteMessage, { name: loginToDelete.name }) : ''
        }
        onConfirm={confirmDelete}
        onCancel={() => setLoginToDelete(null)}
        confirmLabel={intl.formatMessage(i18n.deleteConfirm)}
        cancelLabel={intl.formatMessage(i18n.cancel)}
        confirmVariant="destructive"
        isSubmitting={isDeleting}
      />
    </section>
  );
}
