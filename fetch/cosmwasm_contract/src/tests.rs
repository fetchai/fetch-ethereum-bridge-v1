use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{
    coins, BankMsg, CosmosMsg, Extern, HandleResponse, HumanAddr, InitResponse, MessageInfo,
    StdError, StdResult,
};

use crate::access_control::{
    ac_have_role, AccessRole, ADMIN_ROLE, APPROVER_ROLE, MONITOR_ROLE, RELAYER_ROLE,
};
use crate::contract::{
    amount_from_funds, handle, init, query, verify_not_paused_public_api,
    verify_not_paused_relayer_api,
};
use crate::error::{
    ERR_ACCESS_CONTROL_ALREADY_HAS_ROLE, ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE,
    ERR_ACCESS_CONTROL_ONLY_ADMIN, ERR_ACCESS_CONTROL_ONLY_RELAYER, ERR_ALREADY_REFUNDED,
    ERR_CAP_EXCEEDED, ERR_CONTRACT_PAUSED, ERR_EON, ERR_INVALID_SWAP_ID, ERR_RA_ALLOWANCE_EXCEEDED,
    ERR_SUPPLY_EXCEEDED, ERR_SWAP_LIMITS_INCONSISTENT, ERR_SWAP_LIMITS_VIOLATED,
    ERR_UNRECOGNIZED_DENOM,
};
use crate::msg::{HandleMsg, InitMsg, QueryMsg, Uint128};
use crate::state::{config_read, State};

pub const DEFAULT_OWNER: &str = "Owner";
pub const DEFAULT_DENUM: &str = "atestfet";
pub const DEFAULT_CAP: u128 = 100000u128;
pub const DEFAULT_RA_ALLOWANCE: u128 = 10000u128;
pub const DEFAULT_RA_ALLOWANCE_APPROVER_CAP: u128 = 5000u128;
//pub const DEFAULT_DEPOSIT: u128 = 10000u128;
pub const DEFAULT_SWAP_UPPER_LIMIT: u128 = 1000u128;
pub const DEFAULT_SWAP_LOWER_LIMIT: u128 = 110u128;
pub const DEFAULT_SWAP_FEE: u128 = 100u128;

pub const HAS_ROLE_TRUE: &[u8] = b"{\"has_role\":true}";
pub const HAS_ROLE_FALSE: &[u8] = b"{\"has_role\":false}";

macro_rules! cu128 {
    ($val:expr) => {
        // FIXME(LR) be more explicit of allowed expression
        Uint128::from($val)
    };
}

macro_rules! addr {
    ($val:expr) => {
        // FIXME(LR) be more explicit of allowed expression
        HumanAddr::from($val)
    };
}

macro_rules! str_from_binary {
    ($val:expr) => {
        std::str::from_utf8($val.as_slice()).unwrap()
    };
}

macro_rules! expect_error {
    ($val1:expr, $val2:expr) => {
        match $val1 {
            Ok(_) => panic!("expected error: {:?}", $val2),
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, $val2),
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    };
}

mod init {
    use super::*;

    fn mock_init(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        msg: InitMsg,
        caller: &str,
        amount: u128,
    ) -> StdResult<InitResponse> {
        let info = mock_info(caller, &coins(amount, DEFAULT_DENUM));
        return init(&mut deps, mock_env(), info, msg);
    }

    pub fn init_default(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
    ) -> StdResult<InitResponse> {
        let msg = InitMsg {
            cap: cu128!(DEFAULT_CAP),
            reverse_aggregated_allowance: cu128!(DEFAULT_RA_ALLOWANCE),
            reverse_aggregated_allowance_approver_cap: cu128!(DEFAULT_RA_ALLOWANCE_APPROVER_CAP),
            upper_swap_limit: cu128!(DEFAULT_SWAP_UPPER_LIMIT),
            lower_swap_limit: cu128!(DEFAULT_SWAP_LOWER_LIMIT),
            swap_fee: cu128!(DEFAULT_SWAP_FEE),
            paused_since_block: None,
            denom: Some(DEFAULT_DENUM.to_string()),
        };
        return mock_init(&mut deps, msg, DEFAULT_OWNER, 0);
    }

    #[test]
    fn success_init() {
        let mut deps = mock_dependencies(&[]);

        let response = init_default(&mut deps).unwrap();
        // check return
        assert_eq!(0, response.messages.len());
        assert_eq!(0, response.attributes.len());

        // check state
        let expected_state = State {
            supply: cu128!(0u128),
            fees_accrued: Uint128::from(0u128),
            next_swap_id: 0,
            sealed_reverse_swap_id: 0,
            relay_eon: 0,
            upper_swap_limit: cu128!(DEFAULT_SWAP_UPPER_LIMIT),
            lower_swap_limit: cu128!(DEFAULT_SWAP_LOWER_LIMIT),
            cap: cu128!(DEFAULT_CAP),
            reverse_aggregated_allowance: cu128!(DEFAULT_RA_ALLOWANCE),
            reverse_aggregated_allowance_approver_cap: cu128!(DEFAULT_RA_ALLOWANCE_APPROVER_CAP),
            swap_fee: cu128!(DEFAULT_SWAP_FEE),
            paused_since_block_public_api: u64::MAX,
            paused_since_block_relayer_api: u64::MAX,
            denom: DEFAULT_DENUM.to_string(),
            contract_addr_human: mock_env().contract.address,
        };

        let state = config_read(&deps.storage)
            .load()
            .expect("unexpected reading state error");
        assert_eq!(state, expected_state);

        // check role
        ac_have_role(
            &deps.storage,
            &HumanAddr::from(DEFAULT_OWNER),
            &AccessRole::Admin,
        )
        .expect("owner should have admin role");
    }

    #[test]
    fn failure_init_inconsistent_swap_limits_lower_larger_than_upper() {
        let mut deps = mock_dependencies(&[]);

        let msg = InitMsg {
            cap: cu128!(DEFAULT_CAP),
            reverse_aggregated_allowance: cu128!(DEFAULT_RA_ALLOWANCE),
            reverse_aggregated_allowance_approver_cap: cu128!(DEFAULT_RA_ALLOWANCE_APPROVER_CAP),
            upper_swap_limit: cu128!(DEFAULT_SWAP_LOWER_LIMIT),
            lower_swap_limit: cu128!(DEFAULT_SWAP_UPPER_LIMIT),
            swap_fee: cu128!(DEFAULT_SWAP_FEE),
            paused_since_block: None,
            denom: None,
        };
        let response = mock_init(&mut deps, msg, DEFAULT_OWNER, 1);
        expect_error!(response, ERR_SWAP_LIMITS_INCONSISTENT);
    }

