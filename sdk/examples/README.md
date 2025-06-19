# Eq Service SDK Examples

This folder contains example programs demonstrating how to use `eq-sdk`.

## Running an Example

These typically require a running [eq-service](../../service) instance , see the [workspace README](../../README.md) for setting the service up & configuring `env` variables as needed.

```sh
# Define and source required vars in .env
set -a
source ../.env

# https://mocha.celenium.io/tx/436f223bfa8c4adf1e1b79dde43a84918f3a50809583c57c33c1c079568b47cb
# "height": 6608695, "namespace": "C8EGyFVBxOL3bA==", "commitment":"nPDyRXefks+koMJhy7LzN9269+Oz4PjcsAPk64ke85E="
cargo run --example client -- --socket $EQ_SOCKET --height 6608695 --namespace C8EGyFVBxOL3bA== --commitment nPDyRXefks+koMJhy7LzN9269+Oz4PjcsAPk64ke85E=
```
