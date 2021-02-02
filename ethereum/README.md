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

1. Install correct version of the `ganache-cli` command **LOCALLY** running the following command - point is that
   this locally installed version will **NOT** affect system-wide version of `ganache-cli`(if it is installed)):
   ```shell
   npm install
   ```
   > NOTE: The locally installed version of `ganache-cli` command is located in the directory which can
   > be found by running the `npm bin` command, and by default it is `./node_modules/.bin` directory. Thus, if it is
   > required to use/executed it, do it as shown bellow:
   > ```shell
   > $(npm bin)/ganache-cli --chainId 1337
   > ```

2. Use `pipenv` to install necessary python environment:
   ```shell
   pipenv install
   ```

### Using the `brownie` to interact with contracts (deploy, run tests, etc. ...):

1. start shell with correctly configured python environment:
   ```shell
   pipenv shell
   ```

2. import preconfigured networks configuration:
   ```shell
   brownie networks import networks-config.yaml True
   ```

3. then, in the started python env. shell, use `brownie` command to execute desired actions as many times as necessary,
   see few examples provided bellow:

   3.1. example: compile contracts:
      ```shell
      brownie compile
      ```
   3.2. example: execute all tests in using default network configuration (ganache):
      ```shell
      brownie test -s
      ```
      > Tests use mock ERC20 contract for FET tokens. It has the same capabilities & configured exactly as our official
      > FET token on mainnet.
   
   3.3. example: list all networks `brownie` tool is aware of:
      ```shell
      brownie networks list True
      ```

### Deploying contracts
By default, the deployment is done on the `development` network = locally running Ganage node:
```shell
brownie run deploy_bridge.py
```
, in which case it is not necessary to provide the `--network development` command-line switch.

When deploying contract(s) on desired network, it is necessary to provide the `--network NETORK_NAME` command-line 
switch, as in the example bellow, where `NETWORK_NAME` shall be substituted by one of network `id` listed in the 
`networks` dictionary in the `networks-config.yaml` configuration file, for example, deployment on `mainnet` would
look like this:
```shell
brownie deploy_bridge.py --network mainnet 
```

List of currently configured networks in brownie tool, can be queried by running following command:
```shell
 brownie networks list #true
```
, where the `true` parameter at the end command-line would make the list printout verbose, showing full details.
