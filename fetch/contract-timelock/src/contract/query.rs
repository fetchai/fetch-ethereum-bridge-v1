use crate::contract::msg::{
    FullStateResponse, MessageResponse, PausedSinceBlockResponse, PendingMessagesResponse, QueryMsg,
};
use crate::queue::MessageQueue;
use crate::state::State;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Binary, Deps, Env, StdResult};
use cw_fetch_std::access_control::handle_access_control_query_msg;
use cw_fetch_std::pausing::paused_since_block;

const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Message { nonce } => to_json_binary(&MessageResponse {
            message: MessageQueue::get(deps.storage, nonce)?,
        }),
        QueryMsg::PendingMessages { start_after, limit } => {
            let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
            to_json_binary(&PendingMessagesResponse {
                messages: MessageQueue::pending(deps.storage, start_after, limit)?,
            })
        }

        QueryMsg::PausedSince {} => to_json_binary(&PausedSinceBlockResponse {
            block: paused_since_block(deps.storage)?,
        }),
        QueryMsg::FullState {} => to_json_binary(&FullStateResponse {
            state: State::load(deps.storage)?,
        }),
        QueryMsg::AccessControl(msg) => handle_access_control_query_msg(deps, env, msg),
    }
}
