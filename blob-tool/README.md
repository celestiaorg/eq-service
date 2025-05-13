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

# Known working example from the Mocha Testnet (~1.5MB):
# https://mocha.celenium.io/tx/30a274a332e812df43cef70f395c413df191857ed581b68c44f05a3c5c322312
cargo r -- --height 5967025 --namespace "c27fc4694d31d1" --commitment "Y+8haW3Hi89DdtT4AAgr1iZ4ELFbosTqF+UCnhc4adM="

# Known working example from the Mocha Testnet (~0.125MB):
# https://mocha-4.celenium.io/tx/a54e3b86dc095180ecda631e67e25ef9d8450dc1de5bd2af4dc2cfa50b4b3ac4
cargo r -- --height 6062832 --namespace "5d251311f25b13a549e0" --commitment "JPqS2PmVBNdyo8IadhIgIzvgbV99LQido2LAEaCp+vY="

```