    #[test]
    fn failure_init_inconsistent_swap_limits_fee_larger_than_lower() {
        let mut deps = mock_dependencies(&[]);

        let msg = InitMsg {
            cap: cu128!(DEFAULT_CAP),
            reverse_aggregated_allowance: cu128!(DEFAULT_RA_ALLOWANCE),
            reverse_aggregated_allowance_approver_cap: cu128!(DEFAULT_RA_ALLOWANCE_APPROVER_CAP),
            upper_swap_limit: cu128!(DEFAULT_SWAP_UPPER_LIMIT),
            lower_swap_limit: cu128!(DEFAULT_SWAP_FEE),
            swap_fee: cu128!(DEFAULT_SWAP_LOWER_LIMIT),
            paused_since_block: None,
            denom: None,
        };
        let response = mock_init(&mut deps, msg, DEFAULT_OWNER, 1);
        match response {
            Ok(_) => panic!("expected error"),
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, ERR_SWAP_LIMITS_INCONSISTENT),
            Err(e) => panic!("unexpected error: {:?}", e),
        }
    }
}

mod access_control {
    use super::*;
    use init::init_default;

    pub fn grant_role(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        role: &str,
        account: &str,
        caller: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::GrantRole {
            role: String::from(role),
            address: addr!(account),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn check_grant_role_success(
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        response: &HandleResponse,
        role: &str,
        account: &str,
    ) {
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(3, response.attributes.len());
        assert!(
            response.attributes[0].key == "action" && response.attributes[0].value == "grant_role"
        );
        assert!(response.attributes[1].key == "role" && response.attributes[1].value == role);
        assert!(response.attributes[2].key == "account" && response.attributes[2].value == account);

        // query
        let query_msg = QueryMsg::HasRole {
            role: String::from(role),
            address: addr!(account),
        };
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(HAS_ROLE_TRUE, response.as_slice());

        // state
        assert!(ac_have_role(
            &deps.storage,
            &addr!(account),
            &AccessRole::from_str(role).unwrap()
        )
        .unwrap());
    }

    pub fn revoke_role(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        role: &str,
        account: &str,
        caller: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::RevokeRole {
            role: String::from(role),
            address: addr!(account),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn check_revoke_role_success(
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        response: &HandleResponse,
        role: &str,
        account: &str,
    ) {
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(3, response.attributes.len());
        assert!(
            response.attributes[0].key == "action" && response.attributes[0].value == "revoke_role"
        );
        assert!(response.attributes[1].key == "role" && response.attributes[1].value == role);
        assert!(response.attributes[2].key == "account" && response.attributes[2].value == account);

        // query
        let query_msg = QueryMsg::HasRole {
            role: String::from(role),
            address: addr!(account),
        };
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(HAS_ROLE_FALSE, response.as_slice());

        // state
        assert!(!ac_have_role(
            &deps.storage,
            &addr!(account),
            &AccessRole::from_str(role).unwrap()
        )
        .unwrap());
    }

    pub fn renounce_role(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        role: &str,
        account: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::RenounceRole {
            role: String::from(role),
        };
        let info = mock_info(account, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn check_renounce_role_success(
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        response: &HandleResponse,
        role: &str,
        account: &str,
    ) {
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(3, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "renounce_role"
        );
        assert!(response.attributes[1].key == "role" && response.attributes[1].value == role);
        assert!(response.attributes[2].key == "account" && response.attributes[2].value == account);

        // query
        let query_msg = QueryMsg::HasRole {
            role: String::from(role),
            address: addr!(account),
        };
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(HAS_ROLE_FALSE, response.as_slice());

        // state
        assert!(!ac_have_role(
            &deps.storage,
            &addr!(account),
            &AccessRole::from_str(role).unwrap()
        )
        .unwrap());
    }

    fn test_grant_role(role: &str) {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        // call
        let account = "new_account";
        let response = grant_role(&mut deps, role, account, DEFAULT_OWNER).unwrap();

        // check handle response, query, and state
        check_grant_role_success(&mut deps, &response, role, account);
    }

    #[test]
    fn success_owner_default_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        // state
        assert!(ac_have_role(&deps.storage, &addr!(DEFAULT_OWNER), &AccessRole::Admin,).unwrap());

        // query
        let query_msg = QueryMsg::HasRole {
            role: String::from(ADMIN_ROLE),
            address: addr!(DEFAULT_OWNER),
        };
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(HAS_ROLE_TRUE, response.as_slice())
    }

    #[test]
    fn success_grant_role_admin() {
        test_grant_role(ADMIN_ROLE)
    }

    #[test]
    fn success_grant_role_approver() {
        test_grant_role(APPROVER_ROLE)
    }

    #[test]
    fn success_grant_role_relayer() {
        test_grant_role(RELAYER_ROLE)
    }

    #[test]
    fn success_grant_role_monitor() {
        test_grant_role(MONITOR_ROLE)
    }

    #[test]
    fn success_revoke_roles() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let mut account: &str;
        let mut role: &str;
        let caller = DEFAULT_OWNER;

        // admin
        account = "new_admin";
        role = ADMIN_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = revoke_role(&mut deps, role, account, caller).unwrap();
        check_revoke_role_success(&mut deps, &response, &role, &account);

        // approver
        account = "new_approver";
        role = APPROVER_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = revoke_role(&mut deps, role, account, caller).unwrap();
        check_revoke_role_success(&mut deps, &response, &role, &account);

        // relayer
        account = "new_relayer";
        role = RELAYER_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = revoke_role(&mut deps, role, account, caller).unwrap();
        check_revoke_role_success(&mut deps, &response, &role, &account);

        // monitor
        account = "new_monitor";
        role = MONITOR_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = revoke_role(&mut deps, role, account, caller).unwrap();
        check_revoke_role_success(&mut deps, &response, &role, &account);
    }

    #[test]
    fn success_renounce_roles() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let mut account;
        let mut role;
        let caller = DEFAULT_OWNER;

        // admin
        account = "new_admin";
        role = ADMIN_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = renounce_role(&mut deps, role, account).unwrap();
        check_renounce_role_success(&mut deps, &response, &role, &account);

        // approver
        account = "new_approver";
        role = APPROVER_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = renounce_role(&mut deps, role, account).unwrap();
        check_renounce_role_success(&mut deps, &response, &role, &account);

        // relayer
        account = "new_relayer";
        role = RELAYER_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = renounce_role(&mut deps, role, account).unwrap();
        check_renounce_role_success(&mut deps, &response, &role, &account);

        // monitor
        account = "new_monitor";
        role = MONITOR_ROLE;
        grant_role(&mut deps, role, account, caller).unwrap();
        let response = renounce_role(&mut deps, role, account).unwrap();
        check_renounce_role_success(&mut deps, &response, &role, &account);
    }

    #[test]
    fn failure_grant_role_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = "not_admin";
        let account = "new_relayer";
        let response = grant_role(&mut deps, RELAYER_ROLE, account, caller);
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
    }

    #[test]
    fn failure_grant_role_already_have_role() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = DEFAULT_OWNER;
        let account = "new_approver";
        grant_role(&mut deps, APPROVER_ROLE, account, caller).unwrap();

        let response = grant_role(&mut deps, APPROVER_ROLE, account, caller);
        expect_error!(response, ERR_ACCESS_CONTROL_ALREADY_HAS_ROLE);
    }

    #[test]
    fn failure_revoke_role_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let mut caller = DEFAULT_OWNER;
        let account = "new_monitor";
        grant_role(&mut deps, APPROVER_ROLE, account, caller).unwrap();

        caller = account;
        let response = revoke_role(&mut deps, RELAYER_ROLE, account, caller);
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
    }

