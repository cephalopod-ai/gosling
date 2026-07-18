use super::{CredentialProfile, ProductOutputFolder, ProductType, Workspace};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STORE_SCHEMA_VERSION: u32 = 1;
const WORKSPACE_DIRECTORY: &str = "workspaces";
const STORE_FILE: &str = "workspaces.json";
const TEMP_FILE: &str = ".workspaces.json.tmp";
const LOCK_FILE: &str = ".workspaces.lock";
const CREDENTIAL_LOCK_FILE: &str = ".credential-transaction.lock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceStoreDocument {
    pub schema_version: u32,
    pub active_workspace_id: String,
    pub default_workspace_id: String,
    #[serde(default)]
    pub migration_completed: bool,
    #[serde(default)]
    pub templates_materialized: bool,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub credential_profiles: Vec<CredentialProfile>,
    #[serde(default)]
    pub distribution_profile_secret_fields: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub workspace_profile_required_secret_fields: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub pending_secret_deletions: Vec<String>,
    #[serde(flatten)]
    pub unknown_fields: BTreeMap<String, Value>,
}

impl WorkspaceStoreDocument {
    fn create_default(working_folder: &Path) -> Self {
        let now = Utc::now().to_rfc3339();
        let workspace_id = Uuid::now_v7().to_string();
        let output_path = working_folder.join("Outputs");
        let workspace = Workspace {
            id: workspace_id.clone(),
            schema_version: STORE_SCHEMA_VERSION,
            name: "Default".to_string(),
            working_folder: working_folder.to_string_lossy().to_string(),
            product_output_folders: vec![ProductOutputFolder {
                id: Uuid::now_v7().to_string(),
                label: "Outputs".to_string(),
                path: output_path.to_string_lossy().to_string(),
                product_types: vec![
                    ProductType::Document,
                    ProductType::Spreadsheet,
                    ProductType::Presentation,
                    ProductType::Image,
                    ProductType::Video,
                    ProductType::Code,
                    ProductType::Data,
                    ProductType::Export,
                    ProductType::Other,
                ],
                is_default: true,
                create_if_missing: true,
            }],
            created_at: now.clone(),
            updated_at: now.clone(),
            last_opened_at: now,
            ..Workspace::default()
        };
        Self {
            schema_version: STORE_SCHEMA_VERSION,
            active_workspace_id: workspace_id.clone(),
            default_workspace_id: workspace_id,
            migration_completed: false,
            templates_materialized: false,
            workspaces: vec![workspace],
            credential_profiles: Vec::new(),
            distribution_profile_secret_fields: BTreeMap::new(),
            workspace_profile_required_secret_fields: BTreeMap::new(),
            pending_secret_deletions: Vec::new(),
            unknown_fields: BTreeMap::new(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != STORE_SCHEMA_VERSION {
            return Err(anyhow!(
                "workspace store schema {} is newer than supported schema {}",
                self.schema_version,
                STORE_SCHEMA_VERSION
            ));
        }
        if self.workspaces.is_empty() {
            return Err(anyhow!("workspace store contains no workspaces"));
        }
        if !self
            .workspaces
            .iter()
            .any(|workspace| workspace.id == self.default_workspace_id)
        {
            return Err(anyhow!("default workspace does not exist"));
        }
        if !self
            .workspaces
            .iter()
            .any(|workspace| workspace.id == self.active_workspace_id)
        {
            return Err(anyhow!("active workspace does not exist"));
        }
        ensure_unique(
            self.workspaces
                .iter()
                .map(|workspace| workspace.id.as_str()),
            "workspace IDs",
        )?;
        ensure_unique(
            self.workspaces
                .iter()
                .map(|workspace| workspace.name.to_lowercase()),
            "workspace names",
        )?;
        ensure_unique(
            self.credential_profiles
                .iter()
                .map(|profile| profile.id.as_str()),
            "credential profile IDs",
        )?;
        ensure_unique(
            self.credential_profiles
                .iter()
                .map(|profile| profile.name.to_lowercase()),
            "credential profile names",
        )?;
        for workspace in &self.workspaces {
            if workspace.schema_version != super::WORKSPACE_SCHEMA_VERSION {
                return Err(anyhow!("workspace has an unsupported schema version"));
            }
            super::service::validate_workspace_boundary(&super::WorkspaceMutation::from(
                workspace,
            ))?;
        }
        for profile_id in self
            .distribution_profile_secret_fields
            .keys()
            .chain(self.workspace_profile_required_secret_fields.keys())
        {
            if !self
                .credential_profiles
                .iter()
                .any(|profile| &profile.id == profile_id)
            {
                return Err(anyhow!(
                    "credential requirements reference a missing profile"
                ));
            }
        }
        ensure_unique(
            self.pending_secret_deletions.iter().map(String::as_str),
            "pending secret deletion keys",
        )?;
        super::service::reject_secret_shaped_value(&Value::Object(
            self.unknown_fields.clone().into_iter().collect(),
        ))?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn create_default_for_test() -> Self {
        Self::create_default(Path::new("/tmp"))
    }
}

pub(crate) struct WorkspaceStore {
    directory: PathBuf,
    path: PathBuf,
    temp_path: PathBuf,
    lock_path: PathBuf,
}

impl WorkspaceStore {
    pub(crate) fn new(data_dir: &Path) -> Self {
        let directory = data_dir.join(WORKSPACE_DIRECTORY);
        Self {
            path: directory.join(STORE_FILE),
            temp_path: directory.join(TEMP_FILE),
            lock_path: directory.join(LOCK_FILE),
            directory,
        }
    }

    pub(crate) fn load_or_initialize(
        &self,
        working_folder: &Path,
    ) -> Result<WorkspaceStoreDocument> {
        self.ensure_private_directory()?;
        let lock = self.open_lock()?;
        lock.lock_exclusive()?;
        let result = if self.path.exists() {
            match self.read_document(&self.path) {
                Ok(document) => Ok(document),
                Err(_error) if self.is_recoverable_malformed_store(&self.path)? => {
                    self.recover_malformed_store(working_folder)
                }
                Err(error) => Err(error),
            }
        } else if self.temp_path.exists() {
            let document = self.read_document(&self.temp_path)?;
            fs::rename(&self.temp_path, &self.path)?;
            self.sync_directory()?;
            Ok(document)
        } else {
            let document = WorkspaceStoreDocument::create_default(working_folder);
            self.write_document(&document)?;
            Ok(document)
        };
        FileExt::unlock(&lock)?;
        result
    }

    pub(crate) fn load(&self) -> Result<WorkspaceStoreDocument> {
        self.ensure_private_directory()?;
        let lock = self.open_lock()?;
        lock.lock_shared()?;
        let result = self.read_document(&self.path);
        FileExt::unlock(&lock)?;
        result
    }

    pub(crate) fn mutate<T>(
        &self,
        mutate: impl FnOnce(&mut WorkspaceStoreDocument) -> Result<T>,
    ) -> Result<T> {
        self.ensure_private_directory()?;
        let lock = self.open_lock()?;
        lock.lock_exclusive()?;
        let result = (|| {
            let mut document = self.read_document(&self.path)?;
            let output = mutate(&mut document)?;
            document.validate()?;
            self.write_document(&document)?;
            Ok(output)
        })();
        FileExt::unlock(&lock)?;
        result
    }

    pub(crate) fn lock_credential_transaction(&self) -> Result<File> {
        self.ensure_private_directory()?;
        let path = self.directory.join(CREDENTIAL_LOCK_FILE);
        let file = private_create(&path)?;
        set_private_file_permissions(&path)?;
        file.lock_exclusive()?;
        Ok(file)
    }

    fn read_document(&self, path: &Path) -> Result<WorkspaceStoreDocument> {
        let bytes = fs::read(path)
            .with_context(|| format!("could not read workspace store {}", path.display()))?;
        let document: WorkspaceStoreDocument = serde_json::from_slice(&bytes)
            .with_context(|| format!("workspace store {} is malformed", path.display()))?;
        document.validate()?;
        Ok(document)
    }

    fn is_recoverable_malformed_store(&self, path: &Path) -> Result<bool> {
        let bytes = fs::read(path)?;
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return Ok(true);
        };
        Ok(value
            .get("schema_version")
            .and_then(Value::as_u64)
            .is_none_or(|version| version <= u64::from(STORE_SCHEMA_VERSION)))
    }

    fn recover_malformed_store(&self, working_folder: &Path) -> Result<WorkspaceStoreDocument> {
        let backup = self.directory.join(format!(
            "workspaces.corrupt-{}.json",
            Uuid::now_v7().as_simple()
        ));
        fs::rename(&self.path, &backup)?;
        set_private_file_permissions(&backup)?;
        tracing::warn!(
            backup_path = %backup.display(),
            "Malformed workspace store was preserved and recovered"
        );

        if self.temp_path.exists() {
            if let Ok(document) = self.read_document(&self.temp_path) {
                fs::rename(&self.temp_path, &self.path)?;
                set_private_file_permissions(&self.path)?;
                self.sync_directory()?;
                return Ok(document);
            }
        }

        let document = WorkspaceStoreDocument::create_default(working_folder);
        self.write_document(&document)?;
        Ok(document)
    }

    fn write_document(&self, document: &WorkspaceStoreDocument) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(document)?;
        bytes.push(b'\n');
        let mut file = private_create_truncated(&self.temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&self.temp_path, &self.path)?;
        set_private_file_permissions(&self.path)?;
        self.sync_directory()?;
        Ok(())
    }

    fn ensure_private_directory(&self) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        #[cfg(unix)]
        fs::set_permissions(
            &self.directory,
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )?;
        Ok(())
    }

