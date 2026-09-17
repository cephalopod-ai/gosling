//! Durable skill admissions and the authority ceilings they impose.
//!
//! An admission is scoped to the session turn lease that was current when the
//! host recorded it, so it ends when that turn ends and a later, unrelated turn
//! does not inherit it. A delegated subagent session receives a copy of its
//! parent's active restrictive admissions for its whole lifetime. Admissions
//! are never exported, imported, copied, or forked: they restrict one live
//! task and are not authority to transfer.

use super::SessionStorage;
use crate::skills::admission::{
    ActiveSkillCeiling, AdmissionScope, AuthorityCeiling, SkillAdmission, SkillCeilingDenied,
};
use anyhow::Result;
use sqlx::{Sqlite, Transaction};

/// Bounds the skill ids named in denial messages and approval prompts.
const MAX_REPORTED_SKILL_IDS: usize = 8;

/// Whether the tool-operation begin transaction must re-evaluate the active
/// skill ceiling for a new operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillScopeGate {
    Evaluate {
        verified_non_mutating: bool,
        user_approved: bool,
    },
    #[cfg(test)]
    NotApplicable,
}

impl SessionStorage {
    pub(super) async fn create_skill_admission_schema(
        tx: &mut Transaction<'_, Sqlite>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS skill_admissions (
                admission_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                scope_kind TEXT NOT NULL CHECK(scope_kind IN ('turn', 'delegated_session')),
                turn_lease_id TEXT,
                inherited_from_session_id TEXT,
                record_kind TEXT NOT NULL CHECK(record_kind IN ('skill', 'supporting_file')),
                channel TEXT NOT NULL CHECK(channel IN ('model_tool_load', 'user_slash_command', 'delegation')),
                skill_id TEXT NOT NULL,
                relative_path TEXT,
                source_kind TEXT NOT NULL CHECK(source_kind IN ('configured_catalog', 'project', 'user', 'plugin', 'builtin')),
                catalog_id TEXT,
                declared_version TEXT,
                content_sha256 TEXT NOT NULL,
                declared_hash_status TEXT NOT NULL CHECK(declared_hash_status IN ('not_declared', 'verified', 'unverifiable_format')),
                authority_label TEXT,
                authority_mapping TEXT NOT NULL CHECK(authority_mapping IN ('absent', 'known', 'unrecognized')),
                ceiling TEXT NOT NULL CHECK(ceiling IN ('unrestricted', 'non_mutating', 'human_approval_required')),
                requires_human_approval_for_json TEXT NOT NULL DEFAULT '[]',
                tool_operation_id TEXT,
                created_at INTEGER NOT NULL,
                CHECK((scope_kind = 'turn' AND turn_lease_id IS NOT NULL)
                   OR (scope_kind = 'delegated_session' AND inherited_from_session_id IS NOT NULL))
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_skill_admissions_scope ON skill_admissions(session_id, scope_kind, turn_lease_id)",
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Records an admission against the session's current turn lease in the
    /// same transaction that reads the lease. Without a live turn nothing is
    /// recorded and the caller discloses that no restriction applies.
    pub(super) async fn record_skill_admission(
        &self,
        session_id: &str,
        admission: &SkillAdmission,
        tool_operation_id: Option<&str>,
    ) -> Result<AdmissionScope> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let lease_id = sqlx::query_scalar::<_, String>(
            "SELECT lease_id FROM session_turn_leases WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(lease_id) = lease_id else {
            tx.commit().await?;
            return Ok(AdmissionScope::Unscoped);
        };
        Self::insert_admission(
            &mut tx,
            session_id,
            ("turn", Some(lease_id.as_str()), None),
            admission,
            tool_operation_id,
        )
        .await?;
        tx.commit().await?;
        Ok(AdmissionScope::Turn)
    }