    #[test]
    fn failure_revoke_role_doesnt_have_role() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = DEFAULT_OWNER;
        let account = "new_monitor";
        let response = revoke_role(&mut deps, MONITOR_ROLE, account, caller);
        expect_error!(response, ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE);
    }

    #[test]
    fn failure_renounce_role_doesnt_have_role() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let account = "not_admin";
        let role = ADMIN_ROLE;
        let response = renounce_role(&mut deps, role, account);
        expect_error!(response, ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE)
    }
}

mod pause {
    use super::*;
    use access_control::grant_role;
    use init::init_default;

    fn _pause_public_api(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        info: MessageInfo,
        since_block: u64,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::PausePublicApi { since_block };
        handle(&mut deps, mock_env(), info, msg)
    }

    fn _pause_relayer_api(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        info: MessageInfo,
        since_block: u64,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::PauseRelayerApi { since_block };
        handle(&mut deps, mock_env(), info, msg)
    }

    pub fn pause_public_api(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        info: MessageInfo,
    ) -> StdResult<HandleResponse> {
        _pause_public_api(&mut deps, info, 0)
    }

    pub fn pause_relayer_api(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        info: MessageInfo,
    ) -> StdResult<HandleResponse> {
        _pause_relayer_api(&mut deps, info, 0)
    }

    pub fn unpause_public_api(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        info: MessageInfo,
    ) -> StdResult<HandleResponse> {
        _pause_public_api(&mut deps, info, u64::MAX)
    }

    pub fn unpause_relayer_api(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        info: MessageInfo,
    ) -> StdResult<HandleResponse> {
        _pause_relayer_api(&mut deps, info, u64::MAX)
    }

    fn assert_pause_both(mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>, caller: &str) {
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));

        assert!(verify_not_paused_public_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_ok());
        assert!(verify_not_paused_relayer_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_ok());

        let mut response = pause_public_api(&mut deps, info.clone()).unwrap();
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "pause_public_api"
        );
        assert!(
            response.attributes[1].key == "since_block"
                && response.attributes[1].value == mock_env().block.height.to_string()
        );
        // state
        assert!(verify_not_paused_public_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_err());
        assert!(verify_not_paused_relayer_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_ok());

        response = pause_relayer_api(&mut deps, info.clone()).unwrap();
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "pause_relayer_api"
        );
        assert!(
            response.attributes[1].key == "since_block"
                && response.attributes[1].value == mock_env().block.height.to_string()
        );
        // state
        assert!(verify_not_paused_relayer_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_err());
    }

    fn assert_query_paused(
        deps: &Extern<MockStorage, MockApi, MockQuerier>,
        public_api_paused: bool,
        relayer_api_paused: bool,
    ) {
        let query_msg = QueryMsg::PausedPublicApiSince {};
        let response = query(&deps, query_msg).unwrap();
        if public_api_paused {
            assert_eq!(
                format!("{{\"block\":{}}}", mock_env().block.height.to_string()),
                str_from_binary!(response),
            );
        } else {
            assert_eq!(
                format!("{{\"block\":{}}}", u64::MAX.to_string()).as_bytes(),
                response.as_slice()
            );
        }

        let query_msg = QueryMsg::PausedRelayerApiSince {};
        let response = query(&deps, query_msg).unwrap();
        if relayer_api_paused {
            assert_eq!(
                format!("{{\"block\":{}}}", mock_env().block.height.to_string()).as_bytes(),
                response.as_slice()
            );
        } else {
            assert_eq!(
                format!("{{\"block\":{}}}", u64::MAX.to_string()).as_bytes(),
                response.as_slice()
            );
        }
    }

    #[test]
    fn success_pause_apis_by_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = DEFAULT_OWNER;
        assert_pause_both(&mut deps, caller);
        assert_query_paused(&deps, true, true);
    }

    #[test]
    fn success_pause_apis_by_monitor() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let monitor = "new_monitor";
        grant_role(&mut deps, MONITOR_ROLE, monitor, DEFAULT_OWNER).unwrap();

        assert_pause_both(&mut deps, monitor);
        assert_query_paused(&deps, true, true);
    }

    #[test]
    fn success_unpause_apis_by_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = DEFAULT_OWNER;
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        pause_public_api(&mut deps, info.clone()).unwrap();
        pause_relayer_api(&mut deps, info.clone()).unwrap();

        let mut response = unpause_public_api(&mut deps, info.clone()).unwrap();
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "pause_public_api"
        );
        assert!(
            response.attributes[1].key == "since_block"
                && response.attributes[1].value == u64::MAX.to_string()
        );
        // state
        assert!(verify_not_paused_public_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_ok());
        assert!(verify_not_paused_relayer_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_err());
        assert_query_paused(&deps, false, true);

        response = unpause_relayer_api(&mut deps, info.clone()).unwrap();
        // handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "pause_relayer_api"
        );
        assert!(
            response.attributes[1].key == "since_block"
                && response.attributes[1].value == u64::MAX.to_string()
        );
        // state
        assert!(verify_not_paused_public_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_ok());
        assert!(verify_not_paused_relayer_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_ok());
        assert_query_paused(&deps, false, false);
    }

    #[test]
    fn failure_pause_apis_neither_admin_nor_monitor() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = "user";
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));

        let mut response = pause_public_api(&mut deps, info.clone());
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);

        response = pause_relayer_api(&mut deps, info.clone());
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
    }

    #[test]
    fn failure_unpause_apis_by_monitor() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let monitor = "new_monitor";
        grant_role(&mut deps, MONITOR_ROLE, monitor, DEFAULT_OWNER).unwrap();

        let info = mock_info(monitor, &coins(0, DEFAULT_DENUM));
        pause_public_api(&mut deps, info.clone()).unwrap();
        pause_relayer_api(&mut deps, info.clone()).unwrap();

        let mut response = unpause_public_api(&mut deps, info.clone());
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        response = unpause_relayer_api(&mut deps, info.clone());
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);

        assert!(verify_not_paused_public_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_err());
        assert!(verify_not_paused_relayer_api(
            &mock_env(),
            &config_read(&deps.storage).load().unwrap()
        )
        .is_err());
    }
}

