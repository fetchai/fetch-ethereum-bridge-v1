use cosmwasm_std::testing::{mock_dependencies, mock_env};
use cosmwasm_std::{attr, Addr, Deps};
use cw_fetch_std::access_control::{
    AccessControlExecuteMsg, AccessControlQueryMsg, QueryHasRoleResponse, DEFAULT_ADMIN_ROLE,
};

use crate::contract::migrate::{migrate, CONTRACT_NAME, CONTRACT_VERSION};
use crate::contract::msg::{
    ExecuteMsg, FullStateResponse, InitRoles, InstantiateMsg, MigrateMsg, PausedSinceBlockResponse,
    QueryMsg,
};
use crate::state::{State, StateUpdate};
use crate::testing::helpers::{
    event_names, execute_contract, init_msg, init_state, instantiate_contract, query_contract,
    setup,
};

fn has_admin_role(deps: Deps, addr: &Addr) -> bool {
    let msg = QueryMsg::AccessControl(AccessControlQueryMsg::QueryHasRole {
        addr: addr.clone(),
        role: DEFAULT_ADMIN_ROLE.to_string(),
    });
    query_contract::<QueryHasRoleResponse>(deps, msg)
        .unwrap()
        .has_role
}

fn query_state(deps: Deps) -> State {
    query_contract::<FullStateResponse>(deps, QueryMsg::FullState {})
        .unwrap()
        .state
}

fn query_paused_since(deps: Deps) -> Option<u64> {
    query_contract::<PausedSinceBlockResponse>(deps, QueryMsg::PausedSince {})
        .unwrap()
        .block
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();
    let creator = deps.api.addr_make("creator");
    let admin = deps.api.addr_make("admin");

    let msg = init_msg(&deps.api, &admin);
    let res = instantiate_contract(deps.as_mut(), &creator, &msg).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("method", "instantiate"),
            attr("owner", creator.as_str())
        ]
    );
    assert_eq!(
        event_names(&res),
        vec!["state_updated", "access_control_has_role_updated"]
    );

    assert_eq!(query_state(deps.as_ref()), init_state(&deps.api));
    assert_eq!(query_paused_since(deps.as_ref()), None);
    assert!(has_admin_role(deps.as_ref(), &admin));
    assert!(!has_admin_role(deps.as_ref(), &creator));

    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(
        (version.contract.as_str(), version.version.as_str()),
        (CONTRACT_NAME, CONTRACT_VERSION)
    );
}

#[test]
fn initialization_paused() {
    let mut deps = mock_dependencies();
    let admin = deps.api.addr_make("admin");
    let msg = InstantiateMsg {
        paused_since_block: Some(5),
        ..init_msg(&deps.api, &admin)
    };

    let res = instantiate_contract(deps.as_mut(), &admin, &msg).unwrap();
    assert!(event_names(&res).contains(&"contract_paused"));
    assert_eq!(query_paused_since(deps.as_ref()), Some(5));
}

#[test]
fn initialization_rejects_invalid_input() {
    let mut deps = mock_dependencies();
    let admin = deps.api.addr_make("admin");
    let valid = init_msg(&deps.api, &admin);

    let mismatched_roles = InstantiateMsg {
        roles: Some(InitRoles {
            roles: vec![DEFAULT_ADMIN_ROLE.to_string()],
            addresses: vec![],
        }),
        ..valid.clone()
    };
    let invalid_role_address = InstantiateMsg {
        roles: Some(InitRoles {
            roles: vec![DEFAULT_ADMIN_ROLE.to_string()],
            addresses: vec!["invalid".to_string()],
        }),
        ..valid.clone()
    };
    let invalid_bridge = InstantiateMsg {
        state: State {
            bridge_addr: Addr::unchecked("invalid"),
            ..valid.state.clone()
        },
        ..valid
    };

    for msg in [mismatched_roles, invalid_role_address, invalid_bridge] {
        assert!(instantiate_contract(deps.as_mut(), &admin, &msg).is_err());
    }
}

#[test]
fn update_state_is_admin_only() {
    let (mut deps, admin) = setup();
    let user = deps.api.addr_make("user");

    let update = StateUpdate {
        refund_lock_delay: Some(25),
        ..Default::default()
    };
    assert!(execute_contract(
        deps.as_mut(),
        &user,
        ExecuteMsg::UpdateState(update.clone())
    )
    .is_err());
    assert_eq!(query_state(deps.as_ref()), init_state(&deps.api));

    let res = execute_contract(deps.as_mut(), &admin, ExecuteMsg::UpdateState(update)).unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "update_state"),
            attr("sender", admin.as_str())
        ]
    );
    assert_eq!(event_names(&res), vec!["state_updated"]);
    assert_eq!(
        query_state(deps.as_ref()),
        State {
            refund_lock_delay: 25,
            ..init_state(&deps.api)
        }
    );
}

