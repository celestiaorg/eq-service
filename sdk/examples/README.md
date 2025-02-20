# Eq Service SDK Examples

This folder contains example programs demonstrating how to use `eq-sdk`.

## Running an Example

These typically require a running [eq-service](../../service) instance , see the [workspace README](../../README.md) for setting the service up & configuring `env` variables as needed.

```sh
# Define and source required vars in .env
set -a
source .env
cargo run -p eq-sdk --example client
```
