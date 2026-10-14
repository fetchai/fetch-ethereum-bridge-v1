use cosmwasm_std::testing::{
    message_info, mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{from_json, Addr, Deps, DepsMut, OwnedDeps, Response, StdResult};
use cw_fetch_std::access_control::DEFAULT_ADMIN_ROLE;
use serde::de::DeserializeOwned;

use crate::contract::execute::execute;
use crate::contract::instantiate::instantiate;
use crate::contract::msg::{ExecuteMsg, InitRoles, InstantiateMsg, QueryMsg};
use crate::contract::query::query;
use crate::state::State;

pub fn init_state(api: &MockApi) -> State {
    State {
        bridge_addr: api.addr_make("bridge"),
        reverse_swap_lock_delay: 10,
        refund_lock_delay: 20,
        new_relay_eon_lock_delay: 30,
    }
}

/// Instantiate message granting `admin` the DEFAULT_ADMIN_ROLE.
pub fn init_msg(api: &MockApi, admin: &Addr) -> InstantiateMsg {
    InstantiateMsg {
        state: init_state(api),
        paused_since_block: None,
        roles: Some(InitRoles {
            roles: vec![DEFAULT_ADMIN_ROLE.to_string()],
            addresses: vec![admin.to_string()],
        }),
    }
}

/// Instantiated contract with an admin.
pub fn setup() -> (OwnedDeps<MockStorage, MockApi, MockQuerier>, Addr) {
    let mut deps = mock_dependencies();
    let admin = deps.api.addr_make("admin");
    let msg = init_msg(&deps.api, &admin);
    instantiate_contract(deps.as_mut(), &admin, &msg).unwrap();
    (deps, admin)
}

pub fn instantiate_contract(
    deps: DepsMut,
    creator: &Addr,
    init_msg: &InstantiateMsg,
) -> StdResult<Response> {
    let info = message_info(creator, &[]);
    instantiate(deps, mock_env(), info, init_msg.clone())
}

pub fn execute_contract(deps: DepsMut, sender: &Addr, msg: ExecuteMsg) -> StdResult<Response> {
    let info = message_info(sender, &[]);
    execute(deps, mock_env(), info, msg)
}

pub fn query_contract<T: DeserializeOwned>(deps: Deps, msg: QueryMsg) -> StdResult<T> {
    from_json(query(deps, mock_env(), msg)?)
}

pub fn event_names(response: &Response) -> Vec<&str> {
    response
        .events
        .iter()
        .map(|event| event.ty.as_str())
        .collect()
}
