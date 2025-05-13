# BLOB Tool

This utility is to help in **development & testing** basic operations with a Celestia Network.
Given specifics about a datum, a NMT proof is saved to `./proof_input.json`.
This JSON can be used in [another utility](../runner-keccak-inclusion) to **test creation of a ZK proof** that ultimately the [`eq-service` provides for it's users](../README.md).

## Requirements

You must run a local Celestia Node, hardcoded to use `ws://localhost:26658` to connect.

## Usage

```sh
# Choose a network & transaction from an explorer like Celenium.io
# Mainnet: https://celenium.io/
# Tesetnet: https://mocha-4.celenium.io/
cargo r -- --height <integer> --namespace "hex string" --commitment "base64 string"

# Known working example from the Mocha Testnet:
# https://mocha.celenium.io/tx/30a274a332e812df43cef70f395c413df191857ed581b68c44f05a3c5c322312
# Namespace base64 = "Ucwac9Zflfa95g=="
cargo r -- --height 4499999 --namespace "51cc1a73d65f95f6bde6" --commitment "S2iIifIPdAjQ33KPeyfAga26FSF3IL11WsCGtJKSOTA="

# getting blob...
# getting nmt multiproofs...
# Wrote proof input to proof_input.json
```
