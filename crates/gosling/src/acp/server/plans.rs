//! Thin ACP adapters over the authoritative host-enforced plan service.

use super::*;
use crate::agents::interaction_policy::PlanningCapability;
use crate::session::{
    approved_plan_implementation_reference, NewPlanFeedback, PlanError, PlanExpectation,
    PlanSnapshot, PlanStatus, Session,
};

impl GoslingAcpAgent {
    pub(super) async fn on_get_session_plan(
        &self,
        request: GetSessionPlanRequest,
    ) -> Result<SessionPlanResponse, agent_client_protocol::Error> {
        self.plan_session(&request.session_id).await?;
        let service = self.session_manager.plans();
        let snapshot = match request.generation {
            Some(generation) => {
                service
                    .snapshot_generation(&request.session_id, generation)
                    .await
            }
            None => service.snapshot(&request.session_id).await,
        }
        .map_err(|error| self.plan_error_without_snapshot(error))?;
        Ok(self.plan_response(&request.session_id, snapshot).await)
    }

    pub(super) async fn on_start_session_plan(
        &self,
        request: StartSessionPlanRequest,
    ) -> Result<SessionPlanResponse, agent_client_protocol::Error> {
        let session = self.plan_session(&request.session_id).await?;
        let agent = self
            .get_session_agent(&request.session_id)
            .await
            .map_err(|_| {
                self.plan_error_without_snapshot(PlanError::ProviderUnsupported(
                    "the current session provider is unavailable".to_string(),
                ))
            })?;
        let provider = agent.provider().await.map_err(|_| {
            self.plan_error_without_snapshot(PlanError::ProviderUnsupported(
                "the current session provider is unavailable".to_string(),
            ))
        })?;
        let planner_model = session
            .model_config
            .as_ref()
            .map(|model| model.model_name.clone());
        let result = self
            .session_manager
            .plans()
            .start_or_resume(
                &request.session_id,
                provider.as_ref(),
                planner_model,
                request.expected_generation,
            )
            .await;
        let snapshot = self.plan_result(&request.session_id, result).await?;
        Ok(self
            .plan_response(&request.session_id, Some(snapshot))
            .await)
    }

    pub(super) async fn on_add_session_plan_feedback(
        &self,
        request: AddSessionPlanFeedbackRequest,
    ) -> Result<SessionPlanResponse, agent_client_protocol::Error> {
        self.plan_session(&request.session_id).await?;
        let expectation = plan_expectation(
            request.expected_generation,
            request.expected_revision_id,
            request.expected_revision_sha256,
            request.expected_source_hash,
            request.expected_scope_hash,
        );
        let result = self
            .session_manager
            .plans()
            .add_feedback(
                &request.session_id,
                &expectation,
                NewPlanFeedback {
                    body: request.body,
                    start_line: request.start_line,
                    end_line: request.end_line,
                    selected_text: request.selected_text,
                },
            )
            .await;
        let snapshot = self.plan_result(&request.session_id, result).await?;
        Ok(self
            .plan_response(&request.session_id, Some(snapshot))
            .await)
    }

    pub(super) async fn on_approve_session_plan(
        &self,
        request: ApproveSessionPlanRequest,
    ) -> Result<SessionPlanResponse, agent_client_protocol::Error> {
        self.plan_session(&request.session_id).await?;
        let expectation = plan_expectation(
            request.expected_generation,
            request.expected_revision_id,
            request.expected_revision_sha256,
            request.expected_source_hash,
            request.expected_scope_hash,
        );
        let result = self
            .session_manager
            .plans()
            .approve(&request.session_id, &expectation, request.decision_note)
            .await;
        let snapshot = self.plan_result(&request.session_id, result).await?;
        Ok(self
            .plan_response(&request.session_id, Some(snapshot))
            .await)
    }

    pub(super) async fn on_abandon_session_plan(
        &self,
        request: AbandonSessionPlanRequest,
    ) -> Result<SessionPlanResponse, agent_client_protocol::Error> {
        self.plan_session(&request.session_id).await?;
        let result = self
            .session_manager
            .plans()
            .abandon(&request.session_id, request.expected_generation, None)
            .await;
        let snapshot = self.plan_result(&request.session_id, result).await?;
        Ok(self
            .plan_response(&request.session_id, Some(snapshot))
            .await)
    }

