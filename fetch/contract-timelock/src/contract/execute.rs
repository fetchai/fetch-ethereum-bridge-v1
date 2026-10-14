use crate::contract::msg::ExecuteMsg;
use crate::error::not_implemented_error;
use crate::state::{State, StateUpdate};
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw_fetch_std::access_control::{
    handle_access_control_execute_msg, AccessControl, DEFAULT_ADMIN_ROLE,
};
use cw_fetch_std::events::ResponseHandler;
use cw_fetch_std::pausing::{pause_contract, resume_contract};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        // Relayer messages
        ExecuteMsg::ReverseSwap { .. }
        | ExecuteMsg::Refund { .. }
        | ExecuteMsg::RefundInFull { .. }
        | ExecuteMsg::NewRelayEon {} => Err(not_implemented_error()),

        // Message queue
        ExecuteMsg::Execute { .. } => Err(not_implemented_error()),

        // DEFAULT_ADMIN_ROLE messages
        ExecuteMsg::UpdateState(update) => execute_update_state(deps, info, update),
        ExecuteMsg::Pause { since_block } => execute_pause(deps, info, since_block),
        ExecuteMsg::Resume {} => execute_resume(deps, info),

        ExecuteMsg::AccessControl(access_control_msg) => {
            handle_access_control_execute_msg(deps, env, info, access_control_msg)
        }
    }
}

pub fn execute_update_state(
    deps: DepsMut,
    info: MessageInfo,
    update: StateUpdate,
) -> StdResult<Response> {
    // Admin only
    AccessControl::ensure_has_role(deps.storage, DEFAULT_ADMIN_ROLE, &info.sender)?;
    let mut response_handler = ResponseHandler::default();

    State::update(deps.storage, deps.api, &mut response_handler, update)?;

    Ok(response_handler
        .into_response()
        .add_attribute("action", "update_state")
        .add_attribute("sender", info.sender))
}

pub fn execute_pause(deps: DepsMut, info: MessageInfo, since_block: u64) -> StdResult<Response> {
    // Admin only
    AccessControl::ensure_has_role(deps.storage, DEFAULT_ADMIN_ROLE, &info.sender)?;
    let mut response_handler = ResponseHandler::default();

    pause_contract(deps.storage, &mut response_handler, since_block)?;

    Ok(response_handler
        .into_response()
        .add_attribute("action", "pause")
        .add_attribute("sender", info.sender))
}

pub fn execute_resume(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    // Admin only
    AccessControl::ensure_has_role(deps.storage, DEFAULT_ADMIN_ROLE, &info.sender)?;
    let mut response_handler = ResponseHandler::default();

    resume_contract(deps.storage, &mut response_handler);

    Ok(response_handler
        .into_response()
        .add_attribute("action", "resume")
        .add_attribute("sender", info.sender))
}
