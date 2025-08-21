use crate::syncnode::ClientConfig;
use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use derive_more::Constructor;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject, ErrorObjectOwned},
};
use kona_supervisor_rpc::SupervisorAdminApiServer;
use thiserror::Error;
use tokio::sync::{mpsc::Sender, oneshot};
use tracing::warn;

/// Error types for Supervisor Admin RPC operations.
#[derive(Debug, Error)]
pub enum AdminError {
    /// Indicates that the JWT secret is invalid.
    #[error("invalid jwt secret: {0}")]
    InvalidJwtSecret(String),

    /// Indicates that the request to the admin channel failed to send.
    #[error("failed to send admin request")]
    SendFailed,

    /// Indicates that the admin request timed out.
    #[error("admin request timed out")]
    Timeout,

    /// Indicates a service error occurred during processing the request.
    #[error("service error: {0}")]
    ServiceError(String),
}

impl From<AdminError> for ErrorObjectOwned {
    fn from(err: AdminError) -> Self {
        match err {
            // todo: handle these errors more gracefully
            AdminError::InvalidJwtSecret(_) |
            AdminError::SendFailed |
            AdminError::Timeout |
            AdminError::ServiceError(_) => ErrorObjectOwned::from(ErrorCode::InternalError),
        }
    }
}

/// Represents Admin Request types
#[derive(Debug)]
pub enum AdminRequest {
    /// Adds a new L2 RPC to the Supervisor.
    AddL2Rpc {
        /// The configuration for the L2 RPC client.
        cfg: ClientConfig,
        /// The response channel to send the result back.
        resp: oneshot::Sender<Result<(), AdminError>>,
    },
}

/// Supervisor Admin RPC interface
#[derive(Debug, Constructor)]
pub struct AdminRpc {
    admin_tx: Sender<AdminRequest>,
}

#[async_trait]
impl SupervisorAdminApiServer for AdminRpc {
    /// Adds L2RPC to the supervisor.
    async fn add_l2_rpc(&self, url: String, secret: String) -> RpcResult<()> {
        let (resp_tx, resp_rx) = oneshot::channel();

        let jwt_secret = JwtSecret::from_hex(secret).map_err(|err| {
            warn!(target: "supervisor::admin_rpc", %url, %err, "Failed to decode JWT secret");
            ErrorObject::from(AdminError::InvalidJwtSecret(err.to_string()))
        })?;

        let request = AdminRequest::AddL2Rpc {
            cfg: ClientConfig { url: url.clone(), jwt_secret },
            resp: resp_tx,
        };

        self.admin_tx.send(request).await.map_err(|err| {
            warn!(target: "supervisor::admin_rpc", %url, %err, "Failed to send AdminRequest");
            ErrorObject::from(AdminError::SendFailed)
        })?;

        // todo: add a timeout for the response
        let res = resp_rx.await.map_err(|err| {
            warn!(target: "supervisor::admin_rpc", %url, %err, "Failed to process AdminRequest");
            ErrorObject::from(AdminError::Timeout)
        })?;

        match res {
            Ok(()) => Ok(()),
            Err(err) => Err(ErrorObject::from(err)),
        }
    }
}
