use super::Settings;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use uuid::Uuid;

const VERSION: u32 = 1;
const RECENT_ACTIVITY: usize = 80;
const CLASSIFICATION_ATTEMPTS: u32 = 3;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Account {
    pub settings: Settings,
    pub secret_id: Option<String>,
    pub enabled: bool,
    pub tested: bool,
    #[serde(default)]
    pub smtp_tested: Option<bool>,
}

impl Account {
    pub fn smtp_tested(&self) -> bool {
        // Older mailboxes set tested only after both connections passed.
        self.smtp_tested.unwrap_or(self.tested)
    }

    pub fn can_send(&self) -> bool {
        self.enabled && self.tested && self.smtp_tested()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum Cursor {
    Imap { validity: u32, seen_through: u32 },
    Pop { seen: BTreeSet<String> },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Draft {
    pub recipient: String,
    pub text: String,
    pub thread_id: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Trashed {
    pub folder: String,
    pub uid: u32,
    pub validity: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Activity {
    pub id: String,
    pub subject: String,
    pub status: String,
    pub at: String,
    pub draft: Option<Draft>,
    pub local_body: Option<String>,
    pub local_folder: Option<String>,
    pub trashed: Option<Trashed>,
}

impl Activity {
    pub fn new(subject: String, status: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            subject,
            status: status.into(),
            at: chrono::Utc::now().to_rfc3339(),
            draft: None,
            local_body: None,
            local_folder: None,
            trashed: None,
        }
    }

    fn retained(&self) -> bool {
        self.draft.is_some() || self.local_body.is_some() || self.trashed.is_some()
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct Ledger {
    version: u32,
    pub revision: u64,
    pub account: Option<Account>,
    pub cursor: Option<Cursor>,
    pub activity: Vec<Activity>,
    #[serde(default)]
    pub restored_uids: Vec<(u32, u32)>,
    retired_secrets: Vec<String>,
    last_status: Option<String>,
    #[serde(default)]
    classification_failure: Option<(String, u32)>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            version: VERSION,
            revision: 0,
            account: None,
            cursor: None,
            activity: Vec::new(),
            restored_uids: Vec::new(),
            retired_secrets: Vec::new(),
            last_status: None,
            classification_failure: None,
        }
    }
}

impl Ledger {
    pub fn account(&self) -> Result<&Account> {
        self.account
            .as_ref()
            .context("Configure the dedicated mailbox first")
    }

    pub fn account_mut(&mut self) -> Result<&mut Account> {
        self.account
            .as_mut()
            .context("Configure the dedicated mailbox first")
    }

    pub fn activity_mut(&mut self, id: &str) -> Result<&mut Activity> {
        self.activity
            .iter_mut()
            .find(|item| item.id == id)
            .context("This mail activity is no longer available")
    }

    pub fn check_revision(&self, revision: u64) -> Result<()> {
        ensure!(
            self.revision == revision,
            "Email settings changed. Reload and try again"
        );
        Ok(())
    }

    pub fn changed(&mut self, store: &MailStore) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .context("Invalid email settings revision")?;
        store.write(self)
    }

    pub fn note(&mut self, status: impl Into<String>) {
        self.last_status = Some(status.into());
    }

    pub fn record_connection_test(
        &mut self,
        store: &MailStore,
        inbox: Result<()>,
        smtp: Result<()>,
    ) -> Result<()> {
        let account = self.account_mut()?;
        account.tested = inbox.is_ok();
        account.smtp_tested = Some(smtp.is_ok());
        account.enabled &= account.tested;
        let status = match (inbox, smtp) {
            (Ok(()), Ok(())) => "Incoming mail and SMTP connections verified".into(),
            (Ok(()), Err(error)) => format!("Inbox connection verified. {error}."),
            (Err(error), Ok(())) => format!("{error}. SMTP connection verified."),
            (Err(inbox), Err(smtp)) => format!("{inbox}. {smtp}."),
        };
        self.note(status);
        self.changed(store)
    }

    pub fn add(&mut self, activity: Activity) {
        self.activity.insert(0, activity);
        let mut recent = 0;
        self.activity.retain(|entry| {
            if entry.retained() {
                return true;
            }
            recent += 1;
            recent <= RECENT_ACTIVITY
        });
    }

    pub fn ensure_retention_space(&self) -> Result<()> {
        ensure!(
            self.activity.iter().filter(|item| item.retained()).count() < 120,
            "Email storage is full. Review drafts or discard local copies"
        );
        Ok(())
    }

    pub fn snapshot(&self) -> Result<serde_json::Value> {
        let account = self.account.as_ref().map(|account| {
            serde_json::json!({
                "settings": account.settings,
                "enabled": account.enabled,
                "tested": account.tested,
                "smtpTested": account.smtp_tested(),
                "hasCredentials": account.secret_id.is_some(),
            })
        });
        Ok(serde_json::json!({
            "revision": self.revision,
            "account": account,
            "activity": self.activity,
            "lastStatus": self.last_status,
        }))
    }

    /// Records a failed classification of `message` and reports whether it should be retried on
    /// the next poll. Once the attempts are exhausted the message is released so one message the
    /// model cannot handle does not block the rest of the inbox indefinitely.
    pub fn retry_classification(&mut self, message: &str) -> bool {
        let attempts = match &self.classification_failure {
            Some((failed, attempts)) if failed == message => attempts + 1,
            _ => 1,
        };
        if attempts < CLASSIFICATION_ATTEMPTS {
            self.classification_failure = Some((message.to_owned(), attempts));
            true
        } else {
            self.classification_failure = None;
            false
        }
    }

    pub fn classification_succeeded(&mut self) {
        self.classification_failure = None;
    }

    pub fn save_settings(
        &mut self,
        store: &MailStore,
        settings: Settings,
        incoming: String,
        outgoing: String,
    ) -> Result<()> {
        let previous = self.account.as_ref();
        let same_identity = previous.is_some_and(|before| before.settings.same_identity(&settings));
        let original_key = previous.and_then(|account| account.secret_id.clone());
        let replacement = !incoming.is_empty() || !outgoing.is_empty();
        if replacement {
            ensure!(
                !incoming.is_empty() && !outgoing.is_empty(),
                "Enter both mail passwords"
            );
            ensure!(
                [&incoming, &outgoing]
                    .iter()
                    .all(|value| super::valid_line(value, 2048)),
                "Invalid mail password"
            );
        }
        ensure!(
            replacement || (same_identity && original_key.is_some()),
            "Enter credentials for the configured mail servers"
        );
        if !same_identity && previous.is_some() {
            ensure!(!self.activity.iter().any(Activity::retained), "Review or clear local mail, drafts, and Trash records before changing mailbox identity");
        } else if previous
            .is_some_and(|old| old.settings.include_existing != settings.include_existing)
        {
            ensure!(
                self.cursor.is_none(),
                "The initial inbox selection is already saved"
            );
        }

        let new_key = if replacement {
            let key = Uuid::new_v4().to_string();
            self.retired_secrets.push(key.clone());
            store.write(self)?;
            store.save_passwords(&key, &Passwords { incoming, outgoing })?;
            self.retired_secrets.pop();
            if let Some(old) = original_key {
                self.retired_secrets.push(old);
            }
            Some(key)
        } else {
            original_key
        };
        let tested =
            same_identity && !replacement && previous.is_some_and(|account| account.tested);
        let smtp_tested =
            same_identity && !replacement && previous.is_some_and(Account::smtp_tested);
        let enabled = tested
            && previous.is_some_and(|account| account.enabled)
            && previous.is_some_and(|account| {
                account.settings.provider == settings.provider
                    && account.settings.model == settings.model
            });
        if !same_identity {
            self.cursor = None;
            self.restored_uids.clear();
            self.activity.clear();
        }
        self.account = Some(Account {
            settings,
            secret_id: new_key,
            enabled,
            tested,
            smtp_tested: Some(smtp_tested),
        });
        self.changed(store)?;
        self.clean_retired(store);
        Ok(())
    }

    pub fn clean_retired(&mut self, store: &MailStore) {
        if self.retired_secrets.is_empty() {
            return;
        }
        self.retired_secrets
            .retain(|key| store.delete_passwords(key).is_err());
        if !self.retired_secrets.is_empty() {
            self.note(
                "Some previous credentials could not be removed from the system credential store",
            );
        }
        let _ = store.write(self);
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct Passwords {
    pub incoming: String,
    pub outgoing: String,
}

pub(super) struct MailStore {
    dir: PathBuf,
    _lock: File,
}

impl MailStore {
    pub fn open(dir: PathBuf) -> Result<(Self, Ledger)> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&dir)
            .context("Could not create local email storage")?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("worker.lock"))
            .context("Could not open the email worker lock")?;
        fs2::FileExt::try_lock_exclusive(&lock).context("An email operation is already running")?;
        let store = Self { dir, _lock: lock };
        let mut state = store.read()?;
        state.clean_retired(&store);
        Ok((store, state))
    }

    pub fn read(&self) -> Result<Ledger> {
        let state: Ledger = match fs::read(self.dir.join("mail.json")) {
            Ok(contents) => {
                serde_json::from_slice(&contents).context("Could not decode local email data")?
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ledger::default(),
            Err(error) => return Err(error).context("Could not read local email data"),
        };
        ensure!(
            state.version == VERSION,
            "Unsupported local email data version"
        );
        Ok(state)
    }

    pub fn write(&self, state: &Ledger) -> Result<()> {
        let mut temp = tempfile::NamedTempFile::new_in(&self.dir)?;
        serde_json::to_writer(&mut temp, state)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        temp.persist(self.dir.join("mail.json"))
            .context("Could not save local email data")?;
        #[cfg(unix)]
        File::open(&self.dir)?
            .sync_all()
            .context("Could not sync local email storage")?;
        Ok(())
    }

    pub fn passwords(&self, account: &Account) -> Result<Passwords> {
        let id = account
            .secret_id
            .as_deref()
            .context("Save mailbox credentials first")?;
        #[cfg(feature = "system-keyring")]
        {
            let encoded = self.keyring(id)?.get_password().map_err(|_| {
                anyhow::anyhow!("Email credentials are unavailable in the system credential store")
            })?;
            serde_json::from_str(&encoded).context("Could not decode saved email credentials")
        }
        #[cfg(not(feature = "system-keyring"))]
        {
            let _ = id;
            bail!("This Gosling build does not include the system credential store");
        }
    }

    pub fn save_passwords(&self, id: &str, passwords: &Passwords) -> Result<()> {
        #[cfg(feature = "system-keyring")]
        {
            let value = serde_json::to_string(passwords)?;
            self.keyring(id)?.set_password(&value).map_err(|_| {
                anyhow::anyhow!("Could not save email credentials in the system credential store")
            })
        }
        #[cfg(not(feature = "system-keyring"))]
        {
            let _ = (id, passwords);
            bail!("This Gosling build does not include the system credential store");
        }
    }

    pub fn delete_passwords(&self, id: &str) -> Result<()> {
        #[cfg(feature = "system-keyring")]
        match self.keyring(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => bail!("Could not remove email credentials from the system credential store"),
        }
        #[cfg(not(feature = "system-keyring"))]
        {
            let _ = id;
            bail!("This Gosling build does not include the system credential store");
        }
    }

    #[cfg(feature = "system-keyring")]
    fn keyring(&self, id: &str) -> Result<keyring::Entry> {
        ensure!(
            std::env::var_os("GOSLING_DISABLE_KEYRING").is_none(),
            "The system credential store is disabled"
        );
        ensure!(
            id.parse::<Uuid>().is_ok(),
            "Invalid email credential reference"
        );
        keyring::Entry::new("gosling-dedicated-email", id)
            .map_err(|_| anyhow::anyhow!("System credential store is unavailable"))
    }
}
