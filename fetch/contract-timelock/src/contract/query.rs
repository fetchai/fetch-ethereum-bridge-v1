use crate::contract::msg::{
    FullStateResponse, LastMessageResponse, MessageResponse, MessagesResponse,
    NPendingMessagesResponse, PausedSinceBlockResponse, PendingMessagesResponse, QueryMsg,
};
use crate::queue::{MessageIndex, MessageQueue, NonceType};
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
            to_json_binary(&PendingMessagesResponse {
                messages: MessageQueue::pending(deps.storage, start_after, page_size(limit))?,
            })
        }
        QueryMsg::ReverseSwapMessages {
            rid,
            start_after,
            limit,
        } => query_by_index(deps, MessageIndex::ReverseSwap(rid), start_after, limit),
        QueryMsg::RefundMessages {
            id,
            start_after,
            limit,
        } => query_by_index(deps, MessageIndex::Refund(id), start_after, limit),
        QueryMsg::LastReverseSwapMessage { rid } => {
            query_last_by_index(deps, MessageIndex::ReverseSwap(rid))
        }
        QueryMsg::LastRefundMessage { id } => query_last_by_index(deps, MessageIndex::Refund(id)),
        QueryMsg::NPendingMessages {} => to_json_binary(&NPendingMessagesResponse {
            n_pending_messages: MessageQueue::n_pending(deps.storage)?,
        }),

        QueryMsg::PausedSince {} => to_json_binary(&PausedSinceBlockResponse {
            block: paused_since_block(deps.storage)?,
        }),
        QueryMsg::FullState {} => to_json_binary(&FullStateResponse {
            state: State::load(deps.storage)?,
        }),
        QueryMsg::AccessControl(msg) => handle_access_control_query_msg(deps, env, msg),
    }
}

fn page_size(limit: Option<u32>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize
}

fn query_by_index(
    deps: Deps,
    index: MessageIndex,
    start_after: Option<NonceType>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    to_json_binary(&MessagesResponse {
        messages: MessageQueue::by_index(deps.storage, index, start_after, page_size(limit))?,
    })
}

fn query_last_by_index(deps: Deps, index: MessageIndex) -> StdResult<Binary> {
    to_json_binary(&LastMessageResponse {
        message: MessageQueue::last_by_index(deps.storage, index)?,
    })
}
