# BLOB Tool

This utility is to help in **development & testing** basic operations with a Celestia Network.
Given specifics about a datum, a NMT proof is saved to `./proof_input.json`.
This JSON can be used in [another utility](../runner-keccak-inclusion) to **test creation of a ZK proof** that ultimately the [`eq-service` provides for it's users](../README.md).

## Usage

```sh
# Choose a network & transaction from an explorer like Celenium.io
# Mainnet: https://celenium.io/
# Tesetnet: https://mocha-4.celenium.io/
cargo r -- --height <integer> --namespace "hex string" --commitment "base64 string"

# Known working example from the Mocha Testnet:
# https://mocha-4.celenium.io/blob?commitment=Ok8KERqJ3my8Z/D4DX6DfUDaeoMR0iUlxOrX1YsrAg4=&hash=AAAAAAAAAAAAAAAAAAAAAAAAAMod4SoDpykQeR8=&height=4337783
# The first commitment in https://mocha-4.celenium.io/tx/d88d60f4fb783cc24ab07688ed6f05a50e32d58823df00e2d99ffc5ad5f74b47
cargo r -- --height 4337783 --namespace "ca1de12a03a72910791f" --commitment "Ok8KERqJ3my8Z/D4DX6DfUDaeoMR0iUlxOrX1YsrAg4="

# getting blob...
# getting nmt multiproofs...
# Wrote proof input to proof_input.json
```
