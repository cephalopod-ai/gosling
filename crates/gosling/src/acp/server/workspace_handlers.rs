use super::*;
use crate::workspace::WorkspaceMutation;

fn workspace_request_error(error: anyhow::Error) -> agent_client_protocol::Error {
    match error.downcast_ref::<crate::workspace::WorkspaceRequestError>() {
        Some(crate::workspace::WorkspaceRequestError::NotFound(_)) => {
            agent_client_protocol::Error::resource_not_found(None)
        }
        Some(crate::workspace::WorkspaceRequestError::Invalid(message)) => {
            agent_client_protocol::Error::invalid_params().data(message.clone())
        }
        None => agent_client_protocol::Error::internal_error()
            .data("Workspace storage operation failed"),
    }
}

impl GoslingAcpAgent {
    async fn workspace_response(
        &self,
        workspace: crate::workspace::Workspace,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let validation = self
            .workspace_service
            .validate(&WorkspaceMutation::from(&workspace))
            .internal_err_ctx("Failed to validate workspace")?;
        Ok(WorkspaceResponse {
            workspace,
            validation,
        })
    }

    pub(super) async fn on_workspace_list(
        &self,
        _request: WorkspaceListRequest,
    ) -> Result<WorkspaceListResponse, agent_client_protocol::Error> {
        let (workspaces, active_workspace_id, default_workspace_id) = self
            .workspace_service
            .list()
            .internal_err_ctx("Failed to list workspaces")?;
        Ok(WorkspaceListResponse {
            workspaces,
            active_workspace_id,
            default_workspace_id,
        })
    }

