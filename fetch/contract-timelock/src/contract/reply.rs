use crate::queue::MessageQueue;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Env, Reply, Response, StdResult, SubMsgResult};
use cw_fetch_std::events::ResponseHandler;

/// Result of a bridge message dispatched by `Execute`, the reply ID is the message nonce.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    let mut response_handler = ResponseHandler::default();
    let nonce = msg.id;

    MessageQueue::finish_execution(
        deps.storage,
        &mut response_handler,
        nonce,
        msg.result.is_ok(),
    )?;

    let response = response_handler
        .into_response()
        .add_attribute("action", "execution_result")
        .add_attribute("nonce", nonce.to_string());
    Ok(match msg.result {
        SubMsgResult::Ok(_) => response,
        SubMsgResult::Err(error) => response.add_attribute("error", error),
    })
}