mod deposit {
    use super::*;
    use init::init_default;

    pub fn deposit(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        amount: u128,
        caller: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::Deposit {};
        let info = mock_info(caller, &coins(amount, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    #[test]
    fn success_deposit() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = 1000u128;
        let response = deposit(&mut deps, amount, DEFAULT_OWNER).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(3, response.attributes.len());
        assert!(
            response.attributes[0].key == "action" && response.attributes[0].value == "deposit"
        );
        assert!(
            response.attributes[1].key == "amount"
                && response.attributes[1].value == amount.to_string()
        );
        assert!(
            response.attributes[2].key == "sender" && response.attributes[2].value == DEFAULT_OWNER
        );

        // check contract state
        let state = config_read(&deps.storage)
            .load()
            .expect("unexpected reading state error");
        assert_eq!(cu128!(amount), state.supply);
    }

    #[test]
    fn failure_deposit_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = 1000u128;
        let account = "not_admin";
        let response = deposit(&mut deps, amount, account);
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
    }

    #[test]
    fn failure_deposit_wrong_token() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = 1000u128;
        let msg = HandleMsg::Deposit {};
        let denom = "WRONG";
        let info = mock_info(DEFAULT_OWNER, &coins(amount, denom));
        let response = handle(&mut deps, mock_env(), info, msg);
        expect_error!(response, ERR_UNRECOGNIZED_DENOM);
    }
}

mod new_relay_eon {
    use super::*;
    use access_control::grant_role;
    use init::init_default;
    use pause::pause_relayer_api;

    pub fn new_relay_eon(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::NewRelayEon {};
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    #[test]
    fn success_new_relay_eon() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        let response = new_relay_eon(&mut deps, relayer).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "new_relay_eon"
        );
        assert!(response.attributes[1].key == "eon" && response.attributes[1].value == "1");

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(1u64, state.relay_eon);

        // query
        let query_msg = QueryMsg::RelayEon {};
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(format!("{{\"eon\":{}}}", 1u64), str_from_binary!(response));
    }

    #[test]
    fn failure_new_relay_eon_not_relayer() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let caller = "not_relayer";
        let response = new_relay_eon(&mut deps, caller);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_RELAYER);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(0u64, state.relay_eon);
    }

    #[test]
    fn failure_new_relay_eon_relayer_api_paused() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        let info = mock_info(DEFAULT_OWNER, &coins(0, DEFAULT_DENUM));
        pause_relayer_api(&mut deps, info).unwrap();

        let response = new_relay_eon(&mut deps, relayer);

        expect_error!(response, ERR_CONTRACT_PAUSED);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(0u64, state.relay_eon);
    }
}

mod swap {
    use super::*;
    use init::init_default;
    use pause::pause_public_api;

    pub fn swap(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        from: &str,
        destination: &str,
        amount: u128,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::Swap {
            destination: destination.to_string(),
        };

        let info = mock_info(from, &coins(amount, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    #[test]
    fn success_swap() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let fet_account = "user_account";
        let eth_account = "some_eth_account";
        let amount = 110u128;
        let response = swap(&mut deps, fet_account, eth_account, amount).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(4, response.attributes.len());
        assert!(response.attributes[0].key == "action" && response.attributes[0].value == "swap");
        assert!(
            response.attributes[1].key == "destination"
                && response.attributes[1].value == eth_account
        );
        assert!(response.attributes[2].key == "swap_id" && response.attributes[2].value == "0");
        assert!(
            response.attributes[3].key == "amount"
                && response.attributes[3].value == amount.to_string()
        );

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(1u64, state.next_swap_id);
        assert_eq!(cu128!(amount), state.supply);

        // query
        let query_msg = QueryMsg::Supply {};
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(
            format!("{{\"amount\":\"{}\"}}", amount),
            str_from_binary!(response)
        );
    }

    #[test]
    fn failure_swap_limits() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let fet_account = "user_account";
        let eth_account = "some_eth_account";
        let mut amount: u128;
        let msg = HandleMsg::Swap {
            destination: eth_account.to_string(),
        };

        amount = DEFAULT_SWAP_LOWER_LIMIT - 10u128;
        let info = mock_info(fet_account, &coins(amount, DEFAULT_DENUM));
        let response = handle(&mut deps, mock_env(), info, msg.clone());
        expect_error!(response, ERR_SWAP_LIMITS_VIOLATED);

        amount = DEFAULT_SWAP_UPPER_LIMIT + 10u128;
        let info = mock_info(fet_account, &coins(amount, DEFAULT_DENUM));
        let response = handle(&mut deps, mock_env(), info, msg.clone());
        expect_error!(response, ERR_SWAP_LIMITS_VIOLATED);

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(0u64, state.next_swap_id);
        assert_eq!(cu128!(0u128), state.supply);
    }

    #[test]
    fn failure_swap_cap_exceeded() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let fet_account = "user_account";
        let eth_account = "some_eth_account";
        let amount = DEFAULT_SWAP_UPPER_LIMIT;
        let msg = HandleMsg::Swap {
            destination: eth_account.to_string(),
        };

        for _ in 0..(DEFAULT_CAP / DEFAULT_SWAP_UPPER_LIMIT) {
            let info = mock_info(fet_account, &coins(amount, DEFAULT_DENUM));
            handle(&mut deps, mock_env(), info, msg.clone()).unwrap();
        }

        let info = mock_info(fet_account, &coins(amount, DEFAULT_DENUM));
        let response = handle(&mut deps, mock_env(), info, msg.clone());
        expect_error!(response, ERR_CAP_EXCEEDED);

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert!(state.supply <= cu128!(DEFAULT_CAP));
    }

    #[test]
    fn failure_swap_paused_public_api() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let info = mock_info(DEFAULT_OWNER, &coins(0, DEFAULT_DENUM));
        pause_public_api(&mut deps, info).unwrap();

