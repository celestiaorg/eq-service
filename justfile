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
sp1up-path := shell("which sp1up")
cargo-prove-path := shell("which cargo-prove")

initial-config-installs:
    #!/usr/bin/env bash
    echo {{ path_exists(sp1up-path) }}
    if ! {{ path_exists(sp1up-path) }}; then
        curl -L https://sp1.succinct.xyz | bash
    fi
    echo "✅ sp1up installed"

    if ! {{ path_exists(cargo-prove-path) }}; then
        {{ sp1up-path }}
    else
        echo -e "✅ cargo-prove installed\n     ⚠️👀NOTE: Check you have the correct version needed for this project!"
    fi

_pre-build:
    #!/usr/bin/env bash
    if ! {{ path_exists(cargo-prove-path) }}; then
        echo -e "⛔ Missing zkVM Compiler.\nRun `just initial-config-installs` to prepare your environment"
        exit 1
    fi
    if ! {{ path_exists(zkvm-elf-path) }}; then
        cargo prove build -p eq-program-keccak-inclusion
    fi

_pre-run:
    #!/usr/bin/env bash
    if ! {{ path_exists(env-settings) }}; then
        echo -e "⛔ Missing required `.env` file.\nCreate one with:\n\n\tcp example.env .env\n\nAnd then edit to adjust settings"
        exit 1
    fi

local-mocha-node:
    #!/usr/bin/env bash
    source .env
    export CELESTIA_NODE_AUTH_TOKEN=$(celestia light auth admin --p2p.network mocha)
    echo -e "JWT for Light Node:\n$CELESTIA_NODE_AUTH_TOKEN"
    # celestia light start --p2p.network mocha --core.ip rpc-mocha.pops.one

run *FLAGS: _pre-run build
    #!/usr/bin/env bash
    source .env
    cargo r -- {{ FLAGS }}

run-release *FLAGS: _pre-run build-release
    #!/usr/bin/env bash
    source .env
    cargo r -r -- {{ FLAGS }}

build: _pre-build
    cargo b

build-release: _pre-build
    cargo b -r

clean:
    #!/usr/bin/env bash
    cargo clean

fmt:
    @cargo fmt
    @just --quiet --unstable --fmt > /dev/null