    pub(super) async fn on_export_session_plan(
        &self,
        request: ExportSessionPlanRequest,
    ) -> Result<ExportSessionPlanResponse, agent_client_protocol::Error> {
        self.plan_session(&request.session_id).await?;
        let result = self
            .session_manager
            .plans()
            .export_markdown(
                &request.session_id,
                request.expected_generation,
                plan_status(request.expected_status),
                &request.expected_revision_id,
                &request.expected_revision_sha256,
            )
            .await;
        let markdown = self.plan_result(&request.session_id, result).await?;
        Ok(ExportSessionPlanResponse { markdown })
    }

    async fn plan_session(
        &self,
        session_id: &str,
    ) -> Result<Session, agent_client_protocol::Error> {
        self.session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                self.plan_error_without_snapshot(PlanError::SessionNotFound(session_id.to_string()))
            })
    }

    async fn plan_response(
        &self,
        session_id: &str,
        snapshot: Option<PlanSnapshot>,
    ) -> SessionPlanResponse {
        // The live session agent is authoritative here. Persisted provider metadata may be
        // absent for older/default-provider sessions, and a caller-authored capability flag
        // must never determine whether host-enforced planning is available.
        let agent = self.get_session_agent(session_id).await.ok();
        let provider_supports_host_enforced_planning = match agent.as_ref() {
            Some(agent) => agent
                .provider()
                .await
                .is_ok_and(|provider| !provider.executes_tools_outside_gosling()),
            None => false,
        };
        let drafting = snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.plan.status == PlanStatus::Drafting);
        let permitted_capabilities = if provider_supports_host_enforced_planning && drafting {
            let available_tools = match agent {
                Some(agent) => {
                    agent
                        .extension_manager
                        .get_planning_tools_without_external_catalog()
                        .await
                }
                None => Vec::new(),
            };
            let permission_manager = self.permission_manager();
            available_tools
                .into_iter()
                .filter_map(|(catalog_tool, identity)| {
                    let capability = PlanningCapability::from_host_identity(&identity)?;
                    capability
                        .is_permitted_for_catalog(
                            permission_manager.get_user_permission(catalog_tool.name.as_ref()),
                        )
                        .then(|| planning_capability_dto(capability))
                })
                .collect()
        } else {
            Vec::new()
        };
        // This is a server-authored opaque reference, not a client-side grammar. Only
        // expose it while the returned approved snapshot is still the session's current
        // implementation authority. Historical approved generations remain reviewable
        // but cannot be submitted as current authority.
        let implementation_reference = match snapshot.as_ref() {
            Some(candidate) if candidate.plan.status == PlanStatus::Approved => self
                .session_manager
                .plans()
                .snapshot(session_id)
                .await
                .ok()
                .flatten()
                .filter(|current| same_implementation_authority(current, candidate))
                .and_then(|current| approved_plan_implementation_reference(&current).ok()),
            _ => None,
        };
        SessionPlanResponse {
            snapshot: snapshot.as_ref().map(plan_snapshot_dto),
            provider_supports_host_enforced_planning,
            permitted_capabilities,
            implementation_reference,
        }
    }

    async fn plan_result<T>(
        &self,
        session_id: &str,
        result: Result<T, PlanError>,
    ) -> Result<T, agent_client_protocol::Error> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(self.plan_error(session_id, error).await),
        }
    }

    async fn plan_error(&self, session_id: &str, error: PlanError) -> agent_client_protocol::Error {
        let include_current = matches!(
            error,
            PlanError::Conflict(_) | PlanError::InvalidTransition(_) | PlanError::Busy(_)
        );
        let current_snapshot = if include_current {
            self.session_manager
                .plans()
                .snapshot(session_id)
                .await
                .ok()
                .flatten()
                .as_ref()
                .map(plan_snapshot_dto)
        } else {
            None
        };
        plan_rpc_error(error, current_snapshot)
    }

    fn plan_error_without_snapshot(&self, error: PlanError) -> agent_client_protocol::Error {
        plan_rpc_error(error, None)
    }
}

fn same_implementation_authority(current: &PlanSnapshot, candidate: &PlanSnapshot) -> bool {
    current.plan.id == candidate.plan.id
        && current.plan.generation == candidate.plan.generation
        && current.plan.status == PlanStatus::Approved
        && candidate.plan.status == PlanStatus::Approved
        && match (
            current.active_revision.as_ref(),
            candidate.active_revision.as_ref(),
        ) {
            (Some(current), Some(candidate)) => {
                current.id == candidate.id
                    && current.content_sha256 == candidate.content_sha256
                    && current.source_hash == candidate.source_hash
                    && current.scope_hash == candidate.scope_hash
            }
            _ => false,
        }
}

