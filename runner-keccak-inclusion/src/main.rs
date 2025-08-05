#![doc = include_str!("../README.md")]

use clap::{command, Parser};
use eq_common::ZKStackEqProofInput;
use sp1_sdk::{ProverClient, SP1Stdin};
use std::fs;

const KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, short)]
    input_path: String,
}

fn main() {
    sp1_sdk::utils::setup_logger();
    let args = Args::parse();

    let input_json = fs::read_to_string(&args.input_path).unwrap_or_else(|_| {
        panic!(
            "Failed reading proof JSON input from path: {}",
            args.input_path
        )
    });

    println!("Reading input JSON...");
    let input: ZKStackEqProofInput =
        serde_json::from_str(&input_json).expect("Failed deserializing proof input");

    println!("Building mock ProverClient...");
    let client = ProverClient::builder().mock().build();
    let mut stdin = SP1Stdin::new();
    stdin.write(&input);
    // client
    //     .execute(KECCAK_INCLUSION_ELF, &stdin)
    //     .run()
    //     .expect("Failed executing program");

    /*let (pk, _vk) = client.setup(&KECCAK_INCLUSION_ELF);
    let proof = client
        .prove(&pk, &stdin)
        .groth16()
        .run()
        .expect("Failed proving");
    fs::write(
        "sample_groth16_proof.json",
        serde_json::to_string(&proof).expect("Failed to serialize proof"),
    )
    .expect("Failed to write proof to file");*/
    println!("Executing program... INFO logs with cycle counts should appear shortly:");
    let _r = client
        .execute(&KECCAK_INCLUSION_ELF, &stdin)
        .run()
        .expect("Failed executing program");

    println!("✅ Proof seems OK! Execution completed without issue.");
}
