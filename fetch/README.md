# Cosmwasm contract deployment and events processing

## Environment setup

Build the docker dev image

```bash
docker build -t cosmwasm-bridge -f docker/Dockerfile .
```

## Deploy a local cosmos blockchain 

For quick testing using a local blockchain can be handy. 
We can deploy a single-node blockchain using the built docker image

```bash
 docker run --rm --init --net=host -it cosmwasm-bridge
```

## Prepare the contract 

Cosmwasm contracts need to be compiled before deployment.
We will use the built docker image environment to compile the contract

```bash
docker run --rm --net=host -v $(pwd):/source/ --workdir /source/  --entrypoint /bin/bash -it cosmwasm-bridge
```

To compile the contract, run the following within the docker container

```bash
./scripts/compile.sh cosmwasm_contract/
```

If successfully, this should produce `bridge.wasm` file in the current working directory (i.e. `fetch`). 
This is the file to use for deployment.


## Upload the smart contract

Assuming we are using the local cosmos blockchain, run the following to upload the contract

```bash
RES=$((echo "$PASSWORD"; echo "$PASSWORD") | fetchcli tx wasm store bridge.wasm --from validator --gas="auto" -y)
CODE_ID=$(echo $RES | jq -r ".logs[0].events[0].attributes[-1].value")
echo $CODE_ID

```

upon success `CODE_ID` env variable should contain an integer (`1` if first storage) that we will use to deploy an instance of the contract.

## Deploy/Instantiate the smart contract

Once the contract is successfully uploaded, it can be instantiated as follow
```bash
RES=$((echo "$PASSWORD"; echo "$PASSWORD") | fetchcli tx wasm instantiate $CODE_ID '{"cap":"10000", "deposit":"500", "upper_swap_limit":"1000", "lower_swap_limit":"2", "swap_fee":"1"}' --from validator --label my-bridge-contract --amount 10000ucosm -y)
CONTRACT_ADDRESS=$(echo $RES | jq -r ".logs[0].events[0].attributes[-1].value")
echo $CONTRACT_ADDRESS > contract_address
```

If successful, this will return the contract address in `CONTRACT_ADDRESS` env variable that we will need to provide as a receiver reference for any subsequent execution of the contract operations/actions.


## Watch events on the deployed contract

The current python script only handles actions execution events. To start watching such events for a given action, from the same working directory and in a new terminal run

```bash
docker run --rm --net=host -v $(pwd):/source/ --workdir /source/  --entrypoint /bin/bash -it cosmwasm-bridge
# inside container
CONTRACT_ADDRESS=$(cat contract_address)
python3 cosmwasm_watch_contract_events.py $CONTRACT_ADDRESS swap
```

The script will be watching for events related to successful execution of `swap` action on the deployed contract.

Now, let's produce such events by requesting execution of `Swap` operation. 
Go back to the previous container shell and run the following:

```bash
(echo "$PASSWORD"; echo "$PASSWORD") | fetchcli tx wasm execute $CONTRACT_ADDRESS '{"swap": {"destination":"some-ether-address"}}' --amount 200ucosm --from validator -y
```

## Contract operations

+ `swap` 
  ```bash
  (echo "$PASSWORD"; echo "$PASSWORD") | fetchcli tx wasm execute $CONTRACT_ADDRESS '{"swap": {"destination":"some-ether-address"}}' --amount 200ucosm --from validator -y
  ```
+ `reverse_swap`
  ```bash
  (echo "$PASSWORD"; echo "$PASSWORD") | fetchcli tx wasm execute $CONTRACT_ADDRESS '{"reverse_swap": {"rid":10, "to":"fetch1f8tcyaw6tkq5f6k527leclqp644lcmzv0rgdm9", "sender":"some-ethereum-address", "origin_tx_hash":"11111111", "amount":"10", "relay_eon": 0}}' --from validator -y
  ```




## Resources
### local wasmd deployment

- official repo https://github.com/CosmWasm/wasmd
- dev node setup https://github.com/CosmWasm/wasmd/tree/master/docker
- colearn-contract script 

### contract programming

- rust cosmwasm lib reference
- contract template
- contract examples

### querying events

- hight-based queries
- websocket subscription