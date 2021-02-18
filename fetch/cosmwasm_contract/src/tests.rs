use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{coins, from_binary, Coin, Extern, HumanAddr, InitResponse, StdError};

use crate::contract::{handle, init, query};
use crate::msg::{HandleMsg, InitMsg, QueryMsg, Uint128};
//use crate::state::Config;

#[cfg(test)]
mod tests {

    fn mock_init(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        cap: Option<Uint128>,
        deposit: Option<Uint128>,
        upper_swap_limit: Option<Uint128>,
        lower_swap_limit: Option<Uint128>,
        swap_fee: Option<Uint128>,
    ) {
        let msg = InitMsg {
            cap: cap.unwrap_or(Uint128::from(100000u128)),
            deposit: deposit.unwrap_or(Uint128::from(10000u128)),
            upper_swap_limit: upper_swap_limit.unwrap_or(Uint128::from(1000u128)),
            lower_swap_limit: lower_swap_limit.unwrap_or(Uint128::from(10u128)),
            swap_fee: swap_fee.unwrap_or(Uint128::from(9u128)),
            paused_since_block: None,
            delete_protection_period: None,
        };

        let env = mock_env(&deps.api, "creator", &coins(1000, "fet"));
        let res = init(&mut deps, env, msg);
        res.expect("contract successfully handles InitMsg");
    }

    #[test]
    fn proper_init() {
        let mut deps = mock_dependencies(20, &[]);

        mock_init(&mut deps, None, None, None, None, None);

        assert_eq!(1, 1);
    }
}
