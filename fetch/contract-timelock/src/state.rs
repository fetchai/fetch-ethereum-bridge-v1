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

