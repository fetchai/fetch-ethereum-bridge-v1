#!/usr/bin/env bash
set -e

fetchd init test-node --chain-id test
sed -i 's/stake/atestfet/' ~/.fetchd/config/genesis.json
# Enable rest
sed -i 's/enable = false/enable = true/' ~/.fetchd/config/app.toml

fetchcli config keyring-backend test
fetchcli keys add validator
fetchd add-genesis-account $(fetchcli keys show validator -a) 1152997575000000000000000000atestfet
fetchd gentx --amount 100000000000000000000atestfet --name validator --keyring-backend test
fetchd collect-gentxs

fetchd start --rpc.laddr tcp://0.0.0.0:26657
