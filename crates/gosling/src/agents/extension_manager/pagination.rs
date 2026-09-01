// Owns bounded pagination for MCP tool, resource, and prompt discovery.
// ExtensionManager callers consume complete lists without knowing page mechanics.
// The extension_manager compatibility facade keeps these helpers private.

use super::*;

pub(super) const MAX_MCP_LIST_PAGES: usize = 100;
pub(super) const MAX_MCP_LIST_ITEMS: usize = 10_000;

#[derive(Default)]
pub(super) struct PaginationGuard {
    pages: usize,
    items: usize,
    cursors: HashSet<String>,
}

impl PaginationGuard {
    pub(super) fn record_page(
        &mut self,
        item_kind: &str,
        page_items: usize,
        next_cursor: Option<&str>,
    ) -> std::result::Result<(), String> {
        self.pages += 1;
        if self.pages > MAX_MCP_LIST_PAGES {
            return Err(format!(
                "MCP {item_kind} pagination exceeded {MAX_MCP_LIST_PAGES} pages"
            ));
        }

        self.items = self
            .items
            .checked_add(page_items)
            .ok_or_else(|| format!("MCP {item_kind} item count overflowed"))?;
        if self.items > MAX_MCP_LIST_ITEMS {
            return Err(format!(
                "MCP {item_kind} pagination exceeded {MAX_MCP_LIST_ITEMS} items"
            ));
        }

        if let Some(cursor) = next_cursor {
            if !self.cursors.insert(cursor.to_string()) {
                return Err(format!(
                    "MCP {item_kind} pagination repeated cursor {cursor:?}"
                ));
            }
        }
        Ok(())
    }
}

pub(super) async fn collect_paginated_tools(
    client: &McpClientBox,
    session_id: &str,
    cancellation_token: CancellationToken,
) -> std::result::Result<Vec<Tool>, String> {
    let mut cursor = None;
    let mut guard = PaginationGuard::default();
    let mut tools = Vec::new();
    loop {
        let page = client
            .list_tools(session_id, cursor, cancellation_token.clone())
            .await
            .map_err(|error| format!("MCP list tools request failed: {error}"))?;
        guard.record_page("tool", page.tools.len(), page.next_cursor.as_deref())?;
        tools.extend(page.tools);
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok(tools),
        }
    }
}

pub(super) async fn collect_paginated_resources(
    client: &McpClientBox,
    session_id: &str,
    cancellation_token: CancellationToken,
) -> std::result::Result<Vec<Resource>, String> {
    let mut cursor = None;
    let mut guard = PaginationGuard::default();
    let mut resources = Vec::new();
    loop {
        let page = client
            .list_resources(session_id, cursor, cancellation_token.clone())
            .await
            .map_err(|error| format!("MCP list resources request failed: {error}"))?;
        guard.record_page(
            "resource",
            page.resources.len(),
            page.next_cursor.as_deref(),
        )?;
        resources.extend(page.resources);
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok(resources),
        }
    }
}

pub(super) async fn collect_paginated_prompts(
    client: &McpClientBox,
    session_id: &str,
    cancellation_token: CancellationToken,
) -> std::result::Result<Vec<Prompt>, String> {
    let mut cursor = None;
    let mut guard = PaginationGuard::default();
    let mut prompts = Vec::new();
    loop {
        let page = client
            .list_prompts(session_id, cursor, cancellation_token.clone())
            .await
            .map_err(|error| format!("MCP list prompts request failed: {error}"))?;
        guard.record_page("prompt", page.prompts.len(), page.next_cursor.as_deref())?;
        prompts.extend(page.prompts);
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok(prompts),
        }
    }
}
