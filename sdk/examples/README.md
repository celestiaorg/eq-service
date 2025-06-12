# Eq Service SDK Examples

This folder contains example programs demonstrating how to use `eq-sdk`.

## Running an Example

These typically require a running [eq-service](../../service) instance , see the [workspace README](../../README.md) for setting the service up & configuring `env` variables as needed.

```sh
# Define and source required vars in .env
set -a
source ../.env

# https://mocha.celenium.io/tx/c3c301fe579feb908fe02e2e8549c38f23707d30a3d4aa73e26402d854ff9104
# "height": 4409088, "namespace": "XSUTEfJbE6VJ4A==", "commitment":"DYoAZpU7FrviV7Ui/AjQv0BpxCwexPWaOW/hQVpEl/s="
cargo run --example client -- --socket $EQ_SOCKET --height 4409088 --namespace XSUTEfJbE6VJ4A== --commitment DYoAZpU7FrviV7Ui/AjQv0BpxCwexPWaOW/hQVpEl/s=
```
