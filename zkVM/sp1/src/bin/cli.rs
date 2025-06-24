use clap::Parser;
use hex::FromHex;
use sha2::{Digest, Sha256};
use sp1_sdk::{ProverClient, SP1Stdin};

use zkvm_common::{KEY_LEN, NONCE_LEN, chacha, std_only::ZkvmOutput};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
/// For reproducible builds, you need `cargo prove --docker`
#[cfg(feature = "reproducible-elf")]
pub const CHACHA_ELF: &[u8] = include_bytes!(
    "../../../../target/elf-compilation/docker/riscv32im-succinct-zkvm-elf/release/chacha-program"
);

#[cfg(not(feature = "reproducible-elf"))]
pub const CHACHA_ELF: &[u8] = include_bytes!(
    "../../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/chacha-program"
);

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    execute: bool,

    #[clap(long)]
    prove: bool,

    #[clap(long, default_value = "20")]
    n: u32,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    let mut stdin = SP1Stdin::new();
    // Setup the inputs:
    // - key = 32 bytes
    // - nonce = 12 bytes (MUST BE UNIQUE - NO REUSE!)
    // - input_plaintext = bytes to encrypt

    let input_key = <[u8; KEY_LEN]>::from_hex(
        std::env::var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY env var"),
    )
    .unwrap_or_else(|_| {
        panic!(
            "ENCRYPTION_KEY must be {} bytes, hex encoded (ex: `1234...abcd`)",
            KEY_LEN
        )
    });
    stdin.write_slice(&input_key);

    let input_nonce: [u8; NONCE_LEN] = zkvm_common::random_nonce();
    stdin.write_slice(&input_nonce);

    // TODO: replace example bytes with service interface
    const INPUT_BYTES: &[u8] = include_bytes!("../../../static/proof_input_example.bin");
    stdin.write_slice(INPUT_BYTES);

    let client = ProverClient::from_env();
    if args.execute {
        // Execute the program
        let (output_buffer, report) = client.execute(CHACHA_ELF, &stdin).run().unwrap();

        // Read the output.
        // - privkey sha2 hash = 32 bytes
        // - nonce = 12 bytes
        // - plaintext sha2 hash = 32 bytes
        // - ciphertext = encrypted bytes, ~1M bytes
        let output =
            ZkvmOutput::from_bytes(output_buffer.as_slice()).expect("Failed to parse header");

        // Check against the input
        let input_plaintext_digest = Sha256::digest(INPUT_BYTES);
        println!(
            "Input -> plaintext hash: 0x{}",
            zkvm_common::std_only::bytes_to_hex(&input_plaintext_digest)
        );
        dbg!(&output);

        assert_eq!(input_nonce, output.nonce);
        // NOTE: stream cipher is decrypted by running the chacha encryption again.
        // (plaintext XOR keystream XOR keystream = plaintext; QED)
        let mut output_plaintext = output.ciphertext.to_owned();
        chacha(&input_key, &output.nonce, &mut output_plaintext);

        assert_eq!(output_plaintext, INPUT_BYTES);
        println!("Decryption of zkVM ciphertext matches input!");
        let input_key_hash = Sha256::digest(input_key);
        assert_eq!(input_key_hash.as_slice(), output.privkey_hash);
        println!("Key used matched!");

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        // Setup the program for proving.
        let (pk, vk) = client.setup(CHACHA_ELF);

        // Generate the proof
        //
        // NOTE:
        // Using the [groth16 proof type](https://docs.succinct.xyz/docs/sp1/generating-proofs/proof-types#groth16-recommended) to trade increased proving costs & time for minimal EVM gas costs.
        let proof = client
            .prove(&pk, &stdin)
            .groth16()
            .run()
            .expect("failed to generate proof");

        println!("Successfully generated proof!");

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    }
}