        let fet_account = "user_account";
        let eth_account = "some_eth_account";
        let amount = DEFAULT_SWAP_UPPER_LIMIT;
        let msg = HandleMsg::Swap {
            destination: eth_account.to_string(),
        };

        let info = mock_info(fet_account, &coins(amount, DEFAULT_DENUM));
        let response = handle(&mut deps, mock_env(), info, msg.clone());
        expect_error!(response, ERR_CONTRACT_PAUSED);

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(0u64, state.next_swap_id);
        assert_eq!(cu128!(0u128), state.supply);
    }
}

mod reverse_swap {
    use super::*;
    use access_control::grant_role;
    use deposit::deposit;
    use init::init_default;
    use new_relay_eon::new_relay_eon;
    use pause::pause_relayer_api;

    pub fn reverse_swap(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        rid: u64,
        to: &str,
        from: &str,
        hash: &str,
        amount: u128,
        eon: u64,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::ReverseSwap {
            rid: rid,
            to: addr!(to),
            sender: from.to_string(),
            origin_tx_hash: hash.to_string(),
            amount: cu128!(amount),
            relay_eon: eon,
        };

        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info.clone(), msg)
    }

    fn assert_state_unchanged(deps: &Extern<MockStorage, MockApi, MockQuerier>, deposited: u128) {
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(deposited), state.supply);
        assert_eq!(
            cu128!(DEFAULT_RA_ALLOWANCE),
            state.reverse_aggregated_allowance
        );
        assert_eq!(cu128!(0u128), state.fees_accrued);
    }

    #[test]
    fn success_reverse_swap() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();

        let fet_account = "user_account";
        let eth_account = "some_eth_account";
        let amount = 110u128;
        let rid = 0u64;
        let eon = 0u64;
        let origin_tx_hash: &str = "HHHHHHAAAAAASSSHHH";
        let response = reverse_swap(
            &mut deps,
            relayer,
            rid,
            fet_account,
            eth_account,
            origin_tx_hash,
            amount,
            eon,
        )
        .unwrap();

        // check handle response
        assert_eq!(1, response.messages.len());
        assert_eq!(7, response.attributes.len());
        match &response.messages[0] {
            CosmosMsg::Bank(bank) => match bank {
                BankMsg::Send {
                    from_address,
                    to_address,
                    amount: funds,
                } => {
                    assert_eq!(&mock_env().contract.address, from_address);
                    assert_eq!(&addr!(fet_account), to_address);
                    assert_eq!(
                        cu128!(amount - DEFAULT_SWAP_FEE),
                        amount_from_funds(funds, DEFAULT_DENUM.to_string()).unwrap()
                    );
                }
            },
            _ => panic!("unexpected message in handle response"),
        }
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "reverse_swap"
        );
        assert!(
            response.attributes[1].key == "rid" && response.attributes[1].value == rid.to_string()
        );
        assert!(response.attributes[2].key == "to" && response.attributes[2].value == fet_account);
        assert!(
            response.attributes[3].key == "sender" && response.attributes[3].value == eth_account
        );
        assert!(
            response.attributes[4].key == "origin_tx_hash"
                && response.attributes[4].value == origin_tx_hash
        );
        assert!(
            response.attributes[5].key == "amount"
                && response.attributes[5].value == (amount - DEFAULT_SWAP_FEE).to_string()
        );
        assert!(
            response.attributes[6].key == "swap_fee"
                && response.attributes[6].value == (DEFAULT_SWAP_FEE).to_string()
        );

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(deposited - amount), state.supply);
        assert_eq!(
            cu128!(DEFAULT_RA_ALLOWANCE - amount),
            state.reverse_aggregated_allowance
        );
        assert_eq!(cu128!(DEFAULT_SWAP_FEE), state.fees_accrued);
    }

    #[test]
    fn failure_reverse_swap_not_relayer() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let account = "not_relayer";
        let deposited = 1000u128;
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();

        let response = reverse_swap(
            &mut deps,
            account,
            0u64,
            "user_account",
            "eth_account",
            "HHHHHHHHHAAAAASSSSSSSH",
            110u128,
            0u64,
        );

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_RELAYER);
        assert_state_unchanged(&deps, deposited);
    }

    #[test]
    fn failure_reverse_swap_wrong_eon() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "relayer";
        let deposited = 1000u128;
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        new_relay_eon(&mut deps, relayer).unwrap();

        let response = reverse_swap(
            &mut deps,
            relayer,
            0u64,
            "user_account",
            "eth_account",
            "HHHHHHHHHAAAAASSSSSSSH",
            110u128,
            0u64,
        );

        expect_error!(response, ERR_EON);
        assert_state_unchanged(&deps, deposited);
    }

    #[test]
    fn failure_reverse_swap_paused_relayer_api() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "relayer";
        let deposited = 1000u128;
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        let info = mock_info(DEFAULT_OWNER, &coins(0, DEFAULT_DENUM));
        pause_relayer_api(&mut deps, info).unwrap();

        let response = reverse_swap(
            &mut deps,
            relayer,
            0u64,
            "user_account",
            "eth_account",
            "HHHHHHHHHAAAAASSSSSSSH",
            110u128,
            0u64,
        );

        expect_error!(response, ERR_CONTRACT_PAUSED);
        assert_state_unchanged(&deps, deposited);
    }

    #[test]
    fn failure_reverse_swap_supply_exceeded() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "relayer";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();

        let response = reverse_swap(
            &mut deps,
            relayer,
            0u64,
            "user_account",
            "eth_account",
            "HHHHHHHHHAAAAASSSSSSSH",
            110u128,
            0u64,
        );

        expect_error!(response, ERR_SUPPLY_EXCEEDED);
        assert_state_unchanged(&deps, 0u128);
    }

    #[test]
    fn failure_reverse_swap_aggregated_reverse_allowance_exceeded() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "relayer";
        let deposited = 100000u128;
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();

        for _ in 0..(DEFAULT_RA_ALLOWANCE / DEFAULT_SWAP_UPPER_LIMIT) {
            reverse_swap(
                &mut deps,
                relayer,
                0u64,
                "user_account",
                "eth_account",
                "HHHHHHHHHAAAAASSSSSSSH",
                DEFAULT_SWAP_UPPER_LIMIT,
                0u64,
            )
            .unwrap();
        }

        let response = reverse_swap(
            &mut deps,
            relayer,
            0u64,
            "user_account",
            "eth_account",
            "HHHHHHHHHAAAAASSSSSSSH",
            DEFAULT_SWAP_UPPER_LIMIT,
            0u64,
        );
        expect_error!(response, ERR_RA_ALLOWANCE_EXCEEDED);
    }
}

