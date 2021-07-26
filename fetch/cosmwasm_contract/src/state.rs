use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::Uint128;
use cosmwasm_std::{HumanAddr, Storage};
use cosmwasm_storage::{
    singleton, singleton_read, PrefixedStorage, ReadonlyPrefixedStorage, ReadonlySingleton,
    Singleton,
};

pub static CONFIG_KEY: &[u8] = b"config";
pub static REFUNDS_KEY: &[u8] = b"refunds";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    pub supply: Uint128,       // amount of token migrated to the other chain
    pub fees_accrued: Uint128, // fees
    pub next_swap_id: u64,
    pub sealed_reverse_swap_id: u64,
    pub relay_eon: u64,
    pub upper_swap_limit: Uint128,
    pub lower_swap_limit: Uint128,
    pub reverse_aggregated_allowance: Uint128,
    pub reverse_aggregated_allowance_approver_cap: Uint128,
    pub cap: Uint128,
    pub swap_fee: Uint128,
    pub paused_since_block_public_api: u64,
    pub paused_since_block_relayer_api: u64,
    pub denom: String,

    // optimization FIXME(LR) Not needed any more with version 0.10.0
    pub contract_addr_human: HumanAddr,
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}

pub fn refunds_add(swap_id: u64, storage: &mut dyn Storage) {
    let mut store = PrefixedStorage::new(storage, REFUNDS_KEY);
    store.set(&swap_id.to_be_bytes(), &[1]);
}

pub fn refunds_have(swap_id: u64, storage: &dyn Storage) -> bool {
    let store = ReadonlyPrefixedStorage::new(storage, REFUNDS_KEY);
    match store.get(&swap_id.to_be_bytes()) {
        Some(_) => true,
        None => false,
    }
}
