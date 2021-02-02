use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, HandleResponse, InitResponse, MessageInfo, StdResult,
    HumanAddr,
};

use crate::error::ContractError;
use crate::msg::{CountResponse, HandleMsg, InitMsg, QueryMsg};

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
pub fn init(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InitMsg,
) -> Result<InitResponse, ContractError> {
    Ok(InitResponse::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
pub fn handle(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: HandleMsg,
) -> Result<HandleResponse, ContractError> {
    match msg {
        HandleMsg::Swap { amount: _, destination: _ } => Ok(HandleResponse::default()),
        HandleMsg::ReverseSwap { rid: _, to: _, from: _, origin_tx_hash: _, amount: _, relay_eon: _ } => Ok(HandleResponse::default()),
        HandleMsg::Refund { id: _, to: _, amount: _, relay_eon: _ } => Ok(HandleResponse::default()),
        HandleMsg::Pause { since_block: _ } => Ok(HandleResponse::default()),
        HandleMsg::FreezeFunds {amount: _}  => Ok(HandleResponse::default()),
        HandleMsg::UnFreezeFunds {amount: _}  => Ok(HandleResponse::default()),
        HandleMsg::SetCap { amount: _ } => Ok(HandleResponse::default()),
        HandleMsg::SetLimits { swap_min: _, swap_max: _, swap_fee: _ } => Ok(HandleResponse::default()),
        HandleMsg::GrantRole { role: _, address: _ } => Ok(HandleResponse::default()),
        HandleMsg::RevokeRole { role: _, address: _ } => Ok(HandleResponse::default()),
        HandleMsg::RenounceRole { role: _, address: _ } => Ok(HandleResponse::default()),
    }
}


pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::HasRole { role, address} => to_binary(&query_role(deps, role, address)?),
    }
}

fn query_role(_deps: Deps, _role : u64, _address : HumanAddr) -> StdResult<CountResponse> {
    Ok(CountResponse { has_role: false })
}