    pub(super) async fn on_workspace_create(
        &self,
        request: WorkspaceCreateRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        self.validate_workspace_provider_model(&request.workspace)
            .await?;
        let workspace = self
            .workspace_service
            .create(request.workspace)
            .await
            .map_err(workspace_request_error)?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_update(
        &self,
        request: WorkspaceUpdateRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        self.validate_workspace_provider_model(&request.workspace)
            .await?;
        let workspace = self
            .workspace_service
            .update(&request.workspace_id, request.workspace)
            .await
            .map_err(workspace_request_error)?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_duplicate(
        &self,
        request: WorkspaceDuplicateRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let source = self
            .workspace_service
            .get(&request.workspace_id)
            .map_err(workspace_request_error)?;
        self.validate_workspace_provider_model(&WorkspaceMutation::from(&source))
            .await?;
        let workspace = self
            .workspace_service
            .duplicate(&request.workspace_id)
            .await
            .map_err(workspace_request_error)?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_delete(
        &self,
        request: WorkspaceDeleteRequest,
    ) -> Result<WorkspaceDeleteResponse, agent_client_protocol::Error> {
        let (active_workspace_id, default_workspace_id) = self
            .workspace_service
            .delete(&request.workspace_id)
            .await
            .map_err(workspace_request_error)?;
        Ok(WorkspaceDeleteResponse {
            active_workspace_id,
            default_workspace_id,
        })
    }

    pub(super) async fn on_workspace_set_active(
        &self,
        request: WorkspaceSetActiveRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let workspace = self
            .workspace_service
            .set_active(&request.workspace_id)
            .await
            .map_err(workspace_request_error)?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_validate(
        &self,
        request: WorkspaceValidateRequest,
    ) -> Result<WorkspaceValidationResponse, agent_client_protocol::Error> {
        self.validate_workspace_provider_model(&request.workspace)
            .await?;
        let validation = self
            .workspace_service
            .validate(&request.workspace)
            .map_err(workspace_request_error)?;
        Ok(WorkspaceValidationResponse { validation })
    }

    pub(super) async fn on_workspace_export(
        &self,
        request: WorkspaceExportRequest,
    ) -> Result<WorkspaceExportResponse, agent_client_protocol::Error> {
        let document = self
            .workspace_service
            .export(&request.workspace_id)
            .map_err(workspace_request_error)?;
        Ok(WorkspaceExportResponse { document })
    }

    pub(super) async fn on_workspace_import(
        &self,
        request: WorkspaceImportRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let imported: crate::workspace::Workspace = serde_json::from_str(&request.document)
            .invalid_params_err_ctx("Invalid workspace import")?;
        self.validate_workspace_provider_model(&WorkspaceMutation::from(&imported))
            .await?;
        let workspace = self
            .workspace_service
            .import(&request.document)
            .await
            .map_err(workspace_request_error)?;
        self.workspace_response(workspace).await
    }

    async fn validate_workspace_provider_model(
        &self,
        workspace: &WorkspaceMutation,
    ) -> Result<(), agent_client_protocol::Error> {
        if let (Some(provider_id), Some(model_id)) = (
            workspace.default_provider.as_deref(),
            workspace.default_model.as_deref(),
        ) {
            self.validate_model_for_provider(provider_id, model_id)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn on_workspace_create_output_folder(
        &self,
        request: WorkspaceCreateOutputFolderRequest,
    ) -> Result<WorkspaceValidationResponse, agent_client_protocol::Error> {
        let validation = self
            .workspace_service
            .create_output_folder(&request.workspace_id, &request.output_folder_id)
            .await
            .map_err(workspace_request_error)?;
        Ok(WorkspaceValidationResponse { validation })
    }

    pub(super) async fn on_credential_profile_list(
        &self,
        _request: CredentialProfileListRequest,
    ) -> Result<CredentialProfileListResponse, agent_client_protocol::Error> {
        // Pick up providers configured since the last call. A failure here only
        // costs the alias profiles, so the stored ones are still listed.
        if let Err(error) = self.workspace_service.sync_global_alias_profiles().await {
            tracing::warn!(%error, "Failed to refresh provider credential profiles");
        }
        let profiles = self
            .workspace_service
            .listed_credential_profiles()
            .await
            .internal_err_ctx("Failed to list credential profiles")?;
        Ok(CredentialProfileListResponse { profiles })
    }

    pub(super) async fn on_credential_profile_create(
        &self,
        request: CredentialProfileCreateRequest,
    ) -> Result<CredentialProfileResponse, agent_client_protocol::Error> {
        let profile = self
            .workspace_service
            .create_profile(
                request.name,
                request.provider_or_service_id,
                request.auth_kind,
                request.non_secret_fields,
                request.secret_fields,
            )
            .await
            .invalid_params_err()?;
        Ok(CredentialProfileResponse { profile })
    }

    pub(super) async fn on_credential_profile_update(
        &self,
        request: CredentialProfileUpdateRequest,
    ) -> Result<CredentialProfileResponse, agent_client_protocol::Error> {
        let profile = self
            .workspace_service
            .update_profile(
                &request.profile_id,
                request.name,
                request.auth_kind,
                request.non_secret_fields,
                request.secret_fields,
                request.clear_secret_fields,
            )
            .await
            .invalid_params_err()?;
        Ok(CredentialProfileResponse { profile })
    }

    pub(super) async fn on_credential_profile_delete(
        &self,
        request: CredentialProfileDeleteRequest,
    ) -> Result<CredentialProfileDeleteResponse, agent_client_protocol::Error> {
        self.workspace_service
            .delete_profile(&request.profile_id, request.confirm_referenced)
            .await
            .invalid_params_err()?;
        Ok(CredentialProfileDeleteResponse { deleted: true })
    }

    pub(super) async fn on_credential_profile_usage(
        &self,
        request: CredentialProfileUsageRequest,
    ) -> Result<CredentialProfileUsageResponse, agent_client_protocol::Error> {
        let workspaces = self
            .workspace_service
            .profile_usage(&request.profile_id)
            .invalid_params_err()?
            .into_iter()
            .map(
                |(workspace_id, workspace_name)| CredentialProfileWorkspaceReference {
                    workspace_id,
                    workspace_name,
                },
            )
            .collect();
        Ok(CredentialProfileUsageResponse { workspaces })
    }

    pub(super) async fn on_credential_profile_test(
        &self,
        request: CredentialProfileTestRequest,
    ) -> Result<CredentialProfileTestResponse, agent_client_protocol::Error> {
        let profile = self
            .workspace_service
            .listed_credential_profiles()
            .await
            .internal_err_ctx("Failed to read credential profile")?
            .into_iter()
            .find(|profile| profile.id == request.profile_id)
            .ok_or_else(|| {
                agent_client_protocol::Error::invalid_params().data("credential profile not found")
            })?;
        Ok(CredentialProfileTestResponse {
            status: profile.status,
            supported: false,
        })
    }
}

#[cfg(test)]
mod workspace_error_tests {
    use super::*;

    #[tokio::test]
    async fn workspace_service_preserves_error_classes_at_the_handler_boundary() {
        let root = tempfile::tempdir().unwrap();
        let working = root.path().join("working");
        std::fs::create_dir(&working).unwrap();
        let service = crate::workspace::WorkspaceService::initialize(root.path(), &working)
            .await
            .unwrap();
        let missing = workspace_request_error(service.get("missing").unwrap_err());
        assert_eq!(
            missing.code,
            agent_client_protocol::Error::resource_not_found(None).code
        );
        let invalid = workspace_request_error(
            service
                .create(WorkspaceMutation::default())
                .await
                .unwrap_err(),
        );
        assert_eq!(
            invalid.code,
            agent_client_protocol::Error::invalid_params().code
        );
        std::fs::remove_file(root.path().join("workspaces/.workspaces.lock")).unwrap();
        std::fs::create_dir(root.path().join("workspaces/.workspaces.lock")).unwrap();
        let storage = workspace_request_error(service.get("missing").unwrap_err());
        assert_eq!(
            storage.code,
            agent_client_protocol::Error::internal_error().code
        );
        assert_ne!(missing.code, storage.code);
    }

    #[test]
    fn missing_validation_and_storage_errors_keep_distinct_codes() {
        let missing = workspace_request_error(
            crate::workspace::WorkspaceRequestError::NotFound("workspace").into(),
        );
        let invalid = workspace_request_error(
            crate::workspace::WorkspaceRequestError::Invalid("name cannot be empty".into()).into(),
        );
        let storage = workspace_request_error(anyhow::anyhow!("private storage/lock detail"));
        assert_eq!(
            missing.code,
            agent_client_protocol::Error::resource_not_found(None).code
        );
        assert_eq!(
            invalid.code,
            agent_client_protocol::Error::invalid_params().code
        );
        assert_eq!(
            storage.code,
            agent_client_protocol::Error::internal_error().code
        );
        assert!(!serde_json::to_string(&storage)
            .unwrap()
            .contains("private storage"));
    }
}
