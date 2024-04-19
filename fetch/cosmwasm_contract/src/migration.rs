use crate::access_control::unsafe_remove_all_roles;
use crate::contract::initialise_contract_state;
use crate::error::ERR_ALREADY_REFUNDED;
use crate::msg::MigrateMsg;
use crate::state::is_state_valid;
use cosmwasm_std::{entry_point, DepsMut, Env, MessageInfo, Response, StdError, StdResult};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, info: MessageInfo, msg: MigrateMsg) -> StdResult<Response> {
    // Remove all previously registered roles
    unsafe_remove_all_roles(deps.storage);

    // Ensure correct contract state
    if let Some(msg) = msg.instantiate_msg {
        initialise_contract_state(deps.storage, &env, &info, &msg)?;
    } else if !is_state_valid(deps.storage) {
        return Err(StdError::generic_err(ERR_ALREADY_REFUNDED));
    }

    Ok(Response::default())
}