#[test]
fn pause_and_resume_are_admin_only() {
    let (mut deps, admin) = setup();
    let user = deps.api.addr_make("user");

    assert!(execute_contract(deps.as_mut(), &user, ExecuteMsg::Pause { since_block: 7 }).is_err());
    let res =
        execute_contract(deps.as_mut(), &admin, ExecuteMsg::Pause { since_block: 7 }).unwrap();
    assert_eq!(event_names(&res), vec!["contract_paused"]);
    assert_eq!(query_paused_since(deps.as_ref()), Some(7));

    assert!(execute_contract(deps.as_mut(), &user, ExecuteMsg::Resume {}).is_err());
    let res = execute_contract(deps.as_mut(), &admin, ExecuteMsg::Resume {}).unwrap();
    assert_eq!(event_names(&res), vec!["contract_resumed"]);
    assert_eq!(query_paused_since(deps.as_ref()), None);
}

#[test]
fn admin_can_grant_and_revoke_roles() {
    let (mut deps, admin) = setup();
    let user = deps.api.addr_make("user");

    let grant = ExecuteMsg::AccessControl(AccessControlExecuteMsg::GrantRole {
        role: DEFAULT_ADMIN_ROLE.to_string(),
        addr: user.clone(),
    });
    assert!(execute_contract(deps.as_mut(), &user, grant.clone()).is_err());
    execute_contract(deps.as_mut(), &admin, grant).unwrap();
    assert!(has_admin_role(deps.as_ref(), &user));

    // The new admin can update the state.
    let update = ExecuteMsg::UpdateState(StateUpdate {
        reverse_swap_lock_delay: Some(1),
        ..Default::default()
    });
    execute_contract(deps.as_mut(), &user, update.clone()).unwrap();

    let revoke = ExecuteMsg::AccessControl(AccessControlExecuteMsg::RevokeRole {
        role: DEFAULT_ADMIN_ROLE.to_string(),
        addr: user.clone(),
    });
    execute_contract(deps.as_mut(), &admin, revoke).unwrap();
    assert!(!has_admin_role(deps.as_ref(), &user));
    assert!(execute_contract(deps.as_mut(), &user, update).is_err());
}

#[test]
fn unimplemented_messages_return_errors() {
    let (mut deps, admin) = setup();

    assert!(execute_contract(deps.as_mut(), &admin, ExecuteMsg::NewRelayEon {}).is_err());
    assert!(execute_contract(deps.as_mut(), &admin, ExecuteMsg::Execute { id: 0 }).is_err());
    assert!(query_contract::<()>(deps.as_ref(), QueryMsg::PendingMessages {}).is_err());
}

#[test]
fn migrate_from_older_version() {
    let (mut deps, _) = setup();
    cw2::set_contract_version(&mut deps.storage, CONTRACT_NAME, "0.0.1").unwrap();

    let msg = MigrateMsg {
        forced: None,
        instantiate_msg: None,
    };
    migrate(deps.as_mut(), mock_env(), msg).unwrap();

    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(query_state(deps.as_ref()), init_state(&deps.api));
}

#[test]
fn migrate_rejects_invalid_source() {
    let (mut deps, _) = setup();
    let msg = MigrateMsg {
        forced: None,
        instantiate_msg: None,
    };

    // Same version
    assert!(migrate(deps.as_mut(), mock_env(), msg.clone()).is_err());

    // Different contract
    cw2::set_contract_version(&mut deps.storage, "other-contract", "0.0.1").unwrap();
    assert!(migrate(deps.as_mut(), mock_env(), msg).is_err());
}

#[test]
fn forced_migrate_requires_state() {
    let mut deps = mock_dependencies();
    let admin = deps.api.addr_make("admin");

    let without_state = MigrateMsg {
        forced: Some(true),
        instantiate_msg: None,
    };
    assert!(migrate(deps.as_mut(), mock_env(), without_state).is_err());

    let with_state = MigrateMsg {
        forced: Some(true),
        instantiate_msg: Some(init_msg(&deps.api, &admin)),
    };
    let res = migrate(deps.as_mut(), mock_env(), with_state).unwrap();
    assert_eq!(
        event_names(&res),
        vec!["state_updated", "access_control_has_role_updated"]
    );
    assert_eq!(query_state(deps.as_ref()), init_state(&deps.api));
    assert!(has_admin_role(deps.as_ref(), &admin));
}
