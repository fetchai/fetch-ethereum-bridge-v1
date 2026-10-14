use crate::contract::migrate::{CONTRACT_NAME, CONTRACT_VERSION};
use crate::contract::msg::InstantiateMsg;
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdError, StdResult};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::set_contract_version;
use cw_fetch_std::access_control::AccessControl;
use cw_fetch_std::events::ResponseHandler;
use cw_fetch_std::pausing::pause_contract;

pub fn initialise_contract_state(
    deps: &mut DepsMut,
    response_handler: &mut ResponseHandler,
    msg: InstantiateMsg,
) -> StdResult<()> {
    // Initialise contract storage variables
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    msg.state.save(deps.storage, deps.api, response_handler)?;

    if let Some(paused_since_block) = msg.paused_since_block {
        pause_contract(deps.storage, response_handler, paused_since_block)?;
    }

    if let Some(init_roles) = msg.roles {
        if init_roles.roles.len() != init_roles.addresses.len() {
            return Err(StdError::msg("Invalid roles length"));
        }
        for (role, address) in init_roles.roles.iter().zip(init_roles.addresses) {
            AccessControl::_grant_role_unrestricted(
                deps.storage,
                response_handler,
                role,
                &deps.api.addr_validate(&address)?,
            )?
        }
    }

    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut response_handler = ResponseHandler::default();

    initialise_contract_state(&mut deps, &mut response_handler, msg)?;

    Ok(response_handler
        .into_response()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}
