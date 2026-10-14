use crate::contract::msg::{FullStateResponse, PausedSinceBlockResponse, QueryMsg};
use crate::error::not_implemented_error;
use crate::state::State;
use cosmwasm_std::{to_json_binary, Binary, Deps, Env, StdResult};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw_fetch_std::access_control::handle_access_control_query_msg;
use cw_fetch_std::pausing::paused_since_block;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::PendingMessages {} => Err(not_implemented_error()),
        QueryMsg::PausedSince {} => to_json_binary(&PausedSinceBlockResponse {
            block: paused_since_block(deps.storage)?,
        }),
        QueryMsg::FullState {} => to_json_binary(&FullStateResponse {
            state: State::load(deps.storage)?,
        }),
        QueryMsg::AccessControl(msg) => handle_access_control_query_msg(deps, env, msg),
    }
}
