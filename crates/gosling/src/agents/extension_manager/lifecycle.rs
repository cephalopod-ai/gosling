// Owns extension registration replacement, shutdown, queries, and working-root updates.
// The multi-transport add_extension constructor remains in the compatibility facade.
// ExtensionManager callers retain every original public lifecycle method.

use super::*;

impl ExtensionManager {
    pub async fn add_client(
        &self,
        name: String,
        config: ExtensionConfig,
        client: McpClientBox,
        info: Option<ServerInfo>,
        temp_dir: Option<TempDir>,
    ) {
        let normalized = name_to_key(&name);
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let replaced = self.extensions.lock().await.remove(&normalized);
        if let Some(replaced) = replaced {
            replaced.shutdown().await;
        }
        self.extensions.lock().await.insert(
            normalized,
            Extension::new(config.clone(), config.clone(), client, info, temp_dir, None),
        );
        self.invalidate_tools_cache_and_bump_version().await;
    }

    /// Get extensions info for building the system prompt
    pub async fn get_extensions_info(&self, working_dir: &std::path::Path) -> Vec<ExtensionInfo> {
        let working_dir_str = working_dir.to_string_lossy();
        self.extensions
            .lock()
            .await
            .iter()
            .map(|(name, ext)| {
                let instructions = ext.get_instructions().unwrap_or_default();
                let instructions = instructions.replace("{{WORKING_DIR}}", &working_dir_str);
                ExtensionInfo::new(name, &instructions, ext.supports_resources())
            })
            .collect()
    }

    /// Get aggregated usage statistics
    pub async fn remove_extension(&self, name: &str) -> ExtensionResult<()> {
        let sanitized_name = name_to_key(name);
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let removed = self.extensions.lock().await.remove(&sanitized_name);
        if let Some(removed) = removed {
            removed.shutdown().await;
        }
        self.runtime_blocked_extensions
            .lock()
            .await
            .remove(&sanitized_name);
        self.invalidate_tools_cache_and_bump_version().await;
        Ok(())
    }

    pub async fn shutdown(&self) {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let extensions = std::mem::take(&mut *self.extensions.lock().await);
        self.runtime_blocked_extensions.lock().await.clear();

        let mut shutdowns = FuturesUnordered::new();
        for extension in extensions.into_values() {
            shutdowns.push(extension.shutdown());
        }
        while shutdowns.next().await.is_some() {}

        self.invalidate_tools_cache_and_bump_version().await;
    }

    pub async fn update_working_dirs(
        &self,
        primary: &std::path::Path,
        additional: &[std::path::PathBuf],
    ) -> ExtensionResult<()> {
        let extensions: Vec<_> = self
            .extensions
            .lock()
            .await
            .iter()
            .map(|(name, extension)| (name.clone(), extension.get_client()))
            .collect();
        let mut failures = Vec::new();
        for (name, client) in extensions {
            if let Err(e) = client
                .update_working_dirs(primary.to_path_buf(), additional.to_vec())
                .await
            {
                failures.push(format!("{name}: {e}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(ExtensionError::SetupError(format!(
                "failed to update extension roots: {}",
                failures.join("; ")
            )))
        }
    }

    pub async fn get_extension_and_tool_counts(&self, session_id: &str) -> (usize, usize) {
        let enabled_extensions_count = self.extensions.lock().await.len();

        let total_tools = self
            .get_prefixed_tools(session_id, None)
            .await
            .map(|tools| tools.len())
            .unwrap_or(0);

        (enabled_extensions_count, total_tools)
    }

    pub async fn list_extensions(&self) -> ExtensionResult<Vec<String>> {
        Ok(self.extensions.lock().await.keys().cloned().collect())
    }

    pub async fn is_extension_enabled(&self, name: &str) -> bool {
        let normalized = name_to_key(name);
        self.extensions.lock().await.contains_key(&normalized)
    }

    pub async fn get_extension_configs(&self) -> Vec<ExtensionConfig> {
        self.extensions
            .lock()
            .await
            .values()
            .map(|ext| ext.config.clone())
            .collect()
    }

    pub async fn get_extension_configs_for_persistence(&self) -> Vec<ExtensionConfig> {
        let mut configs = self.get_extension_configs().await;
        configs.extend(
            self.runtime_blocked_extensions
                .lock()
                .await
                .values()
                .cloned(),
        );
        configs
    }

    pub fn is_code_execution_runtime_enabled(&self) -> bool {
        self.code_execution_runtime.is_enabled()
    }
}
