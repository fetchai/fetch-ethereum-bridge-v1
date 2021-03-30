use cosmwasm_std::StdError;
use thiserror::Error;

/* *********************** *
* Contract error messages *
************************* *
NB(LR) **don't** change, used as keywords by external code
*/

// limits error messages
pub const ERR_SWAP_LIMITS_INCONSISTENT: &str =
    "[FET_ERR_SWAP_LIMITS_INCONSISTENT] inconsistent swap fee and swap limits";
pub const ERR_SWAP_LIMITS_VIOLATED: &str = "[FET_ERROR_SWAP_LIMITS] Swap limits violated";
pub const ERR_SUPPLY_EXCEEDED: &str = "[FET_ERR_SUPPLY_EXCEEDED] Amount would exceed supply";
pub const ERR_RA_ALLOWANCE_EXCEEDED: &str =
    "[FET_ERR_RA_ALLOWANCE_EXCEEDED] Amount would exceed reverse aggregated allowance";
pub const ERR_CAP_EXCEEDED: &str = "[FET_ERR_CAP_EXCEEDED] Amount would exceed cap";
// access control error messages
pub const ERR_ACCESS_CONTROL: &str = "[FET_ERR_ACCESS_CONTROL] ";
pub const ERR_ACCESS_CONTROL_ONLY_ADMIN: &str = "[FET_ERR_ACCESS_CONTROL] Only Admin";
pub const ERR_ACCESS_CONTROL_ONLY_RELAYER: &str = "[FET_ERR_ACCESS_CONTROL] Only Relayer";
pub const ERR_ACCESS_CONTROL_ALREADY_HAVE_ROLE: &str = "[FET_ERR_ACCESS_CONTROL] Already have role";
pub const ERR_ACCESS_CONTROL_DONT_HAVE_ROLE: &str = "[FET_ERR_ACCESS_CONTROL] Don't have role";
// funds
pub const ERR_UNRECOGNIZED_DENOM: &str = "[FET_ERR_UNRECOGNIZED_DENOM] unrecognized denom";
// api paused
pub const ERR_CONTRACT_PAUSED: &str = "[FET_ERR_CONTRACT_PAUSED] Contract is paused";
// eon
pub const ERR_EON: &str = "[FET_ERR_EON] Tx doesn't belong to current relayEon";
// refund
pub const ERR_INVALID_SWAP_ID: &str = "[FET_ERR_INVALID_SWAP_ID] Invalid swap id";
pub const ERR_ALREADY_REFUNDED: &str = "[FET_ERR_ALREADY_REFUNDED] Refund was already processed";
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("FET_ERR_CODE_1")]
    Unauthorized {},
}
