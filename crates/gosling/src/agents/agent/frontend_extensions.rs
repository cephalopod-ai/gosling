//! Frontend extension, tool, instruction, and container-derived agent state.
//!
//! Maintainers: keep deterministic frontend projections and persistence inputs together.
//! Clients: frontend tool discovery and extension counts remain stable.

use super::*;

impl Agent {
    /// When set, all stdio extensions will be started via `docker exec` in the specified container.
    pub async fn set_container(&self, container: Option<Container>) {
        *self.container.lock().await = container.clone();
    }

    pub async fn container(&self) -> Option<Container> {
        self.container.lock().await.clone()
    }

    /// Check if a tool is a frontend tool
    pub async fn is_frontend_tool(&self, name: &str) -> bool {
        self.frontend_tools.lock().await.contains_key(name)
    }

    /// Get a reference to a frontend tool
    pub async fn get_frontend_tool(&self, name: &str) -> Option<FrontendTool> {
        self.frontend_tools.lock().await.get(name).cloned()
    }

    pub(super) async fn frontend_extension_configs(&self) -> Vec<ExtensionConfig> {
        let mut configs = self
            .frontend_extensions
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        configs.sort_by_key(|config| config.key());
        configs
    }

    pub(super) async fn frontend_tools_for_extension(
        &self,
        extension_name: Option<&str>,
    ) -> Vec<Tool> {
        let requested_extension = extension_name.map(name_to_key);

        self.frontend_extension_configs()
            .await
            .into_iter()
            .filter_map(|config| {
                let include = requested_extension
                    .as_ref()
                    .is_none_or(|name| *name == config.key());

                match config {
                    ExtensionConfig::Frontend { tools, .. } if include => Some(tools),
                    _ => None,
                }
            })
            .flatten()
            .collect()
    }

    async fn rebuild_frontend_derived_state(&self, extensions: &HashMap<String, ExtensionConfig>) {
        let multiple = extensions.len() > 1;
        let mut tools = HashMap::new();
        let mut instructions = Vec::new();

        for config in extensions.values() {
            if let ExtensionConfig::Frontend {
                name,
                tools: ext_tools,
                instructions: ext_instructions,
                ..
            } = config
            {
                for tool in ext_tools {
                    let tool_name = tool.name.to_string();
                    tools.insert(
                        tool_name.clone(),
                        FrontendTool {
                            name: tool_name,
                            tool: tool.clone(),
                        },
                    );
                }

                let text = ext_instructions
                    .clone()
                    .unwrap_or_else(|| DEFAULT_FRONTEND_INSTRUCTIONS.to_string());
                instructions.push(if multiple {
                    format!("{name}: {text}")
                } else {
                    text
                });
            }
        }

        *self.frontend_tools.lock().await = tools;
        *self.frontend_instructions.lock().await = if instructions.is_empty() {
            None
        } else {
            Some(instructions.join("\n\n"))
        };
    }

    pub(super) async fn insert_frontend_extension(&self, extension: ExtensionConfig) {
        let mut extensions = self.frontend_extensions.lock().await;
        extensions.insert(extension.key(), extension);
        self.rebuild_frontend_derived_state(&extensions).await;
    }

    pub(super) async fn remove_frontend_extension(&self, name: &str) {
        let mut extensions = self.frontend_extensions.lock().await;
        extensions.remove(&name_to_key(name));
        self.rebuild_frontend_derived_state(&extensions).await;
    }

    pub(super) async fn extension_configs_for_persistence(&self) -> Vec<ExtensionConfig> {
        let mut extension_configs = self
            .extension_manager
            .get_extension_configs_for_persistence()
            .await;
        extension_configs.extend(self.frontend_extension_configs().await);
        extension_configs
    }

    pub(crate) async fn total_extension_and_tool_counts(&self, session_id: &str) -> (usize, usize) {
        let (extension_count, tool_count) = self
            .extension_manager
            .get_extension_and_tool_counts(session_id)
            .await;

        (
            extension_count + self.frontend_extensions.lock().await.len(),
            tool_count + self.frontend_tools.lock().await.len(),
        )
    }
}
