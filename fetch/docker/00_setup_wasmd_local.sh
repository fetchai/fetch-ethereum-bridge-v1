#!/bin/bash
set -e 

# Initialise test chain
# Clean state 
rm -rf ~/.wasm*

# SETUP LOCAL CHAIN
# Initialize the genesis.json file that will help you to bootstrap the network
wasmd init --chain-id=testing testing
#sed -i "s/\"stake\"/\"$TOKEN\"/" "$HOME"/.wasmd/config/genesis.json

# Create a key to hold your validator account
(echo "$PASSWORD"; echo "$PASSWORD") | wasmcli keys add validator

# Add validator to genesis block and give him some stake
echo "$PASSWORD" | wasmd add-genesis-account $(wasmcli keys show validator -a) 1000000000$TOKEN,1000000000stake

# Generate the transaction that creates your validator
(echo "$PASSWORD"; echo "$PASSWORD"; echo "$PASSWORD") | wasmd gentx --name validator

# Add the generated bonding transaction to the genesis file
wasmd collect-gentxs

# Configure wasmcli
wasmcli config chain-id testing
wasmcli config trust-node true
wasmcli config node http://localhost:26657
wasmcli config output json
wasmcli config indent true
wasmcli config broadcast-mode block