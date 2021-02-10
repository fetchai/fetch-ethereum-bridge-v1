#!/usr/bin/env bash
set -e

fetchcli config keyring-backend test
echo $FETCHMNEMONIC | fetchcli keys add validator --recover
sleep 1
fetchcli config chain-id test
sleep 1
fetchcli config node tcp://fetchledger:26657
sleep 1
fetchcli config trust-node true
sleep 1
fetchcli config output json
sleep 1
fetchcli config indent true
sleep 1
fetchcli config broadcast-mode block
sleep 1

./scripts/compile.sh cosmwasm_contract/

sleep 1
RES=$(fetchcli tx wasm store bridge.wasm --from validator --gas="auto" -y)
CODE_ID=$(echo $RES | jq -r ".logs[0].events[0].attributes[-1].value")

RES=$(fetchcli tx wasm instantiate $CODE_ID '{}' --from validator --label my-bridge-contract --amount 5000000000000000000000atestfet -y)
CONTRACT_ADDRESS=$(echo $RES | jq -r ".logs[0].events[0].attributes[-1].value")

echo "Contract address: $CONTRACT_ADDRESS"