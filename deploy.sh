#!/bin/bash
set -e

echo "Deploying LaxaFlow contract to Stellar Testnet..."
echo "Ensure you have the Stellar CLI installed and your identity configured."

# Compile WASM
cargo build --target wasm32-unknown-unknown --release

# Dry-run command visualization
echo "Suggested deploy command:"
echo "stellar contract deploy \\"
echo "  --wasm target/wasm32-unknown-unknown/release/laxaflow_contract.wasm \\"
echo "  --network testnet \\"
echo "  --source <YOUR_SIGNER_IDENTITY>"
