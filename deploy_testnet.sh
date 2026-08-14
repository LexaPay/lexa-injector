#!/bin/bash
set -e
echo "Funding testnet signer identity..."
curl -X POST "https://friendbot.stellar.org?addr=$1"
echo "Deploying to Testnet..."
stellar contract deploy --wasm target/wasm32-unknown-unknown/release/laxaflow_contract.wasm --network testnet --source $1
