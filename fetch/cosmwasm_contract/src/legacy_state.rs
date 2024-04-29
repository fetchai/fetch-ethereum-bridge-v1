use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::Uint128;
use cosmwasm_std::{Addr, Storage};
use cosmwasm_storage::{singleton, singleton_read, ReadonlySingleton, Singleton};

pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct LegacyState {
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
    pub contract_addr_human: Addr,
}

pub fn legacy_config_read(storage: &dyn Storage) -> ReadonlySingleton<LegacyState> {
    singleton_read(storage, CONFIG_KEY)
}

#[allow(dead_code)]
pub fn legacy_config(storage: &mut dyn Storage) -> Singleton<LegacyState> {
    singleton(storage, crate::state::CONFIG_KEY)
}