mod refund {
    use super::*;
    use access_control::grant_role;
    use deposit::deposit;
    use init::init_default;
    use new_relay_eon::new_relay_eon;
    use pause::pause_relayer_api;
    use swap::swap;

    pub fn refund(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        id: u64,
        to: &str,
        amount: u128,
        eon: u64,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::Refund {
            id: id,
            to: addr!(to),
            amount: cu128!(amount),
            relay_eon: eon,
        };

        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    pub fn refund_in_full(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        id: u64,
        to: &str,
        amount: u128,
        eon: u64,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::RefundInFull {
            id: id,
            to: addr!(to),
            amount: cu128!(amount),
            relay_eon: eon,
        };

        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn _success_refund(fee: u128) {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        swap(
            &mut deps,
            fet_account,
            "some_eth_account",
            DEFAULT_SWAP_LOWER_LIMIT,
        )
        .unwrap();

        let amount = DEFAULT_SWAP_LOWER_LIMIT + 10u128;
        let id = 0u64;
        let eon = 0u64;
        let response: HandleResponse;
        if fee > 0 {
            response = refund(&mut deps, relayer, id, fet_account, amount, eon).unwrap();
        } else {
            response = refund_in_full(&mut deps, relayer, id, fet_account, amount, eon).unwrap();
        }

        // check handle response
        assert_eq!(1, response.messages.len());
        assert_eq!(5, response.attributes.len());
        match &response.messages[0] {
            CosmosMsg::Bank(bank) => match bank {
                BankMsg::Send {
                    from_address,
                    to_address,
                    amount: funds,
                } => {
                    assert_eq!(&mock_env().contract.address, from_address);
                    assert_eq!(&addr!(fet_account), to_address);
                    assert_eq!(
                        cu128!(amount - fee),
                        amount_from_funds(funds, DEFAULT_DENUM.to_string()).unwrap()
                    );
                }
            },
            _ => panic!("unexpected message in handle response"),
        }
        assert!(response.attributes[0].key == "action" && response.attributes[0].value == "refund");
        assert!(
            response.attributes[1].key == "destination"
                && response.attributes[1].value == fet_account
        );
        assert!(
            response.attributes[2].key == "swap_id"
                && response.attributes[2].value == id.to_string()
        );
        assert!(
            response.attributes[3].key == "amount"
                && response.attributes[3].value == (amount - fee).to_string()
        );
        assert!(
            response.attributes[4].key == "refund_fee"
                && response.attributes[4].value == fee.to_string()
        );

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            cu128!(deposited - amount + DEFAULT_SWAP_LOWER_LIMIT),
            state.supply
        );
        assert_eq!(
            cu128!(DEFAULT_RA_ALLOWANCE - amount),
            state.reverse_aggregated_allowance
        );
        assert_eq!(cu128!(fee), state.fees_accrued);
    }

    #[test]
    fn success_refund() {
        _success_refund(DEFAULT_SWAP_FEE)
    }

    #[test]
    fn success_refund_in_full() {
        _success_refund(0u128)
    }

    #[test]
    fn failure_refund_not_relayer() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        swap(
            &mut deps,
            fet_account,
            "some_eth_account",
            DEFAULT_SWAP_LOWER_LIMIT,
        )
        .unwrap();

        let not_relayer = "not_relayer";
        let amount = DEFAULT_SWAP_LOWER_LIMIT + 10u128;
        let id = 0u64;
        let eon = 0u64;
        let response = refund(&mut deps, not_relayer, id, fet_account, amount, eon);
        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_RELAYER);
    }

    #[test]
    fn failure_refund_wrong_eon() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        swap(
            &mut deps,
            fet_account,
            "some_eth_account",
            DEFAULT_SWAP_LOWER_LIMIT,
        )
        .unwrap();
        new_relay_eon(&mut deps, relayer).unwrap();

        let amount = DEFAULT_SWAP_LOWER_LIMIT + 10u128;
        let id = 0u64;
        let eon = 0u64;
        let response = refund(&mut deps, relayer, id, fet_account, amount, eon);
        expect_error!(response, ERR_EON);
    }

    #[test]
    fn failure_refund_paused_relayer_api() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        swap(
            &mut deps,
            fet_account,
            "some_eth_account",
            DEFAULT_SWAP_LOWER_LIMIT,
        )
        .unwrap();
        let info = mock_info(DEFAULT_OWNER, &coins(0, DEFAULT_DENUM));
        pause_relayer_api(&mut deps, info).unwrap();

        let amount = DEFAULT_SWAP_LOWER_LIMIT + 10u128;
        let id = 0u64;
        let eon = 0u64;
        let response = refund(&mut deps, relayer, id, fet_account, amount, eon);
        expect_error!(response, ERR_CONTRACT_PAUSED);
    }

    #[test]
    fn failure_refund_wrong_swap_id() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();

        let amount = DEFAULT_SWAP_LOWER_LIMIT + 10u128;
        let id = 0u64;
        let eon = 0u64;
        let response = refund(&mut deps, relayer, id, fet_account, amount, eon);
        expect_error!(response, ERR_INVALID_SWAP_ID);
    }

    #[test]
    fn failure_refund_already_processed() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 1000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();
        swap(
            &mut deps,
            fet_account,
            "some_eth_account",
            DEFAULT_SWAP_LOWER_LIMIT,
        )
        .unwrap();

        let amount = DEFAULT_SWAP_LOWER_LIMIT + 10u128;
        let id = 0u64;
        let eon = 0u64;
        refund(&mut deps, relayer, id, fet_account, amount, eon).unwrap();
        let response = refund(&mut deps, relayer, id, fet_account, amount, eon);
        expect_error!(response, ERR_ALREADY_REFUNDED);
    }

    #[test]
    fn failure_refund_aggregated_reverse_allowance_exceeded() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let relayer = "new_relayer";
        let deposited = 10000u128;
        let fet_account = "user_account";
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        deposit(&mut deps, deposited, DEFAULT_OWNER).unwrap();

        let mut id = 0u64;
        let eon = 0u64;
        for _ in 0..(DEFAULT_RA_ALLOWANCE / DEFAULT_SWAP_UPPER_LIMIT) {
            swap(
                &mut deps,
                fet_account,
                "some_eth_account",
                DEFAULT_SWAP_LOWER_LIMIT,
            )
            .unwrap();
            refund(
                &mut deps,
                relayer,
                id,
                fet_account,
                DEFAULT_SWAP_UPPER_LIMIT,
                eon,
            )
            .unwrap();
            id += 1;
        }

        swap(
            &mut deps,
            fet_account,
            "some_eth_account",
            DEFAULT_SWAP_LOWER_LIMIT,
        )
        .unwrap();
        let response = refund(
            &mut deps,
            relayer,
            id,
            fet_account,
            DEFAULT_SWAP_UPPER_LIMIT,
            eon,
        );
        expect_error!(response, ERR_RA_ALLOWANCE_EXCEEDED);
    }
}

