use eq_common::{InclusionServiceError, ZKStackEqProofInput};
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
    DataAvailable(ZKStackEqProofInput),
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

#[cfg(test)]
mod tests {
    use super::*;
    use celestia_types::{nmt::Namespace, RowProof};

    #[test]
    fn test_job_status_serde_bincode() {
        // Create test instances of each variant that can be easily constructed
        let pending = JobStatus::DataAvailabilityPending;

        // Load test vector from json file
        let test_vector = std::fs::read_to_string("test_vector.json").unwrap();
        let test_input: ZKStackEqProofInput = serde_json::from_str(&test_vector).unwrap();
        let available = JobStatus::DataAvailable(test_input);

        let proof_pending = JobStatus::ZkProofPending([4u8; 32]);
        
        let failed_permanent = JobStatus::Failed(
            InclusionServiceError::InternalError("test error".to_string()),
            None
        );
        
        let failed_retryable = JobStatus::Failed(
            InclusionServiceError::InternalError("test error".to_string()),
            Some(Box::new(JobStatus::DataAvailabilityPending))
        );

        // Test each variant that can be easily constructed
        let test_statuses = vec![
            pending,
            available,
            proof_pending,
            failed_permanent,
            failed_retryable
        ];

        for status in test_statuses {
            let serialized = bincode::serialize(&status).unwrap();
            let deserialized: JobStatus = bincode::deserialize(&serialized).unwrap();

            match (&status, &deserialized) {
                (JobStatus::DataAvailabilityPending, JobStatus::DataAvailabilityPending) => (),
                (JobStatus::ZkProofPending(a), JobStatus::ZkProofPending(b)) => assert_eq!(a, b),
                (JobStatus::DataAvailable(a), JobStatus::DataAvailable(b)) => {
                    assert_eq!(a.data, b.data);
                    assert_eq!(a.batch_number, b.batch_number);
                    assert_eq!(a.chain_id, b.chain_id);
                },
                (JobStatus::Failed(e1, r1), JobStatus::Failed(e2, r2)) => {
                    assert_eq!(e1.to_string(), e2.to_string());
                    assert_eq!(r1.is_some(), r2.is_some());
                },
                _ => panic!("Deserialized variant does not match original")
            }
        }
    }
}
