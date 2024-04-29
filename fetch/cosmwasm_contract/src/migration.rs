use crate::access_control::wipe_all_roles_from_storage;
use crate::contract::initialise_contract_state;
use crate::error::ERR_STATE_ERROR;
use crate::legacy_state::legacy_config_read;
use crate::msg::MigrateMsg;
use crate::state::is_state_valid;
use cosmwasm_std::{entry_point, DepsMut, Env, Response, StdError, StdResult};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> StdResult<Response> {
    // Remove all previously registered roles
    wipe_all_roles_from_storage(deps.storage);

    let legacy_state = legacy_config_read(deps.storage).load()?;

    // Ensure correct contract state
    if let Some(re_init) = msg.re_init {
        initialise_contract_state(
            deps.storage,
            &env,
            &re_init.admin,
            legacy_state.supply,
            legacy_state.relay_eon,
            legacy_state.fees_accrued,
            &re_init.init_msg,
        )?;
    } else if !is_state_valid(deps.storage) {
        return Err(StdError::generic_err(ERR_STATE_ERROR));
    }

    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_state::{legacy_config, LegacyState};
    use crate::msg::{InstantiateMsg, ReInitMsg};
    use crate::state::config_read;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{Addr, Uint128};
    use cosmwasm_vm::testing::mock_env;

    #[test]
    fn test_can_migrate() {
        let mut deps = mock_dependencies();

        // Prepare legacy storage
        let legacy_state = LegacyState {
            supply: Uint128::new(1),
            fees_accrued: Uint128::new(2),
            next_swap_id: 3,
            sealed_reverse_swap_id: 4,
            relay_eon: 5,
            upper_swap_limit: Uint128::new(7),
            lower_swap_limit: Uint128::new(6),
            reverse_aggregated_allowance: Uint128::new(8),
            reverse_aggregated_allowance_approver_cap: Uint128::new(9),
            cap: Uint128::new(10),
            swap_fee: Uint128::new(2),
            paused_since_block_public_api: 12,
            paused_since_block_relayer_api: 13,
            denom: "ABC".to_string(),
            contract_addr_human: Addr::unchecked("DEF"),
        };
        assert!(legacy_config(deps.as_mut().storage)
            .save(&legacy_state)
            .is_ok());

        let admin = Addr::unchecked("admin");
        let init_msg = InstantiateMsg {
            next_swap_id: 10,
            cap: Uint128::new(20),
            upper_swap_limit: Uint128::new(40),
            lower_swap_limit: Uint128::new(30),
            swap_fee: Uint128::new(10),
            reverse_aggregated_allowance: Uint128::new(60),
            reverse_aggregated_allowance_approver_cap: Uint128::new(70),
            paused_since_block: Some(80),
            denom: Some("ASI".to_string()),
        };

        let re_init_msg = ReInitMsg {
            init_msg: init_msg.clone(),
            admin: admin.clone(),
        };

        let migrate_msg = MigrateMsg {
            re_init: Some(re_init_msg),
        };

        let mut env = mock_env();
        env.block.height = 1;
        assert!(migrate(deps.as_mut(), env, migrate_msg).is_ok());

        let new_state = config_read(deps.as_ref().storage).load().unwrap();

        // Transferred legacy values
        assert_eq!(new_state.supply, legacy_state.supply);
        assert_eq!(new_state.relay_eon, legacy_state.relay_eon);
        assert_eq!(new_state.fees_accrued, legacy_state.fees_accrued);

        // New values
        assert_eq!(new_state.cap, init_msg.cap);
        assert_eq!(new_state.denom, init_msg.denom.unwrap());
        assert_eq!(new_state.next_swap_id, init_msg.next_swap_id);
        assert_eq!(new_state.lower_swap_limit, init_msg.lower_swap_limit);
        assert_eq!(
            new_state.paused_since_block_public_api,
            init_msg.paused_since_block.unwrap()
        );
        assert_eq!(
            new_state.paused_since_block_relayer_api,
            init_msg.paused_since_block.unwrap()
        );
        assert_eq!(
            new_state.reverse_aggregated_allowance,
            init_msg.reverse_aggregated_allowance
        );
        assert_eq!(
            new_state.reverse_aggregated_allowance_approver_cap,
            init_msg.reverse_aggregated_allowance_approver_cap
        );
        assert_eq!(new_state.sealed_reverse_swap_id, 0);
        assert_eq!(new_state.swap_fee, init_msg.swap_fee);
        assert_eq!(new_state.upper_swap_limit, init_msg.upper_swap_limit);
    }
}