fn plan_expectation(
    generation: u64,
    revision_id: String,
    revision_sha256: String,
    source_hash: String,
    scope_hash: String,
) -> PlanExpectation {
    PlanExpectation {
        generation,
        revision_id: Some(revision_id),
        revision_sha256: Some(revision_sha256),
        source_hash,
        scope_hash,
    }
}

fn planning_capability_dto(capability: PlanningCapability) -> PlanningCapabilityDto {
    match capability {
        PlanningCapability::WorkspaceTree => PlanningCapabilityDto::WorkspaceTree,
        PlanningCapability::WorkspaceReadText => PlanningCapabilityDto::WorkspaceReadText,
        PlanningCapability::WorkspaceSearchText => PlanningCapabilityDto::WorkspaceSearchText,
        PlanningCapability::SessionSearch => PlanningCapabilityDto::SessionHistorySearch,
        PlanningCapability::SessionRead => PlanningCapabilityDto::SessionHistoryRead,
        PlanningCapability::PlanUpdate => PlanningCapabilityDto::PlanUpdate,
        PlanningCapability::PlanRequestReview => PlanningCapabilityDto::PlanRequestReview,
    }
}

fn plan_status_dto(status: PlanStatus) -> PlanStatusDto {
    match status {
        PlanStatus::Drafting => PlanStatusDto::Drafting,
        PlanStatus::AwaitingReview => PlanStatusDto::AwaitingReview,
        PlanStatus::Approved => PlanStatusDto::Approved,
        PlanStatus::Abandoned => PlanStatusDto::Abandoned,
        PlanStatus::Stale => PlanStatusDto::Stale,
    }
}

fn plan_status(status: PlanStatusDto) -> PlanStatus {
    match status {
        PlanStatusDto::Drafting => PlanStatus::Drafting,
        PlanStatusDto::AwaitingReview => PlanStatus::AwaitingReview,
        PlanStatusDto::Approved => PlanStatus::Approved,
        PlanStatusDto::Abandoned => PlanStatus::Abandoned,
        PlanStatusDto::Stale => PlanStatus::Stale,
    }
}

pub(super) fn plan_update_notification(
    update: crate::session::PlanUpdate,
) -> GoslingSessionNotification {
    GoslingSessionNotification {
        session_id: update.session_id,
        update: GoslingSessionUpdate::PlanUpdate(crate::acp::custom_notifications::PlanUpdate {
            plan_id: update.plan_id,
            generation: update.generation,
            status: plan_status_dto(update.status),
            active_revision: update
                .active_revision
                .map(|revision| PlanRevisionIdentityDto {
                    id: revision.id,
                    revision: revision.revision,
                    content_sha256: revision.content_sha256,
                }),
            updated_at: update.updated_at.to_rfc3339(),
        }),
    }
}

pub(super) fn plan_snapshot_dto(snapshot: &PlanSnapshot) -> PlanSnapshotDto {
    PlanSnapshotDto {
        plan: SessionPlanDto {
            id: snapshot.plan.id.clone(),
            generation: snapshot.plan.generation,
            status: plan_status_dto(snapshot.plan.status),
            source_through_row_id: snapshot.plan.source_through_row_id,
            source_hash: snapshot.plan.source_hash.clone(),
            scope_hash: snapshot.plan.scope_hash.clone(),
            capability_policy_version: snapshot.plan.capability_policy_version,
            planner_provider: snapshot.plan.planner_provider.clone(),
            planner_model: snapshot.plan.planner_model.clone(),
            stale_reason: snapshot.plan.stale_reason.clone(),
            created_at: snapshot.plan.created_at.to_rfc3339(),
            updated_at: snapshot.plan.updated_at.to_rfc3339(),
        },
        active_revision: snapshot
            .active_revision
            .as_ref()
            .map(|revision| SessionPlanRevisionDto {
                id: revision.id.clone(),
                revision: revision.revision,
                parent_revision_id: revision.parent_revision_id.clone(),
                content_markdown: revision.content_markdown.clone(),
                content_sha256: revision.content_sha256.clone(),
                planner_provider: revision.planner_provider.clone(),
                planner_model: revision.planner_model.clone(),
                source_through_row_id: revision.source_through_row_id,
                source_hash: revision.source_hash.clone(),
                scope_hash: revision.scope_hash.clone(),
                created_at: revision.created_at.to_rfc3339(),
            }),
        feedback: snapshot
            .feedback
            .iter()
            .map(|feedback| SessionPlanFeedbackDto {
                id: feedback.id.clone(),
                revision_id: feedback.revision_id.clone(),
                body: feedback.body.clone(),
                start_line: feedback.start_line,
                end_line: feedback.end_line,
                selected_text_sha256: feedback.selected_text_sha256.clone(),
                selected_text_preview: feedback.selected_text_preview.clone(),
                consumed_by_revision_id: feedback.consumed_by_revision_id.clone(),
                created_at: feedback.created_at.to_rfc3339(),
            })
            .collect(),
        recent_events: snapshot
            .recent_events
            .iter()
            .map(|event| SessionPlanEventDto {
                event_type: event.event_type.clone(),
                from_status: event.from_status.map(plan_status_dto),
                to_status: event.to_status.map(plan_status_dto),
                revision_id: event.revision_id.clone(),
                revision_sha256: event.revision_sha256.clone(),
                actor: event.actor.clone(),
                created_at: event.created_at.to_rfc3339(),
            })
            .collect(),
    }
}

