#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Digest, Sha256};

use chacha_lib::chacha;

pub fn main() {
    let key = sp1_zkvm::io::read_vec(); // 32 bytes
    let nonce = sp1_zkvm::io::read_vec(); // 12 bytes
    // The plaintext to be encrypted _in place_
    let mut buffer = sp1_zkvm::io::read_vec(); // 12 bytes

    // Commit to buffer (plaintext) hash
    //
    // ## Note
    // The EVM has KECCAK256 opcode (Solidity `keccak256()`)
    // KECCAK256 = 30 gas base & per 32 bytes word = 6 gas
    // So SHA3 is most performat to choose for EVM.
    //
    // BUT the cycle count is significantly higher for SHA3 (even accelerated)
    // so we choose to use SHA2, for slightly higher on chain verification gas costs.
    let plaintext_hash = Sha256::digest(buffer.as_slice());
    // Hash plaintext & commit
    sp1_zkvm::io::commit_slice(&plaintext_hash); // 32 bytes

    // FIXME // TODO:
    // Hash key and/or nonce & commit?

    // Encrypt and commit
    // Incorrect sized buffers passed in are unacceptable, and thus panic.
    chacha(
        &key.try_into().expect("key=32B"),
        &nonce.try_into().expect("nonce=12B"),
        &mut buffer,
    );

    sp1_zkvm::io::commit_slice(&buffer);
}
