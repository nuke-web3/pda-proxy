use sp1_sdk::{HashableKey, Prover, ProverClient};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// For reproducible builds, you need `cargo prove --docker`
pub const CHACHA_ELF: &[u8] = include_bytes!(
    "../../../../target/elf-compilation/docker/riscv32im-succinct-zkvm-elf/release/chacha-program" // "../../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/chacha-program"
);

fn main() {
    let prover = ProverClient::builder().cpu().build();
    let (_, vk) = prover.setup(CHACHA_ELF);
    println!("{}", vk.bytes32());
}
