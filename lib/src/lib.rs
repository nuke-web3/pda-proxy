// Include the binary input file
pub const INPUT_BYTES: &[u8] = include_bytes!("../../static/proof_input_example.bin");

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};

/// Encrypt a buffer in-place using [ChaCha20](https://en.wikipedia.org/wiki/Salsa20#ChaCha_variant).
///
/// ## Important Notice
///
/// This intentionally omits the Poly1305 MAC steps to reduce cycle count.
/// It is intended to be used exclusively in an environment that guarantees integrity and prevents
/// man-in-the-middle manipulation of the ciphertext.
/// A zkVM proving correct execution of this function provides these properties.
///
pub fn chacha(key: &[u8; 32], nonce: &[u8; 12], buffer: &mut [u8]) {
    let mut cipher = ChaCha20::new(key.into(), nonce.into());
    cipher.apply_keystream(buffer);
}

// Helper to format bytes as hex for pretty printing
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let digest_hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    digest_hex
}

// Only compile this when the standard library is available
#[cfg(feature = "std")]
mod std_only {
    use rand::{TryRngCore, rngs::OsRng};

    pub fn random_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        OsRng.try_fill_bytes(&mut nonce).expect("Rng->buffer");
        nonce
    }
}

#[cfg(feature = "std")]
pub use std_only::random_nonce;
