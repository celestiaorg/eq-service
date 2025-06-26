use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue, LabelValueEncoder};
use serde::{Deserialize, Serialize};
use std::fmt::Error as FmtError;
use std::fmt::Write;
use thiserror::Error;

#[derive(PartialEq, Eq, Clone, Hash, Error, Debug, Serialize, Deserialize)]
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

impl EncodeLabelValue for InclusionServiceError {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), FmtError> {
        use InclusionServiceError::*;
        let name = match self {
            MissingBlobIndex => "MissingBlobIndex",
            FailedShareRangeProofSanityCheck => "FailedShareRangeProofSanityCheck",
            KeccakHashConversion => "KeccakHashConversion",
            RowRootVerificationFailed => "RowRootVerificationFailed",
            ShareConversionError(_) => "ShareConversionError",
            InternalError(_) => "InternalError",
            ZkClientError(_) => "ZkClientError",
            DaClientError(_) => "DaClientError",
            InvalidParameter(_) => "InvalidParameter",
            OutputDeserializationError => "OutputDeserializationError",
        };
        encoder.write_str(name)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ErrorLabels {
    pub error_type: InclusionServiceError,
}
