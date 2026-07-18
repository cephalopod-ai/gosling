use super::*;
use crate::workspace::WorkspaceMutation;

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
        let workspace = self
            .workspace_service
            .create(request.workspace)
            .await
            .invalid_params_err()?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_update(
        &self,
        request: WorkspaceUpdateRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let workspace = self
            .workspace_service
            .update(&request.workspace_id, request.workspace)
            .await
            .invalid_params_err()?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_duplicate(
        &self,
        request: WorkspaceDuplicateRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let workspace = self
            .workspace_service
            .duplicate(&request.workspace_id)
            .await
            .invalid_params_err()?;
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
            .invalid_params_err()?;
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
            .invalid_params_err()?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_validate(
        &self,
        request: WorkspaceValidateRequest,
    ) -> Result<WorkspaceValidationResponse, agent_client_protocol::Error> {
        let validation = self
            .workspace_service
            .validate(&request.workspace)
            .invalid_params_err()?;
        Ok(WorkspaceValidationResponse { validation })
    }

    pub(super) async fn on_workspace_export(
        &self,
        request: WorkspaceExportRequest,
    ) -> Result<WorkspaceExportResponse, agent_client_protocol::Error> {
        let document = self
            .workspace_service
            .export(&request.workspace_id)
            .invalid_params_err()?;
        Ok(WorkspaceExportResponse { document })
    }

    pub(super) async fn on_workspace_import(
        &self,
        request: WorkspaceImportRequest,
    ) -> Result<WorkspaceResponse, agent_client_protocol::Error> {
        let workspace = self
            .workspace_service
            .import(&request.document)
            .await
            .invalid_params_err()?;
        self.workspace_response(workspace).await
    }

    pub(super) async fn on_workspace_create_output_folder(
        &self,
        request: WorkspaceCreateOutputFolderRequest,
    ) -> Result<WorkspaceValidationResponse, agent_client_protocol::Error> {
        let validation = self
            .workspace_service
            .create_output_folder(&request.workspace_id, &request.output_folder_id)
            .await
            .invalid_params_err()?;
        Ok(WorkspaceValidationResponse { validation })
    }

    pub(super) async fn on_credential_profile_list(
        &self,
        _request: CredentialProfileListRequest,
    ) -> Result<CredentialProfileListResponse, agent_client_protocol::Error> {
        let profiles = self
            .workspace_service
            .credential_profiles()
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
            .credential_profiles()
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
