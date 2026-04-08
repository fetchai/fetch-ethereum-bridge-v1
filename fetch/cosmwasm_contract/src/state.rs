use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::Uint128;
use cosmwasm_std::storage_keys::to_length_prefixed;
use cosmwasm_std::{Addr, Storage};
use cw_storage_plus::Item;

// To keep backwards compatibility with cosmwasm_storage::singleton
// we must use the same length-prefixed key format.
pub static CONFIG_KEY: &str = "\u{0}\u{6}config";
pub static REFUNDS_KEY: &str = "refunds";

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
    pub use_mint_burn: bool,

    // optimization FIXME(LR) Not needed any more with version 0.10.0
    pub contract_addr_human: Addr,
}

pub const CONFIG: Item<State> = Item::new(CONFIG_KEY);

fn refunds_key(swap_id: u64) -> Vec<u8> {
    let mut k = to_length_prefixed(REFUNDS_KEY.as_bytes());
    k.extend_from_slice(&swap_id.to_be_bytes());
    k
}

pub fn refunds_add(swap_id: u64, storage: &mut dyn Storage) {
    let key = refunds_key(swap_id);
    storage.set(&key, &[1]);
}

pub fn refunds_have(swap_id: u64, storage: &dyn Storage) -> bool {
    let key = refunds_key(swap_id);
    storage.get(&key).is_some()
}
