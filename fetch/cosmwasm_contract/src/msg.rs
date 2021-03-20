use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::HumanAddr;

//use crate::cosmwasm_bignumber::{Uint256};

pub type Uint128 = cosmwasm_std::Uint128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub cap: Uint128,
    pub upper_swap_limit: Uint128,
    pub lower_swap_limit: Uint128,
    pub swap_fee: Uint128,
    pub aggregated_reverse_limit: Uint128,
    pub paused_since_block: Option<u64>,
    pub delete_protection_period: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    // user level methods
    Swap {
        destination: String,
    },

    // relayer
    ReverseSwap {
        rid: u64,
        to: HumanAddr,
        sender: String,
        origin_tx_hash: String, // TOD(LR) should be [32]u8 or String
        amount: Uint128,
        relay_eon: u64,
    },

    Refund {
        id: u64,
        to: HumanAddr,
        amount: Uint128,
        relay_eon: u64,
    },

    RefundInFull {
        id: u64,
        to: HumanAddr,
        amount: Uint128,
        relay_eon: u64,
    },

    Pause {
        since_block: u64,
    },

    NewRelayEon {},

    // admin
    Deposit {},

    Withdraw {
        // withdrawal from contract supply to destination
        amount: Uint128,
        destination: HumanAddr,
    },

    WithdrawFees {
        // withdrawal from contract (account - supply) to destination
        amount: Uint128,
        destination: HumanAddr,
    },

    SetCap {
        amount: Uint128,
    },

    SetAggregatedReverseLimit {
        amount: Uint128,
    },

    SetLimits {
        swap_min: Uint128,
        swap_max: Uint128,
        swap_fee: Uint128,
    },

    // Access Control
    GrantRole {
        role: String,
        address: HumanAddr,
    },

    RevokeRole {
        role: String,
        address: HumanAddr,
    },

    RenounceRole {
        role: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // Access Control
    HasRole { role: u64, address: HumanAddr },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RoleResponse {
    pub has_role: bool,
}
