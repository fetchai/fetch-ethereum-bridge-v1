use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use cw_fetch_std::access_control::{AccessControlExecuteMsg, AccessControlQueryMsg};
use cw_fetch_std::query_responses_hybrid_derive::QueryResponsesHybrid;
use crate::queue::BridgeMessage;
use crate::state::{StateVal};

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

    Pause {
        since_block: u64,
    },

    // Message queue
    Execute {
        id: u64,
    },

    // Access Control
    AccessControl(AccessControlExecuteMsg),


}

#[cw_serde]
#[derive(QueryResponsesHybrid)]
pub enum QueryMsg {

    #[returns(PendingMessagesResponse)]
    PendingMessages {},

    #[returns(RoleResponse)]
    HasRole { role: String, address: Addr },

    #[returns(PausedSinceBlockResponse)]
    PausedSince {},

    #[returns(FullStateResponse)]
    FullState {},

    #[nested]
    AccessControl(AccessControlQueryMsg),
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
    state: StateVal,
}

#[cw_serde]
pub struct PendingMessagesResponse {
    pub messages: Vec<(u64, BridgeMessage)>,
}
