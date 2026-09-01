// Owns tool catalog filtering, cache fill/invalidation, and planning metadata.
// ExtensionManager retains its public methods while lifecycle and dispatch share bounded internals.
// The extension_manager compatibility facade preserves the original manager type and paths.

use super::*;

impl ExtensionManager {
    pub async fn get_prefixed_tools(
        &self,
        session_id: &str,
        extension_name: Option<String>,
    ) -> ExtensionResult<Vec<Tool>> {
        let all_tools = self.get_all_tools_cached(session_id).await?;
        Ok(self.filter_tools(&all_tools, extension_name.as_deref(), None))
    }

    pub async fn get_prefixed_tools_excluding(
        &self,
        session_id: &str,
        exclude: &str,
    ) -> ExtensionResult<Vec<Tool>> {
        let all_tools = self.get_all_tools_cached(session_id).await?;
        Ok(self.filter_tools(&all_tools, None, Some(exclude)))
    }

    fn filter_tools(
        &self,
        tools: &[Tool],
        extension_name: Option<&str>,
        exclude: Option<&str>,
    ) -> Vec<Tool> {
        let extension_name_normalized = extension_name.map(name_to_key);
        let exclude_normalized = exclude.map(name_to_key);

        tools
            .iter()
            .filter(|tool| {
                let tool_owner = get_tool_owner(tool)
                    .map(|s| name_to_key(&s))
                    .unwrap_or_else(|| tool.name.split("__").next().unwrap_or("").to_string());

                if let Some(ref excluded) = exclude_normalized {
                    if tool_owner == *excluded {
                        return false;
                    }
                }

                if let Some(ref name_filter) = extension_name_normalized {
                    tool_owner == *name_filter
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    pub(super) async fn get_all_tools_cached(
        &self,
        session_id: &str,
    ) -> ExtensionResult<Arc<Vec<Tool>>> {
        {
            let cache = self.tools_cache.lock().await;
            if let Some(ref tools) = *cache {
                return Ok(Arc::clone(tools));
            }
        }

        let version_before = self.tools_cache_version.load(Ordering::SeqCst);
        let tools = Arc::new(self.fetch_all_tools(session_id).await?);

        {
            let mut cache = self.tools_cache.lock().await;
            let version_after = self.tools_cache_version.load(Ordering::SeqCst);
            if version_after == version_before && cache.is_none() {
                *cache = Some(Arc::clone(&tools));
            }
        }

        Ok(tools)
    }

    pub(super) fn host_supports_mcp_apps(&self) -> bool {
        if let Some(host_info) = &self.capabilities.host_info {
            if host_info.explicit_extensions {
                return host_info.mcpui_enabled();
            }
        }

        self.capabilities.mcpui
    }

    pub(super) async fn hydrate_mcp_app_attachment(
        client: &McpClientBox,
        session_id: &str,
        resolved_tool: &ResolvedTool,
        cancellation_token: CancellationToken,
    ) -> Option<GoslingMcpAppToolAttachment> {
        let resource_uri = resolved_tool.resource_uri.clone()?;

        let mut attachment = GoslingMcpAppToolAttachment {
            tool_name: resolved_tool.tool_name.clone(),
            extension_name: resolved_tool.extension_name.clone(),
            resource_uri: resource_uri.clone(),
            tool_meta: resolved_tool.tool_meta.clone(),
            resource_result: None,
            read_error: None,
        };

        match client
            .read_resource(session_id, &resource_uri, cancellation_token)
            .await
        {
            Ok(resource_result) => {
                attachment.resource_result = serde_json::to_value(&resource_result).ok();
            }
            Err(error) => {
                attachment.read_error = Some(error.to_string());
            }
        }

        Some(attachment)
    }

    pub(super) async fn invalidate_tools_cache_and_bump_version(&self) {
        self.tools_cache_version.fetch_add(1, Ordering::SeqCst);
        *self.tools_cache.lock().await = None;
    }

    async fn fetch_all_tools(&self, session_id: &str) -> ExtensionResult<Vec<Tool>> {
        let clients: Vec<_> = self
            .extensions
            .lock()
            .await
            .iter()
            .map(|(name, ext)| (name.clone(), ext.config.clone(), ext.get_client()))
            .collect();

        let cancel_token = CancellationToken::default();
        let client_futures = clients.into_iter().map(|(name, config, client)| {
            let cancel_token = cancel_token.clone();
            let ext_name = name.clone();
            async move {
                let mut tools = Vec::new();
                let client_tools =
                    match collect_paginated_tools(&client, session_id, cancel_token).await {
                        Ok(tools) => tools,
                        Err(e) => {
                            warn!(extension = %ext_name, error = %e, "Failed to list tools");
                            return Err((name, e));
                        }
                    };

                let expose_unprefixed = is_unprefixed_extension(&config);

                for mut tool in client_tools {
                    if config.is_tool_available(&tool.name) {
                        let public_name = if expose_unprefixed {
                            tool.name.to_string()
                        } else {
                            format!("{}__{}", name, tool.name)
                        };

                        let mut meta_map =
                            tool.meta.as_ref().map(|m| m.0.clone()).unwrap_or_default();
                        meta_map.insert(
                            TOOL_EXTENSION_META_KEY.to_string(),
                            serde_json::Value::String(name.clone()),
                        );

                        tool.name = public_name.into();
                        tool.meta = Some(rmcp::model::Meta(meta_map));

                        tools.push(tool);
                    }
                }

                Ok((name, tools))
            }
        });

        let results = future::join_all(client_futures).await;
        let errors = results
            .iter()
            .filter_map(|result| result.as_ref().err().cloned())
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let detail = errors
                .into_iter()
                .map(|(name, error)| format!("{name}: {error}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ExtensionError::SetupError(format!(
                "Failed to enumerate extension tools: {detail}"
            )));
        }

        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut tools = Vec::new();
        for (ext_name, client_tools) in results.into_iter().flatten() {
            for tool in client_tools {
                let tool_name = tool.name.to_string();
                if seen_names.contains(&tool_name) {
                    warn!(
                        tool = %tool_name,
                        extension = %ext_name,
                        "Duplicate tool name - skipping"
                    );
                    continue;
                }
                seen_names.insert(tool_name);
                tools.push(tool);
            }
        }

        Ok(tools)
    }

    /// Get the extension prompt including client instructions
    pub async fn get_planning_prompt(&self, tools_info: Vec<ToolInfo>) -> String {
        let mut context: HashMap<&str, Value> = HashMap::new();
        context.insert(
            "tools",
            serde_json::to_value(tools_info).unwrap_or_default(),
        );

        prompt_template::render_template("plan.md", &context).unwrap_or_else(|e| {
            tracing::error!("Failed to render planning prompt: {e}");
            String::new()
        })
    }
}
