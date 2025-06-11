use core::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Error, Serialize, Deserialize)]
pub enum PdaRunnerError {
    #[error("Service: {0}")]
    InternalError(String),

    #[error("ZK Prover: {0}")]
    ZkClientError(String),

    #[error("Data Availability Node: {0}")]
    DaClientError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

// This removes a layer of escape chars for json formatting
impl fmt::Debug for PdaRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdaRunnerError::InternalError(s) => write!(f, "InternalError({})", s),
            PdaRunnerError::ZkClientError(s) => write!(f, "ZkClientError({})", s),
            PdaRunnerError::DaClientError(s) => write!(f, "DaClientError({})", s),
            PdaRunnerError::InvalidParameter(s) => write!(f, "InvalidParameter({})", s),
        }
    }
}