fn plan_rpc_error(
    error: PlanError,
    current_snapshot: Option<PlanSnapshotDto>,
) -> agent_client_protocol::Error {
    let message = error.to_string();
    let (mut response, code, message) = match error {
        PlanError::SessionNotFound(_) | PlanError::NoOpenPlan(_) | PlanError::PlanNotFound(_) => (
            agent_client_protocol::Error::resource_not_found(None),
            "plan_not_found",
            message,
        ),
        PlanError::Conflict(_) => (
            agent_client_protocol::Error::invalid_params(),
            "plan_conflict",
            message,
        ),
        PlanError::InvalidTransition(_) => (
            agent_client_protocol::Error::invalid_params(),
            "plan_invalid_transition",
            message,
        ),
        PlanError::InvalidInput(_) => (
            agent_client_protocol::Error::invalid_params(),
            "plan_validation",
            message,
        ),
        PlanError::LimitExceeded(_) => (
            agent_client_protocol::Error::invalid_params(),
            "plan_limit_exceeded",
            message,
        ),
        PlanError::Busy(_) => (
            agent_client_protocol::Error::invalid_params(),
            "plan_busy",
            message,
        ),
        PlanError::ProviderUnsupported(_) => (
            agent_client_protocol::Error::invalid_params(),
            "plan_provider_unsupported",
            message,
        ),
        PlanError::CorruptState(_) | PlanError::Storage(_) => (
            agent_client_protocol::Error::internal_error(),
            "plan_storage",
            "plan state is temporarily unavailable".to_string(),
        ),
    };
    response.message = message.clone();
    let mut data = serde_json::json!({ "code": code, "message": message });
    if let Some(snapshot) = current_snapshot {
        data["currentSnapshot"] =
            serde_json::to_value(snapshot).unwrap_or_else(|_| serde_json::Value::Null);
    }
    response.data(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_storage_errors_do_not_expose_inner_details() {
        let error = plan_rpc_error(
            PlanError::Storage("database failed while storing # secret plan".to_string()),
            None,
        );
        let wire = serde_json::to_value(error).unwrap();
        assert_eq!(wire["data"]["code"], "plan_storage");
        assert_eq!(wire["message"], "plan state is temporarily unavailable");
        assert!(!wire.to_string().contains("secret plan"));
    }

    #[test]
    fn plan_errors_preserve_stable_machine_codes() {
        for (error, expected) in [
            (PlanError::PlanNotFound("missing".into()), "plan_not_found"),
            (PlanError::Conflict("changed".into()), "plan_conflict"),
            (
                PlanError::InvalidTransition("wrong state".into()),
                "plan_invalid_transition",
            ),
            (PlanError::InvalidInput("invalid".into()), "plan_validation"),
            (
                PlanError::LimitExceeded("too large".into()),
                "plan_limit_exceeded",
            ),
            (PlanError::Busy("running".into()), "plan_busy"),
            (
                PlanError::ProviderUnsupported("unsafe".into()),
                "plan_provider_unsupported",
            ),
        ] {
            let wire = serde_json::to_value(plan_rpc_error(error, None)).unwrap();
            assert_eq!(wire["data"]["code"], expected);
        }
    }

    #[test]
    fn conflict_errors_include_the_current_snapshot() {
        let snapshot = PlanSnapshotDto {
            plan: SessionPlanDto {
                id: "plan-1".to_string(),
                generation: 2,
                status: PlanStatusDto::Drafting,
                ..Default::default()
            },
            ..Default::default()
        };
        let error = plan_rpc_error(
            PlanError::Conflict("generation changed".to_string()),
            Some(snapshot),
        );
        let wire = serde_json::to_value(error).unwrap();
        assert_eq!(wire["data"]["code"], "plan_conflict");
        assert_eq!(wire["data"]["currentSnapshot"]["plan"]["id"], "plan-1");
    }
}
