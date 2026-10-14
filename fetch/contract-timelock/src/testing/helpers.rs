use cosmwasm_std::testing::{
    message_info, mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{
    from_json, Addr, Binary, Deps, DepsMut, Env, OwnedDeps, Reply, Response, StdResult,
    SubMsgResponse, SubMsgResult, Timestamp,
};
use cw_fetch_std::access_control::DEFAULT_ADMIN_ROLE;
use serde::de::DeserializeOwned;

use crate::contract::execute::execute;
use crate::contract::instantiate::instantiate;
use crate::contract::msg::{ExecuteMsg, InitRoles, InstantiateMsg, QueryMsg};
use crate::contract::query::query;
use crate::contract::reply::reply;
use crate::roles::{CANCEL_ROLE, EXECUTE_ROLE, SUBMIT_ROLE};
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

pub struct Actors {
    pub admin: Addr,
    pub submitter: Addr,
    pub executor: Addr,
    pub canceller: Addr,
}

/// Instantiated contract with an admin and one address per queue role.
pub fn setup_with_roles() -> (OwnedDeps<MockStorage, MockApi, MockQuerier>, Actors) {
    let mut deps = mock_dependencies();
    let actors = Actors {
        admin: deps.api.addr_make("admin"),
        submitter: deps.api.addr_make("submitter"),
        executor: deps.api.addr_make("executor"),
        canceller: deps.api.addr_make("canceller"),
    };
    let roles = [DEFAULT_ADMIN_ROLE, SUBMIT_ROLE, EXECUTE_ROLE, CANCEL_ROLE];
    let addresses = [
        &actors.admin,
        &actors.submitter,
        &actors.executor,
        &actors.canceller,
    ];
    let msg = InstantiateMsg {
        roles: Some(InitRoles {
            roles: roles.map(str::to_string).to_vec(),
            addresses: addresses.map(Addr::to_string).to_vec(),
        }),
        ..init_msg(&deps.api, &actors.admin)
    };
    instantiate_contract(deps.as_mut(), &actors.admin, &msg).unwrap();
    (deps, actors)
}

pub fn env_at(seconds: u64) -> Env {
    let mut env = mock_env();
    env.block.time = Timestamp::from_seconds(seconds);
    env
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

pub fn execute_contract_at(
    deps: DepsMut,
    sender: &Addr,
    msg: ExecuteMsg,
    seconds: u64,
) -> StdResult<Response> {
    let info = message_info(sender, &[]);
    execute(deps, env_at(seconds), info, msg)
}

pub fn reply_contract(deps: DepsMut, nonce: u64, result: SubMsgResult) -> StdResult<Response> {
    let msg = Reply {
        id: nonce,
        payload: Binary::default(),
        gas_used: 0,
        result,
    };
    reply(deps, mock_env(), msg)
}

pub fn ok_result() -> SubMsgResult {
    #[allow(deprecated)]
    SubMsgResult::Ok(SubMsgResponse {
        events: vec![],
        data: None,
        msg_responses: vec![],
    })
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
