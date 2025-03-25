use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Error, Debug, Serialize, Deserialize)]
pub enum InclusionServiceError {
    #[error("Blob index not found")]
    MissingBlobIndex,

    #[error("Inclusion proof sanity check failed")]
    FailedShareRangeProofSanityCheck,

    #[error("Failed to convert keccak hash to array")]
    KeccakHashConversion,

    #[error("Failed to verify row root inclusion multiproof")]
    RowRootVerificationFailed,

    #[error("Failed to convert shares to blob: {0}")]
    ShareConversionError(String),

    #[error("Failed with: {0}")]
    InternalError(String),

    #[error("{0}")]
    ZkClientError(String),

    #[error("{0}")]
    DaClientError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Failed to deserialize KeccakInclusionToDataRootProofOutput")]
    OutputDeserializationError,
}
