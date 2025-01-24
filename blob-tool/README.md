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
# https://mocha-4.celenium.io/tx/779fde7afe95df0249410a6f19a37f9b6b645d7005add6e5a64bfa86e58bffce
cargo r -- --height 4336630 --namespace "08e5f679bf7116cb" --commitment "IeQ21D1pTfP5ArfION2SGtxPDYUpg2trwYZ4OxsTK5k="

# getting blob...
# getting nmt multiproofs...
# Wrote proof input to proof_input.json
```
