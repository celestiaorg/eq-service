use eq_common::{InclusionServiceError, KeccakInclusionToDataRootProofInput};
use eq_sdk::types::BlobId;
use serde::{Deserialize, Serialize};
use sp1_sdk::SP1ProofWithPublicValues;

use crate::SuccNetJobId;

/// A job for the service, mapped to a [BlobId]
pub type Job = BlobId;

/// Used as a [Job] state machine for the eq-service.
///
/// Should map 1to1 with [ResponseStatus](eq_common::eqs::get_keccak_inclusion_response::ResponseValue)
/// for consistency in internal state and what is reported by the RPC.
#[derive(Serialize, Deserialize)]
pub enum JobStatus {
    /// DA inclusion proof data is being collected
    DataAvailabilityPending,
    /// DA inclusion is processed and ready to send to the ZK prover
    DataAvailable(KeccakInclusionToDataRootProofInput),
    /// A ZK prover job had been requested, awaiting response
    ZkProofPending(SuccNetJobId),
    /// A ZK proof is ready, and the [Job] is complete
    // For now we'll use the SP1ProofWithPublicValues as the proof
    // Ideally we only want the public values + whatever is needed to verify the proof
    // They don't seem to provide a type for that.
    ZkProofFinished(SP1ProofWithPublicValues),
    /// A wrapper for any [InclusionServiceError], with:
    /// - Option = None                        --> Permanent failure
    /// - Option = Some(\<retry-able status\>) --> Retry is possible, with a JobStatus state to retry with
    Failed(InclusionServiceError, Option<Box<JobStatus>>),
}

impl std::fmt::Debug for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::DataAvailabilityPending => write!(f, "DataAvailabilityPending"),
            JobStatus::DataAvailable(_) => write!(f, "DataAvailable"),
            JobStatus::ZkProofPending(_) => write!(f, "ZkProofPending"),
            JobStatus::ZkProofFinished(_) => write!(f, "ZkProofFinished"),
            JobStatus::Failed(_, _) => write!(f, "Failed"),
        }
    }
}