    fn open_lock(&self) -> Result<File> {
        let file = private_create(&self.lock_path)?;
        set_private_file_permissions(&self.lock_path)?;
        Ok(file)
    }

    fn sync_directory(&self) -> Result<()> {
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

fn private_create(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn private_create_truncated(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn ensure_unique<T>(values: impl IntoIterator<Item = T>, label: &str) -> Result<()>
where
    T: Eq + std::hash::Hash,
{
    let mut seen = HashSet::new();
    if values.into_iter().any(|value| !seen.insert(value)) {
        return Err(anyhow!("workspace store contains duplicate {label}"));
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::set_permissions(
        path,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_and_round_trips_default_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());

        let initialized = store.load_or_initialize(working.path()).unwrap();
        assert_eq!(initialized.workspaces.len(), 1);
        assert_eq!(initialized.workspaces[0].name, "Default");
        assert_eq!(
            store.load().unwrap().active_workspace_id,
            initialized.active_workspace_id
        );
    }

    #[test]
    fn malformed_store_is_backed_up_and_reinitialized() {
        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());
        store.load_or_initialize(working.path()).unwrap();
        fs::write(store.path(), b"{not-json").unwrap();

        let recovered = store.load_or_initialize(working.path()).unwrap();
        assert_eq!(recovered.workspaces[0].name, "Default");
        let backup = fs::read_dir(store.path().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("workspaces.corrupt-")
            })
            .unwrap();
        assert_eq!(fs::read(backup).unwrap(), b"{not-json");
    }

    #[test]
    fn unknown_top_level_fields_survive_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());
        store.load_or_initialize(working.path()).unwrap();
        store
            .mutate(|document| {
                document
                    .unknown_fields
                    .insert("futureField".into(), serde_json::json!({"kept": true}));
                Ok(())
            })
            .unwrap();

        assert_eq!(
            store.load().unwrap().unknown_fields["futureField"],
            serde_json::json!({"kept": true})
        );
    }

