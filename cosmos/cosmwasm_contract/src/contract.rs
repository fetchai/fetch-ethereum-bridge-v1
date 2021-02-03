use cosmwasm_std::{
    log, to_binary, Api, Binary, Env, Extern, HandleResponse, InitResponse, Querier,
    StdResult, Storage, HumanAddr, /*CanonicalAddr, Coin, HandleResult,*/
};

use crate::msg::{RoleResponse, HandleMsg, InitMsg, QueryMsg, U256};
use crate::state::{config, /*config_read,*/ State};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _msg: InitMsg,
) -> StdResult<InitResponse> {
    let state = State {
        source: env.message.sender.clone(),
        next_swap_id: 0,
        relay_eon: 0,
    };

    config(&mut deps.storage).save(&state)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Swap { amount, destination} => try_swap(deps, env, amount, destination),
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

fn try_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    amount: U256,
    destination: String,
) -> StdResult<HandleResponse> {
   config(&mut deps.storage).update(|mut state| {
        state.next_swap_id += 1;
        Ok(state)
    })?;

    let log = vec![log("action", "swap"), log("destination", destination), log("amount", amount)];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::HasRole { role, address} => to_binary(&query_role(deps, role, address)?),
    }
}

fn query_role<S: Storage, A: Api, Q: Querier>(_deps: &Extern<S, A, Q>, _role : u64, _address : HumanAddr) -> StdResult<RoleResponse> {
    Ok(RoleResponse { has_role: true })
}