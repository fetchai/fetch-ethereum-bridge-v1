use crate::queue::BridgeMessage;
use crate::queue::NonceType;
use crate::state::{State, StateUpdate};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_fetch_std::access_control::{AccessControlExecuteMsg, AccessControlQueryMsg};
use cw_fetch_std::query_responses_hybrid_derive::QueryResponsesHybrid;

#[cw_serde]
pub struct MigrateMsg {
    pub forced: Option<bool>, // Some(True) skips migration checks
    pub instantiate_msg: Option<InstantiateMsg>,
}

#[cw_serde]
pub struct InitRoles {
    pub roles: Vec<String>,
    pub addresses: Vec<String>,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub state: State,
    pub paused_since_block: Option<u64>,
    pub roles: Option<InitRoles>,
}

#[cw_serde]
pub enum ExecuteMsg {
    // Submit
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

    // Execute
    Execute {
        nonce: NonceType,
    },

    // Cancel
    Cancel {
        nonce: NonceType,
    },

    // Admin
    /// Changes only the provided fields, omitted or `null` fields keep their current value.
    UpdateState(StateUpdate),

    Pause {
        since_block: u64,
    },

    Resume {},

    // Access Control
    AccessControl(AccessControlExecuteMsg),
}

#[cw_serde]
#[derive(QueryResponsesHybrid)]
pub enum QueryMsg {
    #[returns(MessageResponse)]
    Message { nonce: NonceType },

    /// Pending messages in execution order, starting from the next one to execute.
    #[returns(PendingMessagesResponse)]
    PendingMessages {
        start_after: Option<NonceType>,
        limit: Option<u32>,
    },

    /// Reverse swaps with the given `rid` in submission order.
    #[returns(MessagesResponse)]
    ReverseSwapMessages {
        rid: u64,
        start_after: Option<NonceType>,
        limit: Option<u32>,
    },

    /// Refunds and refunds in full with the given `id` in submission order.
    #[returns(MessagesResponse)]
    RefundMessages {
        id: u64,
        start_after: Option<NonceType>,
        limit: Option<u32>,
    },

    /// The last submitted reverse swap with the given `rid`.
    #[returns(LastMessageResponse)]
    LastReverseSwapMessage { rid: u64 },

    /// The last submitted refund or refund in full with the given `id`.
    #[returns(LastMessageResponse)]
    LastRefundMessage { id: u64 },

    /// Number of pending messages, submitting is rejected once it reaches the state's
    /// `n_max_pending_messages`.
    #[returns(NPendingMessagesResponse)]
    NPendingMessages {},

    #[returns(PausedSinceBlockResponse)]
    PausedSince {},

    #[returns(FullStateResponse)]
    FullState {},

    #[nested]
    AccessControl(AccessControlQueryMsg),
}

#[cw_serde]
pub struct PausedSinceBlockResponse {
    pub block: Option<u64>,
}

#[cw_serde]
pub struct FullStateResponse {
    pub state: State,
}

#[cw_serde]
pub struct MessageResponse {
    pub message: Option<BridgeMessage>,
}

#[cw_serde]
pub struct PendingMessagesResponse {
    pub messages: Vec<(NonceType, BridgeMessage)>,
}

#[cw_serde]
pub struct MessagesResponse {
    pub messages: Vec<(NonceType, BridgeMessage)>,
}

#[cw_serde]
pub struct LastMessageResponse {
    pub message: Option<(NonceType, BridgeMessage)>,
}

#[cw_serde]
pub struct NPendingMessagesResponse {
    pub n_pending_messages: u64,
}
