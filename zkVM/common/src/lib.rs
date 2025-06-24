// Include the binary input file
/// For reproducible builds, you need `cargo prove --docker`
#[cfg(feature = "reproducible-elf")]
pub const CHACHA_ELF: &[u8] = include_bytes!(
    "../../../target/elf-compilation/docker/riscv32im-succinct-zkvm-elf/release/chacha-program"
);

#[cfg(not(feature = "reproducible-elf"))]
pub const CHACHA_ELF: &[u8] = include_bytes!(
    "../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/chacha-program"
);

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};

pub const KEY_LEN: usize = 32;
pub const HASH_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const HEADER_LEN: usize = HASH_LEN + NONCE_LEN + HASH_LEN;

/// Encrypt or Decrypt a buffer in-place using [ChaCha20](https://en.wikipedia.org/wiki/Salsa20#ChaCha_variant).
///
/// ## Important Notice
///
/// This intentionally omits the Poly1305 MAC steps to reduce cycle count.
/// It is intended to be used exclusively in an environment that guarantees integrity and prevents
/// man-in-the-middle manipulation of the ciphertext.
/// A zkVM proving correct execution of this function provides these properties.
///
pub fn chacha(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], buffer: &mut [u8]) {
    let mut cipher = ChaCha20::new(key.into(), nonce.into());
    cipher.apply_keystream(buffer);
}

#[cfg(feature = "std")]
pub mod std_only {
    use super::*;
    use core::fmt;
    use rand::{TryRngCore, rngs::OsRng};
    use sha2::{Digest, Sha256};

    pub struct ZkvmOutput<'a> {
        pub privkey_hash: [u8; HASH_LEN],
        pub nonce: [u8; NONCE_LEN],
        pub plaintext_hash: [u8; HASH_LEN],
        pub ciphertext: &'a [u8],
    }

    impl<'a> ZkvmOutput<'a> {
        pub fn from_bytes(data: &'a [u8]) -> Result<Self, &'static str> {
            if data.len() < HEADER_LEN {
                return Err("Input too short for header");
            }

            let (header, ciphertext) = data.split_at(HEADER_LEN);

            let mut privkey_hash = [0u8; HASH_LEN];
            privkey_hash.copy_from_slice(&header[..HASH_LEN]);

            let mut nonce = [0u8; NONCE_LEN];
            nonce.copy_from_slice(&header[HASH_LEN..HASH_LEN + NONCE_LEN]);

            let mut plaintext_hash = [0u8; HASH_LEN];
            plaintext_hash.copy_from_slice(&header[HASH_LEN + NONCE_LEN..HEADER_LEN]);

            Ok(ZkvmOutput {
                privkey_hash,
                nonce,
                plaintext_hash,
                ciphertext,
            })
        }
    }

    impl core::fmt::Debug for ZkvmOutput<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let privkey_hash_hex = bytes_to_hex(&self.privkey_hash);
            let nonce_hex = bytes_to_hex(&self.nonce);
            let plaintext_hash_hex = bytes_to_hex(&self.plaintext_hash);

            let mut hasher = Sha256::new();
            hasher.update(self.ciphertext);
            let ciphertext_hash = hasher.finalize();
            let ciphertext_hash_hex = bytes_to_hex(&ciphertext_hash);

            f.debug_struct("ZkvmOutput")
                .field("privkey_hash", &privkey_hash_hex)
                .field("nonce", &nonce_hex)
                .field("plaintext_hash", &plaintext_hash_hex)
                .field("ciphertext_sha256", &ciphertext_hash_hex)
                .finish()
        }
    }

    /// Helper to get a OsRng nonce of correct length
    pub fn random_nonce() -> [u8; NONCE_LEN] {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.try_fill_bytes(&mut nonce).expect("Rng->buffer");
        nonce
    }

    /// Helper to format bytes as hex for pretty printing
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        let digest_hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        digest_hex
    }
}

#[cfg(feature = "std")]
pub use std_only::random_nonce;
