#![doc = include_str!("../README.md")]

use eq_common::KeccakInclusionToDataRootProofInput;
use sp1_sdk::{ProverClient, SP1Stdin};
use std::{env, fs, process};

const KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
);

fn main() {
    sp1_sdk::utils::setup_logger();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <relative path to proof_input.json>", args[0]);
        process::exit(1);
    }
    let input_path = &args[1];

    let input_json = fs::read_to_string(input_path)
        .unwrap_or_else(|_| panic!("Failed reading proof input from {}", input_path));
    let input: KeccakInclusionToDataRootProofInput =
        serde_json::from_str(&input_json).expect("Failed deserializing proof input");

    // Set SP1_PROVER in .env or otherwise
    let client = ProverClient::from_env();
    let mut stdin = SP1Stdin::new();
    stdin.write(&input);

    // Execute
    let _r = client
        .execute(&KECCAK_INCLUSION_ELF, &stdin)
        .run()
        .expect("Failed executing program");

    println!("✅ Proof seems OK! Execution completed without issue.");
}
