# First stage: Build the Rust application
FROM rust:latest AS build-env

RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive \
  apt-get install --no-install-recommends --assume-yes \
  protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

# Install SP1 toolchain (before copying source, for Docker cache)
RUN curl -L https://sp1.succinct.xyz | bash
RUN /root/.sp1/bin/sp1up

####################################################################################################
## Builder
####################################################################################################
FROM build-env AS builder

WORKDIR /app

# Copy the Cargo files first to leverage Docker cache
COPY Cargo.lock ./
COPY Cargo.toml ./
COPY service/Cargo.toml ./service/
COPY common/Cargo.toml ./common/
COPY sdk/Cargo.toml ./sdk/
COPY zksync-interface/Cargo.toml ./zksync-interface/
COPY runner-keccak-inclusion/Cargo.toml ./runner-keccak-inclusion/
COPY blob-tool/Cargo.toml ./blob-tool/
COPY eqs-client/Cargo.toml ./eqs-client/
COPY program-keccak-inclusion/Cargo.toml ./program-keccak-inclusion/

# Dummy main OR lib for cargo fetch to resolve
RUN mkdir -p service/src && \
    echo 'fn main() {}' > service/src/main.rs && \
    mkdir -p common/src && \
    echo 'fn main() {}' > common/src/lib.rs && \
    mkdir -p sdk/src && \
    echo 'fn main() {}' > sdk/src/lib.rs && \
    mkdir -p zksync-interface/src && \
    echo 'fn main() {}' > zksync-interface/src/main.rs && \
    mkdir -p runner-keccak-inclusion/src && \
    echo 'fn main() {}' > runner-keccak-inclusion/src/main.rs && \
    mkdir -p blob-tool/src && \
    echo 'fn main() {}' > blob-tool/src/main.rs && \
    mkdir -p eqs-client/src && \
    echo 'fn main() {}' > eqs-client/src/main.rs && \
    mkdir -p program-keccak-inclusion/src && \
    echo 'fn main() {}' > program-keccak-inclusion/src/main.rs

# Fetch dependencies
RUN cargo fetch

# Copy the actual source code
COPY . .

# Build ZK Program ELF
RUN /root/.sp1/bin/cargo-prove -p eq-program-keccak-inclusion

# Compile the Rust project in release mode
RUN cargo build --release

####################################################################################################
## Final
####################################################################################################
FROM debian:bullseye-slim

COPY --from=builder /app/target/release/eq_service ./ 

# Expose the gRPC port
EXPOSE 50051

# Set the default command to run the gRPC service
CMD ["/eq_service"]
