use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use gosling::config::paths::Paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Structure to track project information
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// The absolute path to the project directory
    pub path: String,
    /// Last time the project was accessed
    pub last_accessed: DateTime<Utc>,
    /// Last instruction sent to gosling (if available)
    pub last_instruction: Option<String>,
    /// Last session ID associated with this project
    pub last_session_id: Option<String>,
}

/// Structure to hold all tracked projects
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectTracker {
    projects: HashMap<String, ProjectInfo>,
}

/// Project information with path as a separate field for easier access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfoDisplay {
    /// The absolute path to the project directory
    pub path: String,
    /// Last time the project was accessed
    pub last_accessed: DateTime<Utc>,
    /// Last instruction sent to gosling (if available)
    pub last_instruction: Option<String>,
    /// Last session ID associated with this project
    pub last_session_id: Option<String>,
}

impl ProjectTracker {
    fn get_projects_file() -> Result<PathBuf> {
        let projects_file = Paths::in_data_dir("projects.json");
        ensure_parent(&projects_file)?;
        Ok(projects_file)
    }

    pub fn load() -> Result<Self> {
        let projects_file = Self::get_projects_file()?;
        Self::load_from_path(&projects_file)
    }

    pub fn save(&self) -> Result<()> {
        let projects_file = Self::get_projects_file()?;
        let lock = open_lock(&projects_file)?;
        lock.lock_exclusive()?;
        let result = write_tracker(&projects_file, self);
        FileExt::unlock(&lock)?;
        result
    }

    /// Update project information for the current directory
    ///
    /// # Arguments
    /// * `project_dir` - The project directory to update
    /// * `instruction` - Optional instruction that was sent to gosling
    /// * `session_id` - Optional session ID associated with this project
    pub fn update_project(
        &mut self,
        project_dir: &Path,
        instruction: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<()> {
        let projects_file = Self::get_projects_file()?;
        *self = Self::update_path(&projects_file, project_dir, instruction, session_id)?;
        Ok(())
    }

    /// List all tracked projects
    ///
    /// Returns a vector of ProjectInfoDisplay objects
    pub fn list_projects(&self) -> Vec<ProjectInfoDisplay> {
        self.projects
            .values()
            .map(|info| ProjectInfoDisplay {
                path: info.path.clone(),
                last_accessed: info.last_accessed,
                last_instruction: info.last_instruction.clone(),
                last_session_id: info.last_session_id.clone(),
            })
            .collect()
    }

    fn load_from_path(projects_file: &Path) -> Result<Self> {
        ensure_parent(projects_file)?;
        let lock = open_lock(projects_file)?;
        lock.lock_shared()?;
        let result = read_tracker(projects_file);
        FileExt::unlock(&lock)?;
        result
    }

    fn update_path(
        projects_file: &Path,
        project_dir: &Path,
        instruction: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Self> {
        ensure_parent(projects_file)?;
        let lock = open_lock(projects_file)?;
        lock.lock_exclusive()?;
        let result = (|| {
            let mut tracker = read_tracker(projects_file)?;
            tracker.update_in_memory(project_dir, instruction, session_id);
            write_tracker(projects_file, &tracker)?;
            Ok(tracker)
        })();
        FileExt::unlock(&lock)?;
        result
    }

    fn update_in_memory(
        &mut self,
        project_dir: &Path,
        instruction: Option<&str>,
        session_id: Option<&str>,
    ) {
        let dir_str = project_dir.to_string_lossy().to_string();
        let project_info = self.projects.entry(dir_str.clone()).or_insert(ProjectInfo {
            path: dir_str,
            last_accessed: Utc::now(),
            last_instruction: None,
            last_session_id: None,
        });
        project_info.last_accessed = Utc::now();
        if let Some(instruction) = instruction {
            project_info.last_instruction = Some(instruction.to_string());
        }
        if let Some(session_id) = session_id {
            project_info.last_session_id = Some(session_id.to_string());
        }
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn open_lock(projects_file: &Path) -> Result<File> {
    let lock_path = projects_file.with_extension("json.lock");
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&lock_path)?;
    set_private_permissions(&lock_path)?;
    Ok(file)
}

fn read_tracker(projects_file: &Path) -> Result<ProjectTracker> {
    if !projects_file.exists() {
        return Ok(ProjectTracker {
            projects: HashMap::new(),
        });
    }
    let file_content = fs::read_to_string(projects_file)?;
    serde_json::from_str(&file_content).context("Failed to parse projects.json file")
}

fn write_tracker(projects_file: &Path, tracker: &ProjectTracker) -> Result<()> {
    let parent = projects_file
        .parent()
        .ok_or_else(|| anyhow::anyhow!("projects.json has no parent directory"))?;
    let mut json = serde_json::to_vec_pretty(tracker)?;
    json.push(b'\n');
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&json)?;
    temp.as_file().sync_all()?;
    temp.persist(projects_file).map_err(|error| error.error)?;
    set_private_permissions(projects_file)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn set_private_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::set_permissions(
        path,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
    )?;
    Ok(())
}

/// Update the project tracker with the current directory and optional instruction
///
/// # Arguments
/// * `instruction` - Optional instruction that was sent to gosling
/// * `session_id` - Optional session ID associated with this project
pub fn update_project_tracker(instruction: Option<&str>, session_id: Option<&str>) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let projects_file = ProjectTracker::get_projects_file()?;
    ProjectTracker::update_path(&projects_file, &current_dir, instruction, session_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_updates_preserve_every_project_and_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let projects_file = dir.path().join("projects.json");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles = (0..8)
            .map(|index| {
                let projects_file = projects_file.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    ProjectTracker::update_path(
                        &projects_file,
                        &PathBuf::from(format!("/tmp/project-{index}")),
                        Some(&format!("instruction-{index}")),
                        Some(&format!("session-{index}")),
                    )
                    .unwrap();
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }

        let tracker = ProjectTracker::load_from_path(&projects_file).unwrap();
        assert_eq!(tracker.projects.len(), 8);
        serde_json::from_slice::<serde_json::Value>(&fs::read(&projects_file).unwrap()).unwrap();
        for index in 0..8 {
            let project = tracker
                .projects
                .get(&format!("/tmp/project-{index}"))
                .unwrap();
            assert_eq!(
                project.last_session_id.as_deref(),
                Some(format!("session-{index}").as_str())
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn tracker_and_lock_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let projects_file = dir.path().join("projects.json");
        ProjectTracker::update_path(&projects_file, Path::new("/tmp/project"), None, None).unwrap();

        assert_eq!(
            fs::metadata(&projects_file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(projects_file.with_extension("json.lock"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
