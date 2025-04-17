use log::info;
use serde::{Deserialize, Serialize};
use tokio::signal::{
    self,
    unix::{SignalKind, signal as unix_signal},
};
/// A Succinct Prover Network request ID.
/// See: <https://docs.succinct.xyz/docs/sp1/generating-proofs/prover-network/usage>
pub type SuccNetJobId = [u8; 32];

/// A SHA2 256 bit hash of a zkVM program's ELF.
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

/// Account for shutdown signals, `ctrl+c` and other common Unix signals.
#[cfg(target_os = "linux")]
pub async fn wait_shutdown_signals() {
    // Wait for the Ctrl+C signal.
    let ctrl_c = signal::ctrl_c();

    // Create listeners for SIGTERM, SIGINT, and SIGHUP.
    let mut sigterm =
        unix_signal(SignalKind::terminate()).expect("Failed to set up SIGTERM listener");
    let mut sigint =
        unix_signal(SignalKind::interrupt()).expect("Failed to set up SIGINT listener");
    let mut sighup = unix_signal(SignalKind::hangup()).expect("Failed to set up SIGHUP listener");

    info!("Listening for shutdown signals (ctrl+c, SIGTERM, SIGINT, SIGHUP)");

    // Wait for any of the signals to occur.
    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C.");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM.");
        }
        _ = sigint.recv() => {
            info!("Received SIGINT.");
        }
        _ = sighup.recv() => {
            info!("Received SIGHUP.");
        }
    }
}

use rand::{TryRngCore, rngs::OsRng};

pub fn random_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    OsRng.try_fill_bytes(&mut nonce).expect("Rng->buffer");
    nonce
}

/// Account for shutdown signals, `ctrl+c`
///
/// TODO: handle OS specific signals
#[cfg(not(target_os = "linux"))]
pub async fn wait_shutdown_signals() {
    // Wait for the Ctrl+C signal.
    let ctrl_c = signal::ctrl_c();

    info!("Listening for shutdown signals (ctrl+c)");

    // Wait for any of the signals to occur.
    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C.");
        }
    }
}
