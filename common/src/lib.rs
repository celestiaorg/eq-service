use celestia_types::{
    consts::appconsts::{
        CONTINUATION_SPARSE_SHARE_CONTENT_SIZE, FIRST_SPARSE_SHARE_CONTENT_SIZE, NAMESPACE_SIZE,
        SEQUENCE_LEN_BYTES, SHARE_INFO_BYTES, SHARE_SIZE, SIGNER_SIZE,
    },
    ShareProof,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

#[cfg(feature = "host")]
mod error;
#[cfg(feature = "host")]
pub use error::{ErrorLabels, InclusionServiceError};

#[cfg(feature = "grpc")]
/// gRPC generated bindings
pub mod eqs {
    include!("generated/eqs.rs");
}

/*
    For now, we only support ZKStackEqProofs
    These are used for Celestia integrations with Matter Labs' ZKStack
    TODO: Add support for Payy Celestia integration
*/
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZKStackEqProofInput {
    pub share_proof: ShareProof,
    pub share_version: bool,
    pub tail_padding: usize,
    pub data_availability_root: [u8; 32],
    pub batch_number: u32,
    pub chain_id: u64,
}

pub struct ZKStackEqProofOutput {
    pub keccak_hash: [u8; 32],
    pub data_availability_root: [u8; 32],
    pub batch_number: u32,
    pub chain_id: u64,
}

impl ZKStackEqProofOutput {
    // Simple encoding, rather than use any Ethereum libraries
    pub fn to_vec(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&self.keccak_hash);
        encoded.extend_from_slice(&self.data_availability_root);
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
            data_availability_root: data[32..64]
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

/// Computes Keccak-256 over the blob bytes reconstructed from `raw_shares`,
/// stopping at the end of the last share minus `tail_padding`.
///
/// See: https://celestiaorg.github.io/celestia-app/shares.html
///
/// - `share_version`: false => v0; true => v1 (adds SIGNER in first share)
/// - `tail_padding`: number of *padding bytes in the final share*
///   (0 means last share is completely full of data)
///
/// Caller MUST guarantee:
/// - `raw_shares` is non-empty and belongs to one blob
/// - `share_version` is correct for the sequence
/// - `tail_padding < FIRST_SPARSE_SHARE_CONTENT_SIZE` when `raw_shares.len() == 1`
/// - `tail_padding < CONTINUATION_SPARSE_SHARE_CONTENT_SIZE` when `raw_shares.len() >= 2`
/// - Sufficient shares for the underlying data length
///
/// Violations may panic (intended for performance).
pub fn compute_share_raw_data_keccak(
    raw_shares: &[[u8; SHARE_SIZE]],
    share_version: bool,
    tail_padding: usize,
) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    let n = raw_shares.len();
    let off_first = first_data_offset(share_version);

    if n == 1 {
        // Single-share blob: only first sparse-share payload is present.
        let take = FIRST_SPARSE_SHARE_CONTENT_SIZE - tail_padding;
        let s0 = raw_shares[0].as_ref();
        hasher.update(&s0[off_first..off_first + take]);
        return hasher.finalize().into();
    }

    // n >= 2
    // 1) Full first-share payload
    {
        let s0 = raw_shares[0].as_ref();
        let end0 = off_first + FIRST_SPARSE_SHARE_CONTENT_SIZE;
        hasher.update(&s0[off_first..end0]);
    }

    let off_cont = NAMESPACE_SIZE + SHARE_INFO_BYTES;
    let last_take = CONTINUATION_SPARSE_SHARE_CONTENT_SIZE - tail_padding;
    let last_full = tail_padding == 0;

    // 2) Full continuation shares in the middle (and optionally the last if full)
    let full_end = if last_full { n } else { n - 1 };
    for i in 1..full_end {
        let si = raw_shares[i].as_ref();
        let endi = off_cont + CONTINUATION_SPARSE_SHARE_CONTENT_SIZE;
        hasher.update(&si[off_cont..endi]);
    }

    // 3) Tail of the final continuation share (only if not full)
    if !last_full {
        let slast = raw_shares[n - 1].as_ref();
        hasher.update(&slast[off_cont..off_cont + last_take]);
    }

    hasher.finalize().into()
}

#[inline(always)]
fn first_data_offset(version: bool) -> usize {
    let base = NAMESPACE_SIZE + SHARE_INFO_BYTES + SEQUENCE_LEN_BYTES;
    if version {
        base + SIGNER_SIZE
    } else {
        base
    }
}

/// Returns the number of padding bytes in the **final share** for a blob's raw data of `total_len` bytes.
///
/// Layout:
/// - First share carries up to `FIRST_SPARSE_SHARE_CONTENT_SIZE` data bytes
/// - Each continuation share carries up to `CONTINUATION_SPARSE_SHARE_CONTENT_SIZE` data bytes
#[inline(always)]
pub fn tail_padding_for_len(total_len: usize) -> usize {
    if total_len <= FIRST_SPARSE_SHARE_CONTENT_SIZE {
        // All data fits in the first share
        FIRST_SPARSE_SHARE_CONTENT_SIZE - total_len
    } else {
        let rem = total_len - FIRST_SPARSE_SHARE_CONTENT_SIZE;
        let tail_bytes = rem % CONTINUATION_SPARSE_SHARE_CONTENT_SIZE;
        if tail_bytes == 0 {
            0 // last continuation share is completely full
        } else {
            CONTINUATION_SPARSE_SHARE_CONTENT_SIZE - tail_bytes
        }
    }
}

// NOTE: we only support share versions 0 and 1 - ALL future versions will panic
// for the zkVM proof, we should never be able to create a valid proof with
// forged versions/mangled shares, as the ShareProof.verify will fail
#[cfg(feature = "host")]
#[inline(always)]
pub fn exact_u8_to_bool(val: u8) -> bool {
    match val {
        0 => false,
        1 => true,
        _ => panic!("u8->bool not 0 or 1: {}", val),
    }
}

#[cfg(all(test, feature = "host"))]
mod test {
    use super::*; // brings compute_blob_keccak etc. into scope
    use celestia_types::{nmt::Namespace, AppVersion, Blob};
    use rand::{rngs::StdRng, Rng, SeedableRng};
    use sha3::{Digest, Keccak256};

    #[test]
    fn test_serialization() {
        let output = ZKStackEqProofOutput {
            keccak_hash: [0; 32],
            data_availability_root: [0; 32],
            batch_number: 0u32,
            chain_id: 0u64,
        };
        let encoded = output.to_vec();
        let decoded = ZKStackEqProofOutput::from_bytes(&encoded).unwrap();
        assert_eq!(output.keccak_hash, decoded.keccak_hash);
        assert_eq!(
            output.data_availability_root,
            decoded.data_availability_root
        );
    }

    #[test]
    fn keccak_of_data_matches_keccak_from_blob_shares_randomized() {
        // deterministic RNG so test is reproducible
        let mut rng = StdRng::seed_from_u64(0xCE1E5);

        // Run multiple randomized trials
        for _ in 0..1 {
            // Random length in [100, 1_000_000]
            let len = rng.gen_range(100usize..=1_000_000usize);
            let mut data = vec![0u8; len];
            rng.fill(&mut data[..]);

            // Random share version in {0,1}
            let share_version = rng.gen_bool(0.5);

            // Namespace for the blob
            let ns = Namespace::new_v0(&[1, 2, 3, 4, 5]).expect("invalid namespace");

            // Build the blob using the chosen app/share version
            let blob =
                Blob::new(ns, data.clone(), AppVersion::V6).expect("blob construction failed");

            // Turn blob into shares (exact SHARE_SIZE each)
            let shares: Vec<[u8; SHARE_SIZE]> = blob
                .to_shares()
                .expect("invalid blob->shares")
                .iter()
                .map(|s| s.data().to_owned())
                .collect();

            // Sanity: derive the version from the first share's info byte; it should match
            let first = shares.first().expect("no shares emitted");
            let info = first[NAMESPACE_SIZE]; // info byte follows namespace
            let derived_version_byte = info >> 1; // upper 7 bits = share version
            let derived_version_bool = exact_u8_to_bool(derived_version_byte);
            assert_eq!(
                derived_version_bool, share_version,
                "share version mismatch"
            );

            // Expected hash: Keccak256 over the original raw data
            let expected = {
                let mut h = Keccak256::new();
                h.update(&data);
                <[u8; 32]>::from(h.finalize())
            };

            let tail_padding = tail_padding_for_len(data.len());

            // Hash via the share-based pipeline
            let got = compute_share_raw_data_keccak(&shares, derived_version_bool, tail_padding);

            assert_eq!(
                got, expected,
                "keccak(blob shares) != keccak(raw data) for len={len}, version={share_version}"
            );
        }
    }
}
