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

use super::{Session, SessionImportOutcome, SessionManager, SessionStorage, SessionType};
use crate::config::GoslingMode;
use crate::conversation::Conversation;
use crate::session::extension_data::{EnabledExtensionsState, ExtensionState};
use anyhow::Result;
use chrono::Utc;
use gosling_providers::model::ModelConfig;
use gosling_sdk_types::session_handoff::{SessionHandoffSnapshotV1Dto, SessionHandoffStatusDto};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use std::path::{Path, PathBuf};

impl SessionStorage {
    async fn publish_transferred_plan_update(&self, session_id: &str) {
        match self.plan_snapshot(session_id).await {
            Ok(Some(snapshot)) => self.publish_plan_update(&snapshot),
            Ok(None) => {}
            Err(error) => tracing::warn!(
                session.id = session_id,
                plan.error = %error,
                "transferred plan committed but its update could not be published"
            ),
        }
    }

    /// Looks up a session by an `import_provenance.v1` field inside the same
    /// transaction the caller is about to create a session in, so a
    /// concurrent duplicate import cannot slip between this check and the
    /// creation it guards.
    async fn imported_session_id_by_provenance_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        json_path: &str,
        value: &str,
    ) -> Result<Option<String>> {
        let session_id = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id
            FROM sessions
            WHERE CASE
                WHEN json_valid(extension_data) THEN json_extract(extension_data, ?)
            END = ?
            ORDER BY updated_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(json_path)
        .bind(value)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(session_id)
    }

    pub(super) async fn export_session(&self, id: &str) -> Result<String> {
        // Read-only: a consistent snapshot only needs a deferred transaction,
        // not the process-wide write guard or a write-reserving BEGIN
        // IMMEDIATE. Exporting a large session must not block every other
        // session's writes for its duration.
        let pool = self.pool().await?;
        let mut tx = pool.begin().await?;
        let session = Self::get_session_with_messages_in_tx(&mut tx, id).await?;
        let plan_history =
            Self::native_plan_history_in_tx(&mut tx, id, super::PlanHistorySelection::All).await?;
        let mut value = serde_json::to_value(session)?;
        if !plan_history.plans.is_empty() {
            value
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("native session export must be a JSON object"))?
                .insert(
                    super::NATIVE_PLAN_HISTORY_KEY.to_string(),
                    serde_json::to_value(plan_history)?,
                );
        }
        let exported = serde_json::to_string_pretty(&value)?;
        crate::session::import_formats::ensure_import_payload_size(&exported)?;
        tx.commit().await?;
        Ok(exported)
    }

    pub(super) async fn import_session(
        &self,
        session_manager: &SessionManager,
        json: &str,
        session_type_override: Option<SessionType>,
        working_dir: PathBuf,
        transport: crate::session::import_formats::SessionImportTransport,
        source: Option<(Option<&Path>, String)>,
    ) -> Result<SessionImportOutcome> {
        let source_format = crate::session::import_formats::detect_format(json);
        let normalized = crate::session::import_formats::convert_to_gosling_session_json(json)?;
        let mut normalized_value: Value = serde_json::from_str(&normalized)?;
        let plan_history = normalized_value
            .as_object_mut()
            .and_then(|object| object.remove(super::NATIVE_PLAN_HISTORY_KEY))
            .map(serde_json::from_value::<super::NativePlanHistoryV1>)
            .transpose()?;
        let mut import: Session = serde_json::from_value(normalized_value)?;
        let effective_working_dir =
            crate::session::import_formats::validate_import_working_dir(&working_dir)?;
        let original_working_dir = (!import.working_dir.as_os_str().is_empty())
            .then(|| import.working_dir.to_string_lossy().to_string());
        let mut extension_data = import.extension_data.clone();
        extension_data.remove_extension_state(
            EnabledExtensionsState::EXTENSION_NAME,
            EnabledExtensionsState::VERSION,
        );
        extension_data.remove_extension_state(
            crate::session::SystemPromptExtrasState::EXTENSION_NAME,
            crate::session::SystemPromptExtrasState::VERSION,
        );
        let source_path_string = source
            .as_ref()
            .and_then(|(path, _)| path.map(|path| path.to_string_lossy().to_string()));
        let source_sha256 = source.map(|(_, sha256)| sha256);
        crate::session::import_formats::SessionImportProvenance {
            schema_version: 1,
            transport,
            source_format: source_format.label().to_string(),
            original_working_dir,
            effective_working_dir: effective_working_dir.to_string_lossy().to_string(),
            imported_at: Utc::now(),
            history_trusted: false,
            source_path: source_path_string.clone(),
            source_sha256: source_sha256.clone(),
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

        // The dedup check and the session creation share one write-guarded
        // transaction so two concurrent imports of identical content cannot
        // both pass the check and both create a session. Session creation,
        // the metadata update, and the conversation replace also all run in
        // this same transaction so a process interruption between them can't
        // leave an empty, partially-imported session stray behind — a single
        // commit makes the whole import atomic instead of each step being
        // its own independently committed transaction.
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        if let Some(sha256) = source_sha256.as_deref() {
            if let Some(session_id) = Self::imported_session_id_by_provenance_in_tx(
                &mut tx,
                r#"$."import_provenance.v1".source_sha256"#,
                sha256,
            )
            .await?
            {
                tx.rollback().await?;
                let session = self.get_session(&session_id, true).await?;
                return Ok(SessionImportOutcome::AlreadyImported(session));
            }
        }
        if let Some(path) = source_path_string.as_deref() {
            if let Some(session_id) = Self::imported_session_id_by_provenance_in_tx(
                &mut tx,
                r#"$."import_provenance.v1".source_path"#,
                path,
            )
            .await?
            {
                tx.rollback().await?;
                let session = self.get_session(&session_id, true).await?;
                return Ok(SessionImportOutcome::SourceChanged(session));
            }
        }

        let session = Self::create_session_in_tx(
            &mut tx,
            effective_working_dir,
            import.name.clone(),
            session_type_override.unwrap_or(import.session_type),
            GoslingMode::Approve,
        )
        .await?;

        // Imported files are untrusted history, not an authority transfer.
        // The caller chooses the working directory; provider/model, workspace,
        // credential-profile, folder grants, and workflow ownership remain at
        // their safe new-session defaults and must be selected locally.
        let mut builder = session_manager
            .update(&session.id)
            .extension_data(extension_data)
            .restrict_tools_to_working_dirs(true)
            .usage(import.usage)
            .context_usage_estimated(import.context_usage_estimated)
            .last_request_tokens(import.last_request_tokens)
            .accumulated_usage(import.accumulated_usage)
            .accumulated_cost(import.accumulated_cost);

        if import.user_set_name {
            builder = builder.user_provided_name(import.name.clone());
        }

        Self::apply_update_in_tx(&mut tx, builder).await?;

        if let Some(conversation) = imported_conversation {
            Self::replace_conversation_in_tx(&mut tx, &session.id, &conversation).await?;
        }
        if let Some(plan_history) = &plan_history {
            Self::import_plan_history_as_stale_in_tx(&mut tx, &session.id, plan_history).await?;
        }

        tx.commit().await?;
        self.publish_transferred_plan_update(&session.id).await;
        #[cfg(feature = "telemetry")]
        crate::posthog::emit_session_started();

        Ok(SessionImportOutcome::Imported(
            self.get_session(&session.id, true).await?,
        ))
    }

    pub(super) async fn copy_session(
        &self,
        session_manager: &SessionManager,
        session_id: &str,
        new_name: String,
        conversation_before: Option<i64>,
    ) -> Result<Session> {
        // Session creation, metadata updates, conversation replacement,
        // and input/artifact metadata copies run in one transaction so a process
        // interruption between them can't leave an empty stray copy behind —
        // see import_session's identical comment.
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let original_session = Self::get_session_with_messages_in_tx(&mut tx, session_id).await?;

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
        if let Some(conversation_before) = conversation_before {
            sqlx::query("DELETE FROM messages WHERE session_id = ? AND created_timestamp >= ?")
                .bind(&new_session.id)
                .bind(conversation_before)
                .execute(&mut *tx)
                .await?;
        }

        let library_item_ids = sqlx::query_scalar::<_, String>(
            "SELECT id FROM session_library_items WHERE scope = 'session' AND scope_key = ?",
        )
        .bind(format!("session:{session_id}"))
        .fetch_all(&mut *tx)
        .await?;
        for item_id in library_item_ids {
            // Each branch owns its input entries; linked files keep their original paths.
            sqlx::query(
                r#"
                INSERT INTO session_library_items (
                    id, scope, scope_key, name, kind, mime_type, size_bytes,
                    text_content, image_data, file_path, created_at
                )
                SELECT ?, scope, ?, name, kind, mime_type, size_bytes,
                       text_content, image_data, file_path, created_at
                FROM session_library_items WHERE id = ?
                "#,
            )
            .bind(format!("lib_{}", uuid::Uuid::new_v4()))
            .bind(format!("session:{}", new_session.id))
            .bind(item_id)
            .execute(&mut *tx)
            .await?;
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
        Self::clone_plan_history_as_stale_in_tx(
            &mut tx,
            session_id,
            &new_session.id,
            super::PlanHistorySelection::All,
            "copied plan history cannot transfer approval authority",
        )
        .await?;

        tx.commit().await?;
        self.publish_transferred_plan_update(&new_session.id).await;
        #[cfg(feature = "telemetry")]
        crate::posthog::emit_session_started();

        self.get_session(&new_session.id, true).await
    }

    pub(super) async fn create_handoff_session(
        &self,
        session_manager: &SessionManager,
        source_session_id: &str,
        new_name: String,
        provider_name: String,
        model_config: ModelConfig,
        mut snapshot: SessionHandoffSnapshotV1Dto,
    ) -> Result<(Session, SessionHandoffSnapshotV1Dto)> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let original_session =
            Self::get_session_with_messages_in_tx(&mut tx, source_session_id).await?;
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
            .restrict_tools_to_working_dirs(original_session.restrict_tools_to_working_dirs)
            .provider_name(provider_name)
            .model_config(model_config)
            .gosling_mode(original_session.gosling_mode);
        if !original_session.additional_working_dirs.is_empty() {
            builder = builder.additional_working_dirs(original_session.additional_working_dirs);
        }
        if let Some(project_id) = original_session.project_id {
            builder = builder.project_id(Some(project_id));
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
        Self::apply_update_in_tx(&mut tx, builder).await?;

        snapshot.session_id = new_session.id.clone();
        snapshot.source_session_id = Some(source_session_id.to_string());
        snapshot.generation = 1;
        snapshot.status = SessionHandoffStatusDto::Active;
        snapshot.activated_at = Some(Utc::now().to_rfc3339());
        if snapshot.delivery_strategy
            != gosling_sdk_types::session_handoff::HandoffDeliveryStrategyDto::NewContext
        {
            let conversation = Conversation::new_unvalidated(vec![
                crate::session::handoff::handoff_bootstrap_message(&snapshot)?,
            ]);
            Self::replace_conversation_in_tx(&mut tx, &new_session.id, &conversation).await?;
        }
        Self::insert_handoff_snapshot_in_tx(&mut tx, &snapshot).await?;

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
        .bind(source_session_id)
        .execute(&mut *tx)
        .await?;
        Self::clone_plan_history_as_stale_in_tx(
            &mut tx,
            source_session_id,
            &new_session.id,
            super::PlanHistorySelection::Latest,
            "handoff plan context cannot transfer approval authority",
        )
        .await?;
        tx.commit().await?;
        self.publish_transferred_plan_update(&new_session.id).await;
        #[cfg(feature = "telemetry")]
        crate::posthog::emit_session_started();

        Ok((self.get_session(&new_session.id, true).await?, snapshot))
    }
}