mod withdraw {
    use super::*;
    use deposit::deposit;
    use init::init_default;
    use swap::swap;

    fn withdraw(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        amount: u128,
        recipient: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::Withdraw {
            amount: cu128!(amount),
            destination: addr!(recipient),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    #[test]
    fn success_withdraw() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = 1000u128;
        deposit(&mut deps, amount, DEFAULT_OWNER).unwrap();
        swap(&mut deps, "user", "some_eth_addr", amount).unwrap();

        let recipient = "lucky_user";
        let response = withdraw(&mut deps, DEFAULT_OWNER, 2 * amount, recipient).unwrap();

        // check handle response
        assert_eq!(1, response.messages.len());
        assert_eq!(3, response.attributes.len());
        match &response.messages[0] {
            CosmosMsg::Bank(bank) => match bank {
                BankMsg::Send {
                    from_address,
                    to_address,
                    amount: funds,
                } => {
                    assert_eq!(&mock_env().contract.address, from_address);
                    assert_eq!(&addr!(recipient), to_address);
                    assert_eq!(
                        cu128!(2 * amount),
                        amount_from_funds(funds, DEFAULT_DENUM.to_string()).unwrap()
                    );
                }
            },
            _ => panic!("unexpected message in handle response"),
        }
        assert!(
            response.attributes[0].key == "action" && response.attributes[0].value == "withdraw"
        );
        assert!(
            response.attributes[1].key == "amount"
                && response.attributes[1].value == (2 * amount).to_string()
        );
        assert!(
            response.attributes[2].key == "destination"
                && response.attributes[2].value == recipient.to_string()
        );

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(0u128), state.supply);
    }

    #[test]
    fn failure_withdraw_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = 1000u128;
        deposit(&mut deps, amount, DEFAULT_OWNER).unwrap();

        let recipient = "lucky_user";
        let response = withdraw(&mut deps, recipient, amount, recipient);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(amount), state.supply);
    }

    #[test]
    fn failure_withdraw_not_enough_supply() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = 1000u128;

        let recipient = "lucky_user";
        let response = withdraw(&mut deps, DEFAULT_OWNER, amount, recipient);
        expect_error!(response, ERR_SUPPLY_EXCEEDED);
    }
}

mod withdraw_fees {
    use super::*;
    use access_control::grant_role;
    use deposit::deposit;
    use init::init_default;
    use reverse_swap::reverse_swap;

    fn withdraw_fees(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        amount: u128,
        recipient: &str,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::WithdrawFees {
            amount: cu128!(amount),
            destination: addr!(recipient),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn _do_one_reverse_swap(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
    ) -> StdResult<HandleResponse> {
        let relayer = "relayer";
        deposit(&mut deps, DEFAULT_SWAP_LOWER_LIMIT, DEFAULT_OWNER).unwrap();
        grant_role(&mut deps, RELAYER_ROLE, relayer, DEFAULT_OWNER).unwrap();
        reverse_swap(
            &mut deps,
            relayer,
            0u64,
            DEFAULT_OWNER,
            "some_eth_addr",
            "HHHHHAAAASSSSH",
            DEFAULT_SWAP_LOWER_LIMIT,
            0u64,
        )
    }
    #[test]
    fn success_withdraw_fees() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        _do_one_reverse_swap(&mut deps).unwrap();

        let recipient = "lucky_user";
        let response =
            withdraw_fees(&mut deps, DEFAULT_OWNER, DEFAULT_SWAP_FEE, recipient).unwrap();

        // check handle response
        assert_eq!(1, response.messages.len());
        assert_eq!(3, response.attributes.len());
        match &response.messages[0] {
            CosmosMsg::Bank(bank) => match bank {
                BankMsg::Send {
                    from_address,
                    to_address,
                    amount: funds,
                } => {
                    assert_eq!(&mock_env().contract.address, from_address);
                    assert_eq!(&addr!(recipient), to_address);
                    assert_eq!(
                        cu128!(DEFAULT_SWAP_FEE),
                        amount_from_funds(funds, DEFAULT_DENUM.to_string()).unwrap()
                    );
                }
            },
            _ => panic!("unexpected message in handle response"),
        }
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "withdraw_fees"
        );
        assert!(
            response.attributes[1].key == "amount"
                && response.attributes[1].value == (DEFAULT_SWAP_FEE).to_string()
        );
        assert!(
            response.attributes[2].key == "destination"
                && response.attributes[2].value == recipient.to_string()
        );

        // check contract state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(0u128), state.fees_accrued);
    }

    #[test]
    fn failure_withdraw_fees_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        _do_one_reverse_swap(&mut deps).unwrap();

        let recipient = "lucky_user";
        let response = withdraw_fees(&mut deps, recipient, DEFAULT_SWAP_FEE, recipient);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(DEFAULT_SWAP_FEE), state.fees_accrued);
    }

    #[test]
    fn failure_withdraw_fees_not_enough_supply() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        _do_one_reverse_swap(&mut deps).unwrap();

        let recipient = "lucky_user";
        let response = withdraw_fees(
            &mut deps,
            DEFAULT_OWNER,
            DEFAULT_SWAP_FEE + 1u128,
            recipient,
        );
        expect_error!(response, ERR_SUPPLY_EXCEEDED);
    }
}

mod set_cap {
    use super::*;
    use init::init_default;

    fn set_cap(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        amount: u128,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::SetCap {
            amount: cu128!(amount),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    #[test]
    fn success_set_cap() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_CAP + 1000u128;
        let response = set_cap(&mut deps, DEFAULT_OWNER, amount).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action" && response.attributes[0].value == "set_cap"
        );
        assert!(
            response.attributes[1].key == "cap"
                && response.attributes[1].value == (amount).to_string()
        );

        // check state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(amount), state.cap);

        // query
        let query_msg = QueryMsg::Cap {};
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(
            format!("{{\"amount\":\"{}\"}}", amount),
            str_from_binary!(response)
        );
    }

    #[test]
    fn failure_set_cap_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_CAP + 1000u128;
        let response = set_cap(&mut deps, "not_admin", amount);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(DEFAULT_CAP), state.cap);
    }
}

mod set_limits {
    use super::*;
    use init::init_default;

