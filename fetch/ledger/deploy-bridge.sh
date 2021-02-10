#!/usr/bin/env bash
set -e

fetchcli config keyring-backend test
echo $FETCHMNEMONIC | fetchcli keys add validator --recover
fetchcli config chain-id test
fetchcli config node tcp://fetchledger:26657
fetchcli config trust-node true