#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Digest, Sha256};

use zkvm_common::{NONCE_LEN, chacha};

pub fn main() {
    let key = sp1_zkvm::io::read_vec(); // 32 bytes
    let nonce = sp1_zkvm::io::read_vec(); // 12 bytes
    // The plaintext to be encrypted _in place_
    let mut buffer = sp1_zkvm::io::read_vec(); // ~1M bytes

    // Commit to key used, providing a fixed UID as first bytes in proof data.
    // So now we have a tag we can look for in filtering DA data latter.
    let key_hash = Sha256::digest(key.as_slice());
    sp1_zkvm::io::commit_slice(&key_hash); // 32 bytes

    // Commit to nonce used, this is safe so long as we NEVER reuse a nonce!
    // NOTE: without reading nonce in the next line, it is optimized out or unread = zeros
    // So for a few cycles we ensure it's not dropped also enforce it's the right length
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("nonce=12B");
    sp1_zkvm::io::commit_slice(&nonce);

    // Commit to buffer (plaintext) hash
    //
    // ## Note
    // The EVM has KECCAK256 opcode (Solidity `keccak256()`)
    // KECCAK256 = 30 gas base & per 32 bytes word = 6 gas
    // So SHA3 is most performant to choose for EVM.
    //
    // BUT the cycle count is significantly higher for SHA3 (even accelerated)
    // so we choose to use SHA2, for slightly higher on chain verification gas costs.
    let plaintext_hash = Sha256::digest(buffer.as_slice());
    // Hash plaintext & commit
    sp1_zkvm::io::commit_slice(&plaintext_hash); // 32 bytes

    // Encrypt and commit
    // Incorrect sized buffers passed in are unacceptable, and thus panic.
    chacha(&key.try_into().expect("key=32B"), &nonce, &mut buffer);

    sp1_zkvm::io::commit_slice(&buffer); // ~1M bytes
}
