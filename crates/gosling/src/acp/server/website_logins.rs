use super::*;
use crate::config::ConfigError;
use crate::website_logins::{self, WebsiteLogin, WebsiteLoginInput};

fn login_to_dto(login: WebsiteLogin, has_password: bool) -> WebsiteLoginDto {
    WebsiteLoginDto {
        placeholder: login.placeholder(),
        id: login.id,
        name: login.name,
        url: login.url,
        username: login.username,
        has_password,
    }
}

fn login_error(error: anyhow::Error) -> agent_client_protocol::Error {
    if error.downcast_ref::<ConfigError>().is_some() {
        agent_client_protocol::Error::internal_error().data(error.to_string())
    } else {
        agent_client_protocol::Error::invalid_params().data(error.to_string())
    }
}

impl GoslingAcpAgent {
    pub(super) async fn on_list_website_logins(
        &self,
        _req: WebsiteLoginListRequest,
    ) -> Result<WebsiteLoginListResponse, agent_client_protocol::Error> {
        let with_password = website_logins::ids_with_password().map_err(login_error)?;
        let logins = website_logins::list()
            .map_err(login_error)?
            .into_iter()
            .map(|login| {
                let has_password = with_password.contains(&login.id);
                login_to_dto(login, has_password)
            })
            .collect();
        Ok(WebsiteLoginListResponse { logins })
    }

    pub(super) async fn on_save_website_login(
        &self,
        req: WebsiteLoginSaveRequest,
    ) -> Result<WebsiteLoginSaveResponse, agent_client_protocol::Error> {
        let login = website_logins::save(WebsiteLoginInput {
            id: req.id,
            name: req.name,
            url: req.url,
            username: req.username,
            password: req.password,
        })
        .map_err(login_error)?;
        let has_password = website_logins::ids_with_password()
            .map_err(login_error)?
            .contains(&login.id);
        Ok(WebsiteLoginSaveResponse {
            login: login_to_dto(login, has_password),
        })
    }

    pub(super) async fn on_delete_website_login(
        &self,
        req: WebsiteLoginDeleteRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        website_logins::delete(&req.id).map_err(login_error)?;
        Ok(EmptyResponse {})
    }
}
