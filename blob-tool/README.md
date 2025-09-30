# BLOB Tool

This utility is to help in **development & testing** basic operations with a Celestia Network.
Given specifics about a datum, a NMT proof is saved to `./proof_input.json`.
This JSON can be used in [another utility](../runner-keccak-inclusion) to **test creation of a ZK proof** that ultimately the [`eq-service` provides for it's users](../README.md).

## Requirements

You must run a local Celestia Node, hardcoded to use `ws://localhost:26658` to connect.

## Usage

```sh
# set CELESTIA_NODE_AUTH_TOKEN env variable
set -a       # Automatically export all variables sourced next
source ../.env  # Source the .env file (variables now exported)
set +a       # Stop automatically exporting variables

# Choose a network & transaction from an explorer like Celenium.io
# Mainnet: https://celenium.io/
# Tesetnet: https://mocha-4.celenium.io/
cargo r -- --height <integer> --namespace "hex string" --commitment "base64 string"

# Known working example from the Mocha Testnet (~1.85kB):
# https://mocha.celenium.io/tx/bc6110376a9db2dcf70d29270f40bfd13eacacd26ad52c286f8e5414220d1902
cargo r -- --height 8136361 --namespace "2777d4d961c75a526dd8" --commitment "daNbPqcPOZRD/FgPjVUCQIlKVwxYi15VpksJPQNp7ss="

# Known working example from the Mocha Testnet (~5.47kB):
# https://mocha.celenium.io/tx/2c33302de8b183d40a16c53411dc6f441356558c69c386b02a66c1f0b1f59b24
cargo r -- --height 8135203 --namespace "8f8736b6ff9dc08065a6" --commitment "pALxAoqv6WCyJ1PvFk/ZGPvoGMIHeZDjhP/0LeKLVKE="
```
