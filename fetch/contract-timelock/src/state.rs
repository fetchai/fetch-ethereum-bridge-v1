use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

const STATE: Item<StateVal> = Item::new("state");

#[cw_serde]
pub struct StateVal {
    pub paused_since_block: u64,
    pub bridge_addr: Addr,
    pub reverse_swap_lock_duration: u64,
    pub refund_lock_duration: u64,
    pub new_relay_eon_lock_duration: u64,
}

#[cw_serde]
pub struct BridgeMessage {
    pub payload: BridgeMessagePayload,
    pub execution_time: u64,
}

#[cw_serde]
pub enum BridgeMessagePayload {
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
}


const BRIDGE_MESSAGE_QUEUE: Map<u64, BridgeMessage> =
    Map::new("bridge_queue");
