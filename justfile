default:
    @just --list

alias r := run
alias rr := run-release
alias b := build
alias br := build-release
alias f := fmt
alias c := clean

zkvm-elf-path := "./target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
env-settings := "./.env"

# Private just helper recipe
_pre-build:
    {{ if shell("which cargo-prove") == "" { `echo "zkVM Compiler missing! See README." && exit 1` } else { "" } }}
    {{ if path_exists(zkvm-elf-path) == "false" { `cargo prove build -p eq-program-keccak-inclusion 1> /dev/null` } else { "" } }}
    {{ if path_exists(env-settings) == "false" { `echo "missing required .env file! see example.env"` } else { "" } }}

run *FLAGS: build
    #!/usr/bin/env bash
    set -euxo pipefail
    source .env
    cargo r -- {{ FLAGS }}

run-release *FLAGS: build-release
    #!/usr/bin/env bash
    set -euxo pipefail
    source .env
    cargo r -r -- {{ FLAGS }}

build: _pre-build
    cargo b

build-release: _pre-build
    cargo b -r

clean:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo clean

fmt:
    @cargo fmt
    @just --quiet --unstable --fmt > /dev/null
