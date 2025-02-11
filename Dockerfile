# First stage: Build environment (based on Rust)
FROM rust:latest AS build-env

RUN apt-get update \
  && DEBIAN_FRONTEND=noninteractive \
  apt-get install --no-install-recommends --assume-yes \
    protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

# Install SP1 toolchain (this is done early to benefit from caching)
RUN curl -L https://sp1.succinct.xyz | bash
RUN /root/.sp1/bin/sp1up

####################################################################################################
## Dependency stage: Cache Cargo dependencies via cargo fetch
####################################################################################################
FROM build-env AS deps

WORKDIR /app

# Copy only the Cargo files that affect dependency resolution.
COPY Cargo.lock Cargo.toml ./
COPY service/Cargo.toml ./service/
COPY common/Cargo.toml ./common/
COPY sdk/Cargo.toml ./sdk/
COPY zksync-interface/Cargo.toml ./zksync-interface/
COPY runner-keccak-inclusion/Cargo.toml ./runner-keccak-inclusion/
COPY blob-tool/Cargo.toml ./blob-tool/
COPY eqs-client/Cargo.toml ./eqs-client/
COPY program-keccak-inclusion/Cargo.toml ./program-keccak-inclusion/

# Create dummy targets for each workspace member so that cargo fetch can succeed.
RUN mkdir -p service/src && echo 'fn main() {}' > service/src/main.rs && \
    mkdir -p common/src && echo 'fn main() {}' > common/src/lib.rs && \
    mkdir -p sdk/src && echo 'fn main() {}' > sdk/src/lib.rs && \
    mkdir -p zksync-interface/src && echo 'fn main() {}' > zksync-interface/src/main.rs && \
    mkdir -p runner-keccak-inclusion/src && echo 'fn main() {}' > runner-keccak-inclusion/src/main.rs && \
    mkdir -p blob-tool/src && echo 'fn main() {}' > blob-tool/src/main.rs && \
    mkdir -p eqs-client/src && echo 'fn main() {}' > eqs-client/src/main.rs && \
    mkdir -p program-keccak-inclusion/src && echo 'fn main() {}' > program-keccak-inclusion/src/main.rs

# Run cargo fetch so that dependency downloads are cached in the image.
RUN cargo fetch

####################################################################################################
## Builder stage: Build the application using cached dependencies
####################################################################################################
FROM build-env AS builder

WORKDIR /app

# Import the cached Cargo registry from the deps stage.
COPY --from=deps /usr/local/cargo /usr/local/cargo

# Now copy the rest of your source code.
COPY . .

# Build ZK Program ELF using SP1 toolchain.
RUN /root/.sp1/bin/cargo-prove prove build -p eq-program-keccak-inclusion

# Finally, compile the project in release mode.
RUN cargo build --release

####################################################################################################
## Final stage: Prepare the runtime image
####################################################################################################
FROM debian:bullseye-slim

COPY --from=builder /app/target/release/eq_service ./

EXPOSE 50051

CMD ["/eq_service"]
