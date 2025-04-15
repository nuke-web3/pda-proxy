use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Error, Debug, Serialize, Deserialize)]
pub enum PdaServiceError {
    #[error("Service: {0}")]
    InternalError(String),

    #[error("ZK Prover: {0}")]
    ZkClientError(String),

    #[error("Data Availability Node: {0}")]
    DaClientError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}