    fn set_limits(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        fee: u128,
        min: u128,
        max: u128,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::SetLimits {
            swap_min: cu128!(min),
            swap_max: cu128!(max),
            swap_fee: cu128!(fee),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    #[test]
    fn success_set_limits() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let fee = 10u128;
        let min = 20u128;
        let max = 30u128;
        let response = set_limits(&mut deps, DEFAULT_OWNER, fee, min, max).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(4, response.attributes.len());
        assert!(
            response.attributes[0].key == "action" && response.attributes[0].value == "set_limits"
        );
        assert!(
            response.attributes[1].key == "swap_fee"
                && response.attributes[1].value == (fee).to_string()
        );
        assert!(
            response.attributes[2].key == "swap_min"
                && response.attributes[2].value == (min).to_string()
        );
        assert!(
            response.attributes[3].key == "swap_max"
                && response.attributes[3].value == (max).to_string()
        );

        // check state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(fee), state.swap_fee);
        assert_eq!(cu128!(min), state.lower_swap_limit);
        assert_eq!(cu128!(max), state.upper_swap_limit);

        // query
        let query_msg = QueryMsg::SwapMax {};
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(
            format!("{{\"amount\":\"{}\"}}", max),
            str_from_binary!(response)
        );
    }

    #[test]
    fn failure_set_limits_not_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let fee = 10u128;
        let min = 20u128;
        let max = 30u128;
        let response = set_limits(&mut deps, "not_admin", fee, min, max);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(DEFAULT_SWAP_FEE), state.swap_fee);
        assert_eq!(cu128!(DEFAULT_SWAP_LOWER_LIMIT), state.lower_swap_limit);
        assert_eq!(cu128!(DEFAULT_SWAP_UPPER_LIMIT), state.upper_swap_limit);
    }

    #[test]
    fn failure_set_limits_unconsistent_limits() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let fee = 20u128;
        let min = 20u128;
        let max = 10u128;
        let response = set_limits(&mut deps, DEFAULT_OWNER, fee, min, max);

        expect_error!(response, ERR_SWAP_LIMITS_INCONSISTENT);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(DEFAULT_SWAP_FEE), state.swap_fee);
        assert_eq!(cu128!(DEFAULT_SWAP_LOWER_LIMIT), state.lower_swap_limit);
        assert_eq!(cu128!(DEFAULT_SWAP_UPPER_LIMIT), state.upper_swap_limit);
    }
}

mod set_reverse_aggregated_allowance {
    use super::*;
    use access_control::grant_role;
    use init::init_default;

    fn set_reverse_aggregated_allowance(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        amount: u128,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::SetReverseAggregatedAllowance {
            amount: cu128!(amount),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn set_reverse_aggregated_allowance_approver_cap(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        caller: &str,
        amount: u128,
    ) -> StdResult<HandleResponse> {
        let msg = HandleMsg::SetReverseAggregatedAllowanceApproverCap {
            amount: cu128!(amount),
        };
        let info = mock_info(caller, &coins(0, DEFAULT_DENUM));
        handle(&mut deps, mock_env(), info, msg)
    }

    fn set_reverse_aggregated_allowance_by_approver(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        amount: u128,
    ) -> StdResult<HandleResponse> {
        let approver = "new_approver";
        grant_role(&mut deps, APPROVER_ROLE, approver, DEFAULT_OWNER).unwrap();
        set_reverse_aggregated_allowance(&mut deps, approver, amount)
    }

    #[test]
    fn success_set_reverse_aggregated_allowance_approver_cap() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_RA_ALLOWANCE_APPROVER_CAP + 1000u128;
        let response =
            set_reverse_aggregated_allowance_approver_cap(&mut deps, DEFAULT_OWNER, amount)
                .unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "set_reverse_aggregated_allowance_approver_cap"
        );
        assert!(
            response.attributes[1].key == "amount"
                && response.attributes[1].value == (amount).to_string()
        );

        // check state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            cu128!(amount),
            state.reverse_aggregated_allowance_approver_cap
        );
    }

    #[test]
    fn success_set_reverse_aggregated_allowance_by_admin() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_RA_ALLOWANCE + 1000u128;
        let response = set_reverse_aggregated_allowance(&mut deps, DEFAULT_OWNER, amount).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "set_reverse_aggregated_allowance"
        );
        assert!(
            response.attributes[1].key == "amount"
                && response.attributes[1].value == (amount).to_string()
        );

        // check state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(amount), state.reverse_aggregated_allowance);

        // query
        let query_msg = QueryMsg::ReverseAggregatedAllowance {};
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(
            format!("{{\"amount\":\"{}\"}}", amount),
            str_from_binary!(response)
        );
    }

    #[test]
    fn success_set_reverse_aggregated_allowance_by_approver() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_RA_ALLOWANCE_APPROVER_CAP;
        let response = set_reverse_aggregated_allowance_by_approver(&mut deps, amount).unwrap();

        // check handle response
        assert_eq!(0, response.messages.len());
        assert_eq!(2, response.attributes.len());
        assert!(
            response.attributes[0].key == "action"
                && response.attributes[0].value == "set_reverse_aggregated_allowance"
        );
        assert!(
            response.attributes[1].key == "amount"
                && response.attributes[1].value == (amount).to_string()
        );

        // check state
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(cu128!(amount), state.reverse_aggregated_allowance);

        // query
        let query_msg = QueryMsg::ReverseAggregatedAllowance {};
        let response = query(&deps, query_msg).unwrap();
        assert_eq!(
            format!("{{\"amount\":\"{}\"}}", amount),
            str_from_binary!(response)
        );
    }

    #[test]
    fn failure_set_reverse_aggregated_allowance_by_approver_exceeds_cap() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_RA_ALLOWANCE_APPROVER_CAP + 1u128;
        let response = set_reverse_aggregated_allowance_by_approver(&mut deps, amount);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            cu128!(DEFAULT_RA_ALLOWANCE),
            state.reverse_aggregated_allowance
        );
    }

    #[test]
    fn failure_set_reverse_aggregated_allowance_not_admin_nor_approver() {
        let mut deps = mock_dependencies(&[]);
        init_default(&mut deps).unwrap();

        let amount = DEFAULT_RA_ALLOWANCE_APPROVER_CAP;
        let response = set_reverse_aggregated_allowance(&mut deps, "user", amount);

        expect_error!(response, ERR_ACCESS_CONTROL_ONLY_ADMIN);
        let state = config_read(&deps.storage).load().unwrap();
        assert_eq!(
            cu128!(DEFAULT_RA_ALLOWANCE),
            state.reverse_aggregated_allowance
        );
    }
}
