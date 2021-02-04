use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

use crate::msg::Uint128;

pub static CONFIG_KEY: &[u8] = b"config";

pub const DELEGATE_ROLE: &[u8] = b"DELEGATE_ROLE";
pub const RELAYER_ROLE: &[u8] = b"RELAYER_ROLE";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    pub supply: Uint128,               // amount of token migrated to the other chain
    pub refunds_fees_accrued: Uint128, // fees used to pay for refunds
    pub next_swap_id: u64,
    pub sealed_reverse_swap_id: u64,
    pub relay_eon: u64,
    pub upper_swap_limit: Uint128,
    pub lower_swap_limit: Uint128,
    pub cap: Uint128,
    pub swap_fee: Uint128,
    pub paused_since_block: u64,
    // (1) Refund operation
    // is excuted when a swap can be finalized on the other chain
    // the case when that happen include:
    // - an error in the destination address: malformed wallet, invalid destination address
    // - failure to finalized the swap command on the other chain:
    //    + error in the contract
    //    + on the dest chain: highly imporbable for ether and mostly probable for cosmos native
    // Refund will rebalance the `supply`
    // (2) TODO(LR)(low) check if this is possible in cosmos
    // Objective: end of the life of the contract
    pub earliest_delete: u64, // delete the whole state of the contract and the stored contract

    // access control
    pub admin: CanonicalAddr,
    pub relayer: CanonicalAddr,

    // temporary
    pub denom: String,
}

pub fn config<S: Storage>(storage: &mut S) -> Singleton<S, State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read<S: Storage>(storage: &S) -> ReadonlySingleton<S, State> {
    singleton_read(storage, CONFIG_KEY)
}
