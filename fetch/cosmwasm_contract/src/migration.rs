use crate::access_control::wipe_all_roles_from_storage;
use crate::contract::initialise_contract_state;
use crate::error::ERR_ALREADY_REFUNDED;
use crate::msg::MigrateMsg;
use crate::state::is_state_valid;
use cosmwasm_std::{entry_point, DepsMut, Env, Response, StdError, StdResult};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> StdResult<Response> {
    // Remove all previously registered roles
    wipe_all_roles_from_storage(deps.storage);

    // Ensure correct contract state
    if let Some(re_init) = msg.re_init {
        initialise_contract_state(
            deps.storage,
            &env,
            &re_init.admin,
            re_init.supply,
            re_init.relay_eon,
            re_init.fees_accrued,
            &re_init.init_msg,
        )?;
    } else if !is_state_valid(deps.storage) {
        return Err(StdError::generic_err(ERR_ALREADY_REFUNDED));
    }

    Ok(Response::default())
}