    #[test]
    fn secret_shaped_unknown_fields_are_never_persisted() {
        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());
        store.load_or_initialize(working.path()).unwrap();

        let result = store.mutate(|document| {
            document.unknown_fields.insert(
                "api_key".into(),
                Value::String("GOSLING_SENTINEL_SECRET".into()),
            );
            Ok(())
        });

        assert!(result.is_err());
        assert!(!fs::read_to_string(store.path())
            .unwrap()
            .contains("GOSLING_SENTINEL_SECRET"));
    }

    #[test]
    fn concurrent_store_mutations_preserve_both_updates() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());
        store.load_or_initialize(working.path()).unwrap();
        let barrier = Arc::new(Barrier::new(2));

        let writers = ["futureOne", "futureTwo"].map(|field| {
            let data_dir = temp.path().to_path_buf();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let store = WorkspaceStore::new(&data_dir);
                barrier.wait();
                store
                    .mutate(|document| {
                        document
                            .unknown_fields
                            .insert(field.into(), serde_json::json!({"kept": true}));
                        Ok(())
                    })
                    .unwrap();
            })
        });

        for writer in writers {
            writer.join().unwrap();
        }
        let document = store.load().unwrap();
        assert_eq!(document.unknown_fields["futureOne"]["kept"], true);
        assert_eq!(document.unknown_fields["futureTwo"]["kept"], true);
    }

    #[test]
    fn stale_longer_temp_is_truncated_before_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());
        store.load_or_initialize(working.path()).unwrap();
        fs::write(&store.temp_path, vec![b'x'; 64 * 1024]).unwrap();

        store
            .mutate(|document| {
                document.active_workspace_id = document.default_workspace_id.clone();
                Ok(())
            })
            .unwrap();

        assert!(store.load().is_ok());
        assert!(!fs::read(store.path()).unwrap().ends_with(b"xxxx"));
    }

    #[test]
    fn duplicate_workspace_identity_is_quarantined_on_startup() {
        let temp = tempfile::tempdir().unwrap();
        let working = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(temp.path());
        let initialized = store.load_or_initialize(working.path()).unwrap();
        let mut corrupted = initialized.clone();
        corrupted.workspaces.push(initialized.workspaces[0].clone());
        fs::write(store.path(), serde_json::to_vec(&corrupted).unwrap()).unwrap();

        let recovered = store.load_or_initialize(working.path()).unwrap();

        assert_eq!(recovered.workspaces.len(), 1);
        assert!(fs::read_dir(store.path().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .any(|name| name.to_string_lossy().starts_with("workspaces.corrupt-")));
    }
}
