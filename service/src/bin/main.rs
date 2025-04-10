//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use clap::Parser;
use hex::FromHex;
use sha2::{Digest, Sha256};
use sp1_sdk::{ProverClient, SP1Stdin, include_elf};

use chacha_lib::chacha;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const CHACHA_ELF: &[u8] = include_elf!("chacha-program");

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

    let key = <[u8; 32]>::from_hex(
        std::env::var("ENCRYPTION_KEY").expect("Missing ENCRYPTION_KEY env var"),
    )
    .expect("Key must be 32 bytes");
    stdin.write_slice(&key);

    let nonce: [u8; 12] = chacha_lib::random_nonce();
    stdin.write_slice(&nonce);

    // TODO: replace example bytes with service interface
    let input_plaintext: &[u8] = chacha_lib::INPUT_BYTES;
    stdin.write_slice(input_plaintext);

    let client = ProverClient::from_env();
    if args.execute {
        // Execute the program
        let (output, report) = client.execute(CHACHA_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Read the output.
        // - sha2 hash = 32 bytes
        // - ciphertext = encrypted bytes
        let output = output.to_vec();
        let (output_hash_plaintext, output_ciphertext) = output.split_at(32);

        // Check against the input
        let input_plaintext_digest = Sha256::digest(input_plaintext);
        println!(
            "Input -> plaintext hash: 0x{}",
            chacha_lib::bytes_to_hex(&input_plaintext_digest)
        );
        println!(
            "zkVM -> plaintext hash: 0x{}",
            chacha_lib::bytes_to_hex(output_hash_plaintext)
        );

        let ciphertext_digest = Sha256::digest(output_ciphertext);
        println!(
            "zkVM -> ciphertext hash: 0x{}",
            chacha_lib::bytes_to_hex(&ciphertext_digest)
        );

        // NOTE: stream cipher is decrypted by running the chacha encryption again.
        // (plaintext XOR keystream XOR keystream = plaintext; QED)
        let mut output_plaintext = output_ciphertext.to_owned();
        chacha(&key, &nonce, &mut output_plaintext);

        assert_eq!(output_plaintext, input_plaintext);
        println!("Decryption of zkVM ciphertext matches input!");

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
