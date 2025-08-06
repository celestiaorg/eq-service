use celestia_types::{
    nmt::{Namespace, NamespaceProof},
    RowProof,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "host")]
mod error;
#[cfg(feature = "host")]
pub use error::{ErrorLabels, InclusionServiceError};

#[cfg(feature = "grpc")]
/// gRPC generated bindings
pub mod eqs {
    include!("generated/eqs.rs");
}

/// Newtype for data type in https://docs.rs/celestia-types/latest/celestia_types/struct.ShareProof.html
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawShare(pub [u8; 512]);

impl From<[u8; 512]> for RawShare {
    fn from(value: [u8; 512]) -> Self {
        RawShare(value)
    }
}
impl From<RawShare> for [u8; 512] {
    fn from(raw: RawShare) -> Self {
        raw.0
    }
}
impl From<celestia_types::Share> for RawShare {
    fn from(value: celestia_types::Share) -> Self {
        RawShare(*value.data())
    }
}

impl Serialize for RawShare {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RawShare {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Deserialize as Vec<u8>
        let vec = Vec::<u8>::deserialize(deserializer)?;
        if vec.len() != 512 {
            return Err(serde::de::Error::invalid_length(vec.len(), &"512 bytes"));
        }
        let mut arr = [0u8; 512];
        arr.copy_from_slice(&vec);
        Ok(RawShare(arr))
    }
}

/*
    For now, we only support ZKStackEqProofs
    These are used for Celestia integrations with Matter Labs' ZKStack
    TODO: Add support for Payy Celestia integration
*/
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZKStackEqProofInput {
    pub blob_data: Vec<u8>,
    pub shares_data: Vec<RawShare>,
    pub blob_namespace: Namespace,
    pub nmt_multiproofs: Vec<NamespaceProof>,
    pub row_root_multiproof: RowProof,
    pub data_root: [u8; 32],
    pub keccak_hash: [u8; 32],
    // batch_number and chain_id are passed through to prevent proofs from being replayed
    pub batch_number: u32,
    pub chain_id: u64,
}

/// Expecting bytes:
/// (keccak_hash: [u8; 32], pub data_root: [u8; 32])
pub struct ZKStackEqProofOutput {
    pub keccak_hash: [u8; 32],
    pub data_root: [u8; 32],
    pub batch_number: u32,
    pub chain_id: u64,
}
impl ZKStackEqProofOutput {
    // Simple encoding, rather than use any Ethereum libraries
    pub fn to_vec(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&self.keccak_hash);
        encoded.extend_from_slice(&self.data_root);
        encoded.extend_from_slice(&self.batch_number.to_le_bytes());
        encoded.extend_from_slice(&self.chain_id.to_le_bytes());
        encoded
    }

    #[cfg(feature = "host")]
    pub fn from_bytes(data: &[u8]) -> Result<Self, InclusionServiceError> {
        if data.len() != 76 {
            return Err(InclusionServiceError::OutputDeserializationError);
        }
        let decoded = ZKStackEqProofOutput {
            keccak_hash: data[0..32]
                .try_into()
                .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            data_root: data[32..64]
                .try_into()
                .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            batch_number: u32::from_le_bytes(
                data[64..68]
                    .try_into()
                    .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            ),
            chain_id: u64::from_le_bytes(
                data[68..76]
                    .try_into()
                    .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            ),
        };
        Ok(decoded)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[cfg(feature = "host")]
    fn test_serialization() {
        let output = ZKStackEqProofOutput {
            keccak_hash: [0; 32],
            data_root: [0; 32],
            batch_number: 0u32,
            chain_id: 0u64,
        };
        let encoded = output.to_vec();
        let decoded = ZKStackEqProofOutput::from_bytes(&encoded).unwrap();
        assert_eq!(output.keccak_hash, decoded.keccak_hash);
        assert_eq!(output.data_root, decoded.data_root);
    }
}
