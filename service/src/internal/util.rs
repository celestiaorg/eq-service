use serde::{Deserialize, Serialize};

/// A Succinct Prover Network request ID.
/// See: https://docs.succinct.xyz/docs/generating-proofs/prover-network/usage
pub type SuccNetJobId = [u8; 32];

/// A SHA3 256 bit hash of a zkVM program's ELF.
pub type SuccNetProgramId = [u8; 32];

#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub struct SP1ProofSetup {
    pub pk: sp1_sdk::SP1ProvingKey,
    pub vk: sp1_sdk::SP1VerifyingKey,
}

impl From<(sp1_sdk::SP1ProvingKey, sp1_sdk::SP1VerifyingKey)> for SP1ProofSetup {
    fn from(tuple: (sp1_sdk::SP1ProvingKey, sp1_sdk::SP1VerifyingKey)) -> Self {
        Self {
            pk: tuple.0,
            vk: tuple.1,
        }
    }
}
