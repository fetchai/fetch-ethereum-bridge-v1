use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {

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

    NewRelayEon {},

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

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {

    #[returns(RoleResponse)]
    HasRole { role: String, address: Addr },

    #[returns(PausedSinceBlockResponse)]
    PausedSince {},

    #[returns(FullStateResponse)]
    FullState {},
}

#[cw_serde]
pub struct RoleResponse {
    pub has_role: bool,
}

#[cw_serde]
pub struct PausedSinceBlockResponse {
    pub block: u64,
}

#[cw_serde]
pub struct FullStateResponse {
}
