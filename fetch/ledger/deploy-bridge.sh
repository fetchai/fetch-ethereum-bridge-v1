#!/usr/bin/env bash
set -e

fetchd config keyring-backend test
if [ -z "$(fetchd keys show validator 2>/dev/null)" ]; then
    echo $FETCHMNEMONIC | fetchd keys add validator --recover
fi
fetchd config chain-id test
fetchd config node tcp://fetchledger:26657
fetchd config output json
fetchd config broadcast-mode block

./scripts/compile.sh cosmwasm_contract/

RES=$(fetchd tx wasm store bridge.wasm --from validator --gas="auto" -y)
CODE_ID=$(fetchd query tx $(echo $RES | jq -r '.txhash') --output json | jq -r ".logs[0].events[0].attributes[-1].value")

RES=$(fetchd tx wasm instantiate $CODE_ID '{"cap":"10000000000000000000000", "deposit":"500000000000000000000", "upper_swap_limit":"1000000000000000000000", "lower_swap_limit":"1000000000000000000", "swap_fee":"10000000000000000", "reverse_aggregated_allowance":"1000000000000000000000", "reverse_aggregated_allowance_approver_cap":"1000000000000000000000"}' --from validator --label my-bridge-contract --amount 5000000000000000000000atestfet -y)
CONTRACT_ADDRESS=$(fetchd query tx $(echo $RES | jq -r '.txhash') --output json | jq -r ".logs[0].events[0].attributes[-1].value")

echo "Contract address: $CONTRACT_ADDRESS"