    /// Copies the parent's active restrictive admissions onto a delegated
    /// session. A delegate cannot shed a restriction by running elsewhere.
    pub(super) async fn inherit_skill_admissions(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
    ) -> Result<usize> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let inserted = sqlx::query(
            r#"
            INSERT INTO skill_admissions (
                admission_id, session_id, scope_kind, turn_lease_id, inherited_from_session_id,
                record_kind, channel, skill_id, relative_path, source_kind, catalog_id,
                declared_version, content_sha256, declared_hash_status, authority_label,
                authority_mapping, ceiling, requires_human_approval_for_json, tool_operation_id,
                created_at
            )
            SELECT
                'skilladm_' || lower(hex(randomblob(16))), ?, 'delegated_session', NULL, ?,
                record_kind, 'delegation', skill_id, relative_path, source_kind, catalog_id,
                declared_version, content_sha256, declared_hash_status, authority_label,
                authority_mapping, ceiling, requires_human_approval_for_json, NULL,
                ?
            FROM skill_admissions
            WHERE session_id = ?
              AND record_kind = 'skill'
              AND ceiling != 'unrestricted'
              AND (
                scope_kind = 'delegated_session'
                OR (scope_kind = 'turn' AND turn_lease_id = (
                    SELECT lease_id FROM session_turn_leases WHERE session_id = ?
                ))
              )
            "#,
        )
        .bind(child_session_id)
        .bind(parent_session_id)
        .bind(chrono::Utc::now().timestamp())
        .bind(parent_session_id)
        .bind(parent_session_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(inserted as usize)
    }

    pub(super) async fn active_skill_ceiling(
        &self,
        session_id: &str,
    ) -> Result<ActiveSkillCeiling> {
        let pool = self.pool().await?;
        let mut connection = pool.acquire().await?;
        Self::active_skill_ceiling_in(&mut connection, session_id).await
    }

