#!/bin/bash
set -e 

# Initialise test chain
# Clean state 
rm -rf ~/.fetch*

# SETUP LOCAL CHAIN
# Initialize the genesis.json file that will help you to bootstrap the network
fetchd init --chain-id=testing testing
#sed -i "s/\"stake\"/\"$TOKEN\"/" "$HOME"/.wasmd/config/genesis.json # FIXME(LR) use only one token

# Create a key to hold your validator account
#fetchcli config keyring-backend test
(echo "$PASSWORD"; echo "$PASSWORD") | fetchcli keys add validator

# Add validator to genesis block and give him some stake
echo "$PASSWORD" | fetchd add-genesis-account validator 100000000000000000000$TOKEN,100000000000000000000stake
# FIXME(LR) use only one token

# Generate the transaction that creates your validator
(echo "$PASSWORD"; echo "$PASSWORD"; echo "$PASSWORD") | fetchd gentx --name validator

# Add the generated bonding transaction to the genesis file
fetchd collect-gentxs

# Configure fetchcli
fetchcli config chain-id testing
fetchcli config trust-node true
fetchcli config node http://localhost:26657
fetchcli config output json
fetchcli config indent true
fetchcli config broadcast-mode block