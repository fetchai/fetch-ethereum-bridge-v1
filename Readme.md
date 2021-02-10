# Fetch-Ethereum-Bridge-v1
The docker-compose in the root of this repo is used to deploy the etehreum migraton contract in a local ganache container running in docker.

## Spin up infrastructure
By running `docker-compose up` a local ganache and a contract deployment container are being built and spinned up

If you always want to build images run `docker-compose up --build`.

After that, in a new terminal please run the following command depending on your desired action:
```/bin/bash
# Import brownie networks
docker-compose exec ethereum brownie networks import networks-config.yaml True

#Deploy ERC20 mock
docker-compose exec ethereum brownie run deploy_erc20mock.py --network docker

# Deploy Ethereum Bridge contract
docker-compose exec ethereum brownie run deploy_bridge.py --network docker

# Deploy Cosmos Bridge contract. Please run this after block 44
# If you get ERROR: aborted execture the following command
# docker-compose exec fetch fetchcli keys delete validator
docker-compose exec fetch ./ledger/deploy-bridge.sh
```

The infrastructure can be deleted by running `docker-compose down`.