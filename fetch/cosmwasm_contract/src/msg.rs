use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;

//use crate::cosmwasm_bignumber::{Uint256};

pub type Uint128 = cosmwasm_std::Uint128;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub next_swap_id: u64,
    pub cap: Uint128,
    pub upper_swap_limit: Uint128,
    pub lower_swap_limit: Uint128,
    pub swap_fee: Uint128,
    pub reverse_aggregated_allowance: Uint128,
    pub reverse_aggregated_allowance_approver_cap: Uint128,
    pub paused_since_block: Option<u64>,
    pub denom: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // user level methods
    Swap {
        destination: String,
    },

    // relayer
    ReverseSwap {
        rid: u64,
        to: Addr,
        sender: String,
        origin_tx_hash: String, // TOD(LR) should be [32]u8 or String
        amount: Uint128,
        relay_eon: u64,
    },

    Refund {
        id: u64,
        to: Addr,
        amount: Uint128,
        relay_eon: u64,
    },

    RefundInFull {
        id: u64,
        to: Addr,
        amount: Uint128,
        relay_eon: u64,
    },

    PausePublicApi {
        since_block: u64,
    },

    PauseRelayerApi {
        since_block: u64,
    },

    NewRelayEon {},

    // admin
    Deposit {},

    Withdraw {
        // withdrawal from contract supply to destination
        amount: Uint128,
        destination: Addr,
    },

    WithdrawFees {
        // withdrawal from contract (account - supply) to destination
        amount: Uint128,
        destination: Addr,
    },

    SetCap {
        amount: Uint128,
    },

    SetReverseAggregatedAllowance {
        amount: Uint128,
    },

    SetReverseAggregatedAllowanceApproverCap {
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
        address: Addr,
    },

    RevokeRole {
        role: String,
        address: Addr,
    },

    RenounceRole {
        role: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    HasRole { role: String, address: Addr },
    RelayEon {},
    Supply {},
    ReverseAggregatedAllowance {},
    SwapMax {},
    Cap {},
    PausedPublicApiSince {},
    PausedRelayerApiSince {},
    Denom {},
    FullState {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RoleResponse {
    pub has_role: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RelayEonResponse {
    pub eon: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SupplyResponse {
    pub amount: Uint128,
}

pub type CapResponse = SupplyResponse;
pub type SwapMaxResponse = SupplyResponse;
pub type ReverseAggregatedAllowanceResponse = SupplyResponse;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PausedSinceBlockResponse {
    pub block: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DenomResponse {
    pub denom: String,
}
