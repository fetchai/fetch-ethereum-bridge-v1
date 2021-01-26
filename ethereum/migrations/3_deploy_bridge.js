const {FET_ERC20, Contract} = require("../utility/constants.js");
const Bridge = artifacts.require("Bridge");
const ERC20Token = artifacts.require("FetERC20Mock");

module.exports = function(deployer, network) {
    if (!network.includes("development")) {
        return;
    }

    deployer.deploy(Bridge,
        ERC20Token.address,
        Contract.Status.INITIAL_CAP,
        Contract.Status.INITIAL_UPPER_SWAP_LIMIT,
        Contract.Status.INITIAL_LOWER_SWAP_LIMIT,
        Contract.Status.INITIAL_SWAP_FEE,
        Contract.Status.INITIAL_PAUSED_SINCE_BLOCK,
        Contract.Status.INITIAL_DELETION_PROTECTION_PERIOD);
};