    pub(super) async fn active_skill_ceiling_in(
        connection: &mut sqlx::SqliteConnection,
        session_id: &str,
    ) -> Result<ActiveSkillCeiling> {
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT skill_id, ceiling
            FROM skill_admissions
            WHERE session_id = ?
              AND record_kind = 'skill'
              AND ceiling != 'unrestricted'
              AND (
                scope_kind = 'delegated_session'
                OR (scope_kind = 'turn' AND turn_lease_id = (
                    SELECT lease_id FROM session_turn_leases WHERE session_id = ?
                ))
              )
            ORDER BY created_at, admission_id
            "#,
        )
        .bind(session_id)
        .bind(session_id)
        .fetch_all(&mut *connection)
        .await?;
        let mut active = ActiveSkillCeiling::unrestricted();
        for (skill_id, ceiling) in rows {
            active.ceiling = active.ceiling.max(AuthorityCeiling::from_stored(&ceiling));
            if active.restricting_skill_ids.len() < MAX_REPORTED_SKILL_IDS
                && !active.restricting_skill_ids.contains(&skill_id)
            {
                active.restricting_skill_ids.push(skill_id);
            }
        }
        Ok(active)
    }

    /// Evaluated inside the tool-operation begin transaction, so an admission
    /// committed before this point cannot be missed and one committed after it
    /// applies only to later operations.
    ///
    /// Turn-scoped admissions are found through the current lease, so a turn
    /// whose lease another process took over would otherwise look unrestricted
    /// until its heartbeat notices the takeover. Its conversation operations
    /// are refused instead.
    pub(super) async fn enforce_skill_scope_in_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        conversation_bound: bool,
        gate: SkillScopeGate,
    ) -> Result<()> {
        match gate {
            SkillScopeGate::Evaluate {
                verified_non_mutating,
                user_approved,
            } if !verified_non_mutating && !user_approved => {}
            _ => return Ok(()),
        }
        if conversation_bound {
            let lease_owner = sqlx::query_scalar::<_, String>(
                "SELECT owner_id FROM session_turn_leases WHERE session_id = ?",
            )
            .bind(session_id)
            .fetch_optional(&mut **tx)
            .await?;
            if lease_owner.is_some_and(|owner| owner != self.owner_id) {
                anyhow::bail!(
                    "the session's turn lease is held by another process; this turn was superseded"
                );
            }
        }
        let active = Self::active_skill_ceiling_in(tx, session_id).await?;
        if active.ceiling.is_restrictive() {
            return Err(SkillCeilingDenied { ceiling: active }.into());
        }
        Ok(())
    }

    async fn insert_admission(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        (scope_kind, turn_lease_id, inherited_from): (&str, Option<&str>, Option<&str>),
        admission: &SkillAdmission,
        tool_operation_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO skill_admissions (
                admission_id, session_id, scope_kind, turn_lease_id, inherited_from_session_id,
                record_kind, channel, skill_id, relative_path, source_kind, catalog_id,
                declared_version, content_sha256, declared_hash_status, authority_label,
                authority_mapping, ceiling, requires_human_approval_for_json, tool_operation_id,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("skilladm_{}", uuid::Uuid::new_v4().simple()))
        .bind(session_id)
        .bind(scope_kind)
        .bind(turn_lease_id)
        .bind(inherited_from)
        .bind(admission.record_kind().as_str())
        .bind(admission.channel().as_str())
        .bind(admission.skill_id())
        .bind(admission.relative_path())
        .bind(admission.source_kind().as_str())
        .bind(admission.catalog_id())
        .bind(admission.declared_version())
        .bind(admission.content_sha256())
        .bind(admission.declared_hash_status().as_str())
        .bind(admission.authority_label())
        .bind(admission.authority_mapping().as_str())
        .bind(admission.ceiling().as_str())
        .bind(serde_json::to_string(
            admission.requires_human_approval_for(),
        )?)
        .bind(tool_operation_id)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::GoslingMode;
    use crate::session::extension_data::{DeepResearchState, ExtensionData, ExtensionState};
    use crate::session::session_manager::{SessionImportOutcome, SessionType};
    use crate::session::SessionManager;
    use crate::skills::admission::{
        AdmissionChannel, AdmissionScope, SkillAdmission, SkillOrigin, SkillSourceKind,
    };
    use tempfile::TempDir;

    fn read_only_admission() -> SkillAdmission {
        SkillAdmission::for_skill(
            "eia-audit",
            &SkillOrigin::new(SkillSourceKind::Project),
            Some("read_only"),
            b"inspect only",
            AdmissionChannel::ModelToolLoad,
        )
        .unwrap()
    }

    async fn restricted_session(sessions: &SessionManager, dir: &TempDir) -> String {
        let session = sessions
            .create_session(
                dir.path().to_path_buf(),
                "restricted".to_string(),
                SessionType::User,
                GoslingMode::Auto,
            )
            .await
            .unwrap();
        session.id
    }

    async fn restricted(sessions: &SessionManager, session_id: &str) -> bool {
        sessions
            .active_skill_ceiling(session_id)
            .await
            .unwrap()
            .ceiling
            .is_restrictive()
    }

    // EIA-COMPAT-001: without a live turn nothing is recorded, so nothing restricts or grants.
    #[tokio::test]
    async fn admission_without_a_live_turn_records_nothing() {
        let dir = TempDir::new().unwrap();
        let sessions = SessionManager::new(dir.path().join("data"));
        let session_id = restricted_session(&sessions, &dir).await;
        assert_eq!(
            sessions
                .record_skill_admission(&session_id, &read_only_admission(), None)
                .await
                .unwrap(),
            AdmissionScope::Unscoped
        );
        assert!(!restricted(&sessions, &session_id).await);
    }

    // EIA-HANDOFF-002: copy, fork, export, and import preserve history but never
    // transfer admissions, and import discards file-supplied Deep Research paths.
    #[tokio::test]
    async fn transfers_carry_no_admissions_or_imported_host_paths() {
        let dir = TempDir::new().unwrap();
        let sessions = SessionManager::new(dir.path().join("data"));
        let session_id = restricted_session(&sessions, &dir).await;
        let mut extension_data = ExtensionData::new();
        DeepResearchState {
            library_path: "/etc".to_string(),
            output_paths: vec!["/tmp/eia-imported-output".to_string()],
        }
        .to_extension_data(&mut extension_data)
        .unwrap();
        sessions
            .update(&session_id)
            .extension_data(extension_data)
            .apply()
            .await
            .unwrap();
        sessions
            .add_message(
                &session_id,
                &crate::conversation::message::Message::user().with_text("historical evidence"),
            )
            .await
            .unwrap();
        let _lease = sessions
            .acquire_session_turn_lease(&session_id, None)
            .await
            .unwrap();
        sessions
            .record_skill_admission(&session_id, &read_only_admission(), None)
            .await
            .unwrap();
        assert!(restricted(&sessions, &session_id).await);

        let copied = sessions
            .copy_session(&session_id, "copy".to_string())
            .await
            .unwrap();
        let forked = sessions
            .fork_session(&session_id, "fork".to_string(), None)
            .await
            .unwrap();
        let exported = sessions.export_session(&session_id).await.unwrap();
        assert!(!exported.contains("skill_admissions"));
        let SessionImportOutcome::Imported(imported) = sessions
            .import_session(
                &exported,
                None,
                dir.path().to_path_buf(),
                crate::session::import_formats::SessionImportTransport::Json,
            )
            .await
            .unwrap()
        else {
            panic!("expected a new import");
        };

        for transferred in [&copied, &forked, &imported] {
            assert!(!restricted(&sessions, &transferred.id).await);
        }
        let imported = sessions.get_session(&imported.id, true).await.unwrap();
        assert!(DeepResearchState::from_extension_data(&imported.extension_data).is_none());
        assert_eq!(imported.gosling_mode, GoslingMode::Approve);
        assert!(imported
            .conversation
            .unwrap()
            .messages()
            .iter()
            .any(|message| message.as_concat_text().contains("historical evidence")));
    }

    #[tokio::test]
    async fn deleting_a_session_removes_its_admissions() {
        let dir = TempDir::new().unwrap();
        let sessions = SessionManager::new(dir.path().join("data"));
        let session_id = restricted_session(&sessions, &dir).await;
        let lease = sessions
            .acquire_session_turn_lease(&session_id, None)
            .await
            .unwrap();
        sessions
            .record_skill_admission(&session_id, &read_only_admission(), None)
            .await
            .unwrap();
        lease.release().await.unwrap();
        sessions.delete_session(&session_id).await.unwrap();
        let pool = sessions.storage.pool().await.unwrap();
        let remaining = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM skill_admissions WHERE session_id = ?",
        )
        .bind(&session_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(remaining, 0);
    }

    // EIA-COMPAT-002: a stored ceiling that bypassed the CHECK constraint fails closed.
    #[tokio::test]
    async fn corrupt_stored_ceiling_restricts_instead_of_granting() {
        let dir = TempDir::new().unwrap();
        let sessions = SessionManager::new(dir.path().join("data"));
        let session_id = restricted_session(&sessions, &dir).await;
        let _lease = sessions
            .acquire_session_turn_lease(&session_id, None)
            .await
            .unwrap();
        sessions
            .record_skill_admission(&session_id, &read_only_admission(), None)
            .await
            .unwrap();
        let pool = sessions.storage.pool().await.unwrap();
        let mut connection = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("UPDATE skill_admissions SET ceiling = 'wide_open' WHERE session_id = ?")
            .bind(&session_id)
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        let active = sessions.active_skill_ceiling(&session_id).await.unwrap();
        assert_eq!(
            active.ceiling,
            crate::skills::admission::AuthorityCeiling::HumanApprovalRequired
        );
    }
}
