const BN = require('bn.js');
const { web3 } = require('@openzeppelin/test-environment');

const decimals = 18;
const multiplier = (new BN(10)).pow(new BN(decimals)); // = 10 [FET] = 10**18


exports.FET_ERC20 = {
    _name : "Fetch.AI",
    _symbol : "FET",
    // source codes seems to require initial supply to already be multiplied by _decimals
    _initialSupply : new BN("1152997575").mul(multiplier),
    _decimals : decimals,
    //_mintable : false,
    multiplier: multiplier  // according to decimals, to convert [FET] into the [CanonicalFET] unit
};


exports.Contract = {Status: {
                     INITIAL_CAP: new BN(1000000).mul(multiplier),
                     INITIAL_UPPER_SWAP_LIMIT: new BN(100).mul(multiplier), // = 0.1 = 10%
                     INITIAL_LOWER_SWAP_LIMIT: new BN(10).mul(multiplier), // = (2**256)-1 = ~uint256(0) = 0xFF...FF (for all 32 bytes)
                     INITIAL_SWAP_FEE: new BN(1).mul(multiplier), // = 1 [FET]
                     INITIAL_PAUSED_SINCE_BLOCK: (new BN(0)).notn(256), // = (2**256)-1 = ~uint256(0) = 0xFF...FF (for all 32 bytes)
                     INITIAL_DELETION_PROTECTION_PERIOD: new BN(10), // 10 blocks
                     DEFAULT_ADMIN_ROLE: '0x0000000000000000000000000000000000000000000000000000000000000000',
                     DELEGATE_ROLE: web3.utils.soliditySha3('DELEGATE_ROLE')
                 }};
