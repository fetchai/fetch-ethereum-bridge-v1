use crate::contract::msg::ExecuteMsg;
use crate::error::{execution_time_overflow_error, too_many_pending_messages_error};
use crate::queue::NonceType;
use crate::queue::{BridgeMessagePayload, MessageQueue};
use crate::roles::{CANCEL_ROLE, EXECUTE_ROLE, SUBMIT_ROLE};
use crate::state::{State, StateUpdate};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, DepsMut, Env, MessageInfo, Response, StdResult, SubMsg, WasmMsg,
};
use cw_fetch_std::access_control::{
    handle_access_control_execute_msg, AccessControl, DEFAULT_ADMIN_ROLE,
};
use cw_fetch_std::events::ResponseHandler;
use cw_fetch_std::pausing::{ensure_not_paused, pause_contract, resume_contract};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        // SUBMIT_ROLE messages
        ExecuteMsg::ReverseSwap {
            rid,
            to,
            sender,
            origin_tx_hash,
            amount,
            relay_eon,
        } => execute_submit(
            deps,
            env,
            info,
            BridgeMessagePayload::ReverseSwap {
                rid,
                to,
                sender,
                origin_tx_hash,
                amount,
                relay_eon,
            },
        ),
        ExecuteMsg::Refund {
            id,
            to,
            amount,
            relay_eon,
        } => execute_submit(
            deps,
            env,
            info,
            BridgeMessagePayload::Refund {
                id,
                to,
                amount,
                relay_eon,
            },
        ),
        ExecuteMsg::RefundInFull {
            id,
            to,
            amount,
            relay_eon,
        } => execute_submit(
            deps,
            env,
            info,
            BridgeMessagePayload::RefundInFull {
                id,
                to,
                amount,
                relay_eon,
            },
        ),
        ExecuteMsg::NewRelayEon {} => {
            execute_submit(deps, env, info, BridgeMessagePayload::NewRelayEon {})
        }

        // EXECUTE_ROLE messages
        ExecuteMsg::Execute { nonce } => execute_message(deps, env, info, nonce),

        // CANCEL_ROLE messages
        ExecuteMsg::Cancel { nonce } => execute_cancel(deps, info, nonce),

        // DEFAULT_ADMIN_ROLE messages
        ExecuteMsg::UpdateState(update) => execute_update_state(deps, info, update),
        ExecuteMsg::Pause { since_block } => execute_pause(deps, info, since_block),
        ExecuteMsg::Resume {} => execute_resume(deps, info),

        ExecuteMsg::AccessControl(access_control_msg) => {
            handle_access_control_execute_msg(deps, env, info, access_control_msg)
        }
    }
}

pub fn execute_submit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    payload: BridgeMessagePayload,
) -> StdResult<Response> {
    AccessControl::ensure_has_role(deps.storage, SUBMIT_ROLE, &info.sender)?;
    ensure_not_paused(deps.storage, &env)?;
    let mut response_handler = ResponseHandler::default();

    // Reject invalid recipients now rather than failing on the bridge after the delay
    if let Some(to) = payload.recipient() {
        deps.api.addr_validate(to.as_str())?;
    }

    let state = State::load(deps.storage)?;
    if MessageQueue::n_pending(deps.storage)? >= state.n_max_pending_messages {
        return Err(too_many_pending_messages_error(
            state.n_max_pending_messages,
        ));
    }
    let execution_time = env
        .block
        .time
        .seconds()
        .checked_add(payload.lock_delay(&state))
        .ok_or_else(execution_time_overflow_error)?;
    let nonce = MessageQueue::push(deps.storage, &mut response_handler, payload, execution_time)?;

    Ok(response_handler
        .into_response()
        .add_attribute("action", "submit")
        .add_attribute("sender", info.sender)
        .add_attribute("nonce", nonce.to_string()))
}

pub fn execute_message(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: NonceType,
) -> StdResult<Response> {
    AccessControl::ensure_has_role(deps.storage, EXECUTE_ROLE, &info.sender)?;
    ensure_not_paused(deps.storage, &env)?;
    let mut response_handler = ResponseHandler::default();

    let message = MessageQueue::start_execution(
        deps.storage,
        &mut response_handler,
        nonce,
        env.block.time.seconds(),
    )?;
    let bridge_msg = WasmMsg::Execute {
        contract_addr: State::load(deps.storage)?.bridge_addr.into_string(),
        msg: to_json_binary(&message.payload)?,
        funds: vec![],
    };
    // The reply records the result, a failing bridge message does not revert the execution
    response_handler.add_submessage(SubMsg::reply_always(bridge_msg, nonce));

    Ok(response_handler
        .into_response()
        .add_attribute("action", "execute")
        .add_attribute("sender", info.sender)
        .add_attribute("nonce", nonce.to_string()))
}

pub fn execute_cancel(deps: DepsMut, info: MessageInfo, nonce: NonceType) -> StdResult<Response> {
    AccessControl::ensure_has_role(deps.storage, CANCEL_ROLE, &info.sender)?;
    let mut response_handler = ResponseHandler::default();

    MessageQueue::cancel(deps.storage, &mut response_handler, nonce)?;

    Ok(response_handler
        .into_response()
        .add_attribute("action", "cancel")
        .add_attribute("sender", info.sender)
        .add_attribute("nonce", nonce.to_string()))
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
