use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{HumanAddr, Uint128};

//use crate::cosmwasm_bignumber::{Uint256};

//pub type U256 = Uint256;
pub type U256 = Uint128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    
    // user level methods
    Swap {
        amount: U256,
        destination: String,
    },

    // relayer
    ReverseSwap {
        rid: u64,
        to: HumanAddr,
        from: String,
        origin_tx_hash: U256,
        amount: U256,
        relay_eon: u64,
    },

    Refund {
        id: u64,
        to: HumanAddr,
        amount: U256,
        relay_eon: u64,
    },

    Pause {
        since_block: u64,
    },


    // admin
    FreezeFunds { // add funds to contracts from the owner
        amount: U256,
    },

    UnFreezeFunds { // withdrawal from contract to the owner
        amount: U256,
    },

    SetCap {
        amount: U256,
    },

    SetLimits {
        swap_min: U256,
        swap_max: U256,
        swap_fee: U256,
    },

    // Access Control
    GrantRole {
        role: u64,
        address: HumanAddr,
    },

    RevokeRole {
        role: u64,
        address: HumanAddr,
    },

    RenounceRole {
        role: u64,
        address: HumanAddr, // has to be sender
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // Access Control
    HasRole {
        role: u64,
        address: HumanAddr,
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RoleResponse {
    pub has_role: bool,
}
