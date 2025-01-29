# Data Availability Equivalence Service

A service acting as a "cryptographic adapter"[^1] providing proofs that data exists on [Celestia](https://celestia.org/) that are efficiently verifiable on EVM networks.
A NMT proof is transformed into keccak inclusion for data roots using a zkVM.

[^1] See "The Universal Protocol" in [0xPARC's blog](https://0xparc.org/blog/programmable-cryptography-1) for some more concrete ideas.

## Prerequisites

- Rust & Cargo - [install instructions](https://www.rust-lang.org/tools/install)
- Succinct's SP1 zkVM Toolchain - [install instructions](https://docs.succinct.xyz/docs/getting-started/install)
- Protocol Buffers (Protobuf) compiler - [official examples](https://github.com/hyperium/tonic/tree/master/examples#examples) contain install instructions
- Celestia Light Node - [installed](https://docs.celestia.org/how-to-guides/celestia-node) & [running](https://docs.celestia.org/tutorials/node-tutorial#auth-token) accessible on `localhost`, or elsewhere. Alternatively, use [an RPC provider](https://github.com/celestiaorg/awesome-celestia/?tab=readme-ov-file#node-operator-contributions) you trust.
- Just - a modern alternative to `make` [installed](https://just.systems/man/en/packages.html)

## Installation

1. Clone the repository:

   ```sh
   git clone https://github.com/your-repo-name/eq-service.git
   cd eq-service
   ```

2. Build and run the service:
   ```sh
   just run-release
   ```

## Configuration

Service settings are configured via a required `.env` file. See [`example.env`](./example.env) for configurable items.

```sh
cp example.env .env
# edit .env
```

This config is overridable by existing environment variables:

```sh
# Set a new database location, as a one-off
DB_PATH="/tmp/just-playing" just rr

# Set a new database for the remainder of this shell's life
export DB_PATH="/some/other/place"
just rr
```

## Architecture

```mermaid
flowchart TB
    user@{ shape: circle, label: "End User" } ==>|POST Request Eq. Proof| jobs
    zkep@{ shape: doc, label: "zkVM Equivalence Program" } --> zkc

    subgraph eq ["`**Equivalence Service**`"]
        nmtp@{ shape: lean-r, label: "NMT Proofs" }
        zkp@{ shape: lean-r, label: "ZK Proofs" }
        lc[Celestia Light Client] --> nmtp --> jobs
        nmtp --> zkc
        zkc[ZK Proof Generation Client] --> zkp --> jobs
        jobs[Jobs Que & Results Cache] <--> sled
        sled[(Sled DB)]
    end

    lc --->|GET NMT Inclusion| cel{Celestia}

    zkc --->|POST Proof Generation| pn{ZK Prover Network}
    style zkep fill:#66f
    style jobs fill:#888
```

### Celestia Data Availability (DA)

The service interacts with a Celestia Node using gRPC to:

- Fetch blob data.
- Get headers.
- Retrieve Merkle tree proofs for blobs.

### Zero-Knowledge Proofs (ZKPs)

The service uses [Succinct's pover network](https://docs.succinct.xyz/docs/generating-proofs/prover-network) as a provider to generate keccak proofs of data existing on Celestia.

- See the [zkVM program](./program-keccak-inclusion/src/main.rs) for details on what is proven.

## Usage

To interact with the service, clients can use any gRPC client that supports protobuf messages. Here is an example using the [`grpcurl`](https://github.com/fullstorydev/grpcurl) CLI tool:

### Connect to a Celestia Node

See the [How-to-guides on nodes](https://docs.celestia.org/how-to-guides/light-node) to run one yourself, or choose a provider.
Set the corret info in your `.env` file of choice.

### Launch the Eq Service

```sh
# Bring required vars into scope, or replace $<variable> below
source .env

# Fetching the Keccak inclusion proof for a specific Celestia commitment, namespace, and height
grpcurl -import-path $EQ_PROTO_DIR -proto eqservice.proto \
  -d '{height": <block height (integer)>", "namespace": "<your_namespace_hex>", commitment": "<your_commitment_hex>"}'
  -plaintext $EQ_SOCKET eqs.Inclusion.GetKeccakInclusion

# Working examples using Celestia's mainnet network
grpcurl -import-path $EQ_PROTO_DIR -proto eqservice.proto \
  -d '{"height": 4214864, "namespace": "3q2+796tvu8=", "commitment":"YcARQRj9KE/7sSXd4090FAONKkPz9ajYKIZq8liv3A0="}' \
  -plaintext $EQ_SOCKET eqs.Inclusion.GetKeccakInclusion

grpcurl -import-path $EQ_PROTO_DIR -proto eqservice.proto \
  -d '{"height": 4409088, "namespace": "XSUTEfJbE6VJ4A==", "commitment":"DYoAZpU7FrviV7Ui/AjQv0BpxCwexPWaOW/hQVpEl/s="}' \
  -plaintext $EQ_SOCKET eqs.Inclusion.GetKeccakInclusion
```

## Development

### `grpc` and ELF Generation

The requies `prost` and `tonic` to generate gRPC bindings, as well as a compiled verifiable program to execute in a RISC-V zkVM. To (re)generate the required files, run:

```sh
just build-fresh
```

### Testing

The service includes basic tests to ensure that the core functionality works as expected. You can run these tests using Cargo:

```sh
just test
```

## License

TODO
