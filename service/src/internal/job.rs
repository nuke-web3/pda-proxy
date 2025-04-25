use crate::PdaRunnerError;

use serde::{Deserialize, Serialize};
use sp1_sdk::SP1ProofWithPublicValues;

use crate::SuccNetJobId;

/// A job for the service, mapped to a...
/// TODO
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub anchor: Anchor,
    pub input: Input,
}

/// A (set of) input(s) for the zkVM
/// TODO: what should structure be?
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Input {
    pub data: Vec<u8>,
}

/// A committment to the input, likley a hash or related fingerprint.
/// TODO: what should structure be?
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Anchor {
    pub data: Vec<u8>,
}

impl AsRef<[u8]> for Anchor {
    fn as_ref(&self) -> &[u8] {
        self.data.as_slice()
    }
}

/// Used as a [Job] state machine for the PDA service.
#[derive(Serialize, Deserialize)]
pub enum JobStatus {
    /// A ZK prover job had been started, awaiting completion
    LocalZkProofPending,
    /// A ZK prover job had been requested, awaiting response
    RemoteZkProofPending(SuccNetJobId),
    /// A ZK proof is ready, and the [Job] is complete
    // For now we'll use the SP1ProofWithPublicValues as the proof
    // Ideally we only want the public values + whatever is needed to verify the proof
    // They don't seem to provide a type for that.
    ZkProofFinished(SP1ProofWithPublicValues),
    /// A wrapper for any [PdaServiceError], with:
    /// - Option = None                        --> Permanent failure
    /// - Option = Some(\<retry-able status\>) --> Retry is possible, with a JobStatus state to retry with
    Failed(PdaRunnerError, Option<Box<JobStatus>>),
}

impl std::fmt::Debug for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::LocalZkProofPending => write!(f, "LocalZkProofPending"),
            JobStatus::RemoteZkProofPending(_) => write!(f, "ZkProofPending"),
            JobStatus::ZkProofFinished(_) => write!(f, "ZkProofFinished"),
            JobStatus::Failed(_, _) => write!(f, "Failed"),
        }
    }
}
