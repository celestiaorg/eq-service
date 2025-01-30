use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Error, Debug, Serialize, Deserialize)]
pub enum InclusionServiceError {
    #[error("Blob index not found")]
    MissingBlobIndex,

    #[error("Failed to convert keccak hash to array")]
    KeccakHashConversion,

    #[error("Failed to verify row root inclusion multiproof")]
    RowRootVerificationFailed,

    #[error("Failed to convert shares to blob: {0}")]
    ShareConversionError(String),

    #[error("Failed to create inclusion proof input: {0}")]
    GeneralError(String),

    #[error("Failed to query Celestia: {0}")]
    CelestiaError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}
