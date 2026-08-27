//! Whole-session transfer: pretty-printed JSON export, cross-format import
//! (with untrusted-history marking and provenance recording), and same-store
//! copy — both import and copy commit session creation, metadata, and
//! conversation replacement atomically to avoid a stray partial session on
//! interruption.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! `pub(super)` matches these methods' pre-extraction (private, same-module)
//! visibility — the facade's `impl SessionManager` delegates to all three.
//! Fixed one path issue during extraction: the original facade's
//! `super::import_formats::...` referred to `crate::session::import_formats`
//! from session_manager's own level; moved one module deeper, `super::`
//! would instead resolve to `session_manager`, so these are now the
//! explicit `crate::session::import_formats::...` path (same fix already
//! applied in session_listing.rs for last_message_snippet).

use super::{Session, SessionManager, SessionStorage, SessionType};
use crate::config::GoslingMode;
use crate::conversation::Conversation;
use crate::session::extension_data::{EnabledExtensionsState, ExtensionState};
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};

impl SessionStorage {
    pub(super) async fn export_session(&self, id: &str) -> Result<String> {
        let session = self.get_session(id, true).await?;
        serde_json::to_string_pretty(&session).map_err(Into::into)
    }

    pub(super) async fn import_session(
        &self,
        session_manager: &SessionManager,
        json: &str,
        session_type_override: Option<SessionType>,
        working_dir: PathBuf,
        transport: crate::session::import_formats::SessionImportTransport,
        source: Option<(Option<&Path>, String)>,
    ) -> Result<Session> {
        let source_format = crate::session::import_formats::detect_format(json);
        let normalized = crate::session::import_formats::convert_to_gosling_session_json(json)?;
        let mut import: Session = serde_json::from_str(&normalized)?;
        let effective_working_dir =
            crate::session::import_formats::validate_import_working_dir(&working_dir)?;
        let original_working_dir = (!import.working_dir.as_os_str().is_empty())
            .then(|| import.working_dir.to_string_lossy().to_string());
        let mut extension_data = import.extension_data.clone();
        extension_data.remove_extension_state(
            EnabledExtensionsState::EXTENSION_NAME,
            EnabledExtensionsState::VERSION,
        );
        crate::session::import_formats::SessionImportProvenance {
            schema_version: 1,
            transport,
            source_format: source_format.label().to_string(),
            original_working_dir,
            effective_working_dir: effective_working_dir.to_string_lossy().to_string(),
            imported_at: Utc::now(),
            history_trusted: false,
            source_path: source
                .as_ref()
                .and_then(|(path, _)| path.map(|path| path.to_string_lossy().to_string())),
            source_sha256: source.map(|(_, sha256)| sha256),
        }
        .to_extension_data(&mut extension_data)?;

        let imported_conversation = import.conversation.take().map(|conversation| {
            Conversation::new_unvalidated(conversation.messages().iter().cloned().map(
                |mut message| {
                    message.metadata = message.metadata.with_imported_untrusted();
                    message
                },
            ))
        });

        // Session creation, the metadata update, and the conversation replace
        // all run in one transaction so a process interruption between them
        // can't leave an empty, partially-imported session stray behind — a
        // single commit makes the whole import atomic instead of each step
        // being its own independently committed transaction.
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        let session = Self::create_session_in_tx(
            &mut tx,
            effective_working_dir,
            import.name.clone(),
            session_type_override.unwrap_or(import.session_type),
            GoslingMode::Approve,
        )
        .await?;

        let mut builder = session_manager
            .update(&session.id)
            .extension_data(extension_data)
            .restrict_tools_to_working_dirs(true)
            .usage(import.usage)
            .accumulated_usage(import.accumulated_usage)
            .accumulated_cost(import.accumulated_cost);

        if import.user_set_name {
            builder = builder.user_provided_name(import.name.clone());
        }

        Self::apply_update_in_tx(&mut tx, builder).await?;

        if let Some(conversation) = imported_conversation {
            Self::replace_conversation_in_tx(&mut tx, &session.id, &conversation).await?;
        }

        tx.commit().await?;
        #[cfg(feature = "telemetry")]
        crate::posthog::emit_session_started();

        self.get_session(&session.id, true).await
    }

    pub(super) async fn copy_session(
        &self,
        session_manager: &SessionManager,
        session_id: &str,
        new_name: String,
    ) -> Result<Session> {
        let original_session = self.get_session(session_id, true).await?;

        // Session creation, the metadata update, the conversation replace,
        // and the artifact copy all run in one transaction so a process
        // interruption between them can't leave an empty stray copy behind —
        // see import_session's identical comment.
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        let new_session = Self::create_session_in_tx(
            &mut tx,
            original_session.working_dir.clone(),
            new_name,
            original_session.session_type,
            original_session.gosling_mode,
        )
        .await?;

        let mut builder = session_manager
            .update(&new_session.id)
            .extension_data(original_session.extension_data)
            .restrict_tools_to_working_dirs(original_session.restrict_tools_to_working_dirs);

        if !original_session.additional_working_dirs.is_empty() {
            builder = builder.additional_working_dirs(original_session.additional_working_dirs);
        }

        if let Some(project_id) = original_session.project_id {
            builder = builder.project_id(Some(project_id));
        }
        if let Some(provider_name) = original_session.provider_name {
            builder = builder.provider_name(provider_name);
        }
        if let Some(model_config) = original_session.model_config {
            builder = builder.model_config(model_config);
        }
        if let (Some(workspace_id), Some(workspace_name), Some(context)) = (
            original_session.workspace_id,
            original_session.workspace_name,
            original_session.workspace_context,
        ) {
            builder = builder.workspace_snapshot(
                workspace_id,
                workspace_name,
                original_session.credential_profile_id,
                original_session.credential_profile_name,
                original_session.credential_binding_id,
                context,
            );
        }
        builder = builder.gosling_mode(original_session.gosling_mode);
        Self::apply_update_in_tx(&mut tx, builder).await?;

        if let Some(conversation) = original_session.conversation {
            Self::replace_conversation_in_tx(&mut tx, &new_session.id, &conversation).await?;
        }

        sqlx::query(
            r#"
            INSERT INTO session_artifacts (
                session_id, display_path, resolved_path, base_working_dir, workspace_id,
                mime_type, relation, provenance, source_id, first_seen_at, last_seen_at
            )
            SELECT ?, display_path, resolved_path, base_working_dir, workspace_id,
                   mime_type, relation, provenance, source_id, first_seen_at, last_seen_at
            FROM session_artifacts WHERE session_id = ?
            ON CONFLICT(session_id, resolved_path) DO NOTHING
            "#,
        )
        .bind(&new_session.id)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        #[cfg(feature = "telemetry")]
        crate::posthog::emit_session_started();

        self.get_session(&new_session.id, true).await
    }
}
