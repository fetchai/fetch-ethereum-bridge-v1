## Contract setup
Use node version manager (nvm) to install the latest stable version of npm:

```
curl -o- https://raw.githubusercontent.com/creationix/nvm/v0.33.0/install.sh | bash
nvm install node
nvm use node
nvm install --lts
nvm use --lts
```

### Install all necessary dependencies:
```shell
npm install
```

### Start Ganache node
Following command will start **LOCALLY** installed version of ganache configured using correct Ganache chainId
```shell
$(npm bin)/ganache-cli --chainId 1337
```

### Running Tests
Run following command to execute all tests.
```shell
$(npm bin)/truffle test
```
>This will also provide estimates of the average gas costs
of the different methods through the `eth-gas-reporter`.

> Tests use mock ERC20 contract for FET tokens. It has the same capabilities & configured exactly as our official 
> FET token on mainnet.


### Deploying contracts
By default the deployment is done on the `development` network = locally running Ganage node:
```shell
$(npm bin)/truffle deploy
```
, in which case it is not necessary to provide the `--network development` command-line switch.

When deploying contract(s) on desired network, it is necessary to provide the `--network NETORK_NAME` command-line 
switch, as in the example bellow, where `NETWORK_NAME` shall be substituted by one of **keys** listed in the 
`networks` dictionary in the `truffle-config.js` configuration file, for example, deployment on `mainnet` would
look like this:
```shell
$(npm bin)/truffle --network mainnet deploy
```
