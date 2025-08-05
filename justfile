default:
    @just --list

alias e := run-examples
alias r := run-debug
alias gb := get-blob-input
alias rk := run-keccack-mock
alias rr := run-release
alias db := docker-build
alias dr := docker-run
alias pb := podman-build
alias pr := podman-run
alias b := build-debug
alias br := build-release
alias f := fmt
alias c := clean

zkvm-elf-path := "./target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
env-settings := "./.env"
sp1up-path := shell("which sp1up")
cargo-prove-path := shell("which cargo-prove")
websocat-path := shell("which cargo-prove")

# Install SP1 tooling & more
initial-config-installs:
    #!/usr/bin/env bash
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
        cd program-keccak-inclusion
        cargo prove build
    fi

_pre-run:
    #!/usr/bin/env bash
    if ! {{ path_exists(env-settings) }}; then
        echo -e "⛔ Missing required \`{{ env-settings }}\` file.\nCreate one with:\n\n\tcp example.env .env\n\nAnd then edit to adjust settings"
        exit 1
    fi

# Run examples
run-examples *FLAGS: _pre-build _pre-run
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    cargo t --workspace
    cargo run -p eq-sdk --example client -- {{ FLAGS }}

# Run in release mode, with optimizations AND debug logs
run-release *FLAGS: _pre-build _pre-run
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    RUST_LOG=eq_service=debug cargo r -r -- {{ FLAGS }}

# Run in debug mode, with extra pre-checks, no optimizations
run-debug *FLAGS: _pre-build _pre-run
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    # Check node up with https://github.com/vi/websocat?tab=readme-ov-file#from-source
    if ! {{ path_exists(websocat-path) }}; then
        echo -e "⛔ Missing websocat tool.\nRun `cargo install websocat` to install"
        exit 1
    fi
    if ! echo "ping" | websocat $CELESTIA_NODE_HTTP -1 -E &> /dev/null ; then
        echo -e "⛔ Node not available @ $CELESTIA_NODE_HTTP - start a mocha one locally with 'just mocha' "
        exit 1
    fi

    # export CELESTIA_NODE_AUTH_TOKEN=$(celestia light auth admin --p2p.network mocha)
    RUST_LOG=eq_service=debug cargo r -- {{ FLAGS }}

# Build docker image & tag
docker-build:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    docker build --build-arg BUILDKIT_INLINE_CACHE=1 --tag "$DOCKER_CONTAINER_NAME" --progress=plain .

# Run a pre-built docker image
docker-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source {{ env-settings }}
    mkdir -p $EQ_DB_PATH
    docker run --rm -it -v $EQ_DB_PATH:$EQ_DB_PATH --env-file {{ env-settings }} --env RUST_LOG=eq_service=debug --network=host -p $EQ_PORT:$EQ_PORT eq-service

# Build docker image & tag `eq-service`
podman-build:
    podman build -t eq-service .

# Run a pre-built podman image
podman-run:
    #!/usr/bin/env bash
    set -a  # Auto export vars
    source .env
    mkdir -p $EQ_DB_PATH
    podman run --rm -it -v $EQ_DB_PATH:$EQ_DB_PATH --env-file {{ env-settings }} --env RUST_LOG=eq_service=debug --network=host -p $EQ_PORT:$EQ_PORT eq-service

# Build in debug mode, no optimizations
build-debug: _pre-build
    cargo b

# Build in release mode, includes optimizations
build-release: _pre-build
    cargo b -r

# Scrub build artifacts
clean:
    #!/usr/bin/env bash
    cargo clean

# Format source code
fmt:
    @cargo fmt
    @just --quiet --unstable --fmt > /dev/null

# Build & open Rustdocs for the workspace
doc:
    RUSTDOCFLAGS="--enable-index-page -Zunstable-options" cargo +nightly doc --no-deps --workspace
    xdg-open {{ justfile_directory() }}/target/doc/index.html

# Launch a local Celestia testnet: Mocha
mocha:
    # Assumes you already did init for this & configured
    # If not, see https://docs.celestia.org/tutorials/node-tutorial#setting-up-dependencies
    celestia light start --core.ip rpc-mocha.pops.one --p2p.network mocha

# Setup and print to stdout, needs to be set in env to be picked up by eq-service
mocha-local-auth:
    celestia light auth admin --p2p.network mocha

# Run the blob-tool program to generate proof_input.json
get-blob-input:
    #!/usr/bin/env bash
    set -a       # Automatically export all variables sourced next
    source ./.env  # Source the .env file (variables now exported)
    set +a       # Stop automatically exporting variables
    # cargo r -p blob-tool -- --height 7459012 --namespace "736f762d6d696e692d64" --commitment "UO0o/fdzhobbekE/HyYAH6FK5jGkdpSMHyxeclQHvWc="
    cargo r -p blob-tool -- --height 7501765 --namespace "a0fc6c7568eb2756a483" --commitment "Cytx86AUkY/HPVeVkiKbkIJpsdWXvkCvluqUtidVDE0="

# Run the runner-keccak-input program with get-blob-input's output to see cycle counts
run-keccack-mock: _pre-build
    RUST_LOG=info cargo r -p runner-keccak-inclusion -- -i large_proof_input.json
