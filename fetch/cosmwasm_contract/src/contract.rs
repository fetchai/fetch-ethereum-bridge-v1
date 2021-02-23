use cosmwasm_std::{
    log,
    to_binary,
    Api,
    BankMsg,
    Binary,
    CanonicalAddr,
    Coin,
    CosmosMsg,
    Env,
    Extern,
    HandleResponse,
    HandleResult,
    HumanAddr,
    InitResponse,
    Querier,
    StdError,
    StdResult,
    Storage, //ReadonlyStorage,
};

use crate::access_control::{
    ac_add_role, ac_get_owner, ac_have_role, ac_revoke_role, ac_set_owner, AccessRole,
};
use crate::msg::{HandleMsg, InitMsg, QueryMsg, RoleResponse, Uint128};
use crate::state::{config, config_read, State};

/* ***************************************************
 * **************    Initialization      *************
 * ***************************************************/

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let current_block_number = env.block.height;

    let mut paused_since_block = msg.paused_since_block.unwrap_or(u64::MAX);
    if paused_since_block < current_block_number {
        paused_since_block = current_block_number
    }

    let delete_protection_period = msg.delete_protection_period.unwrap_or(0u64);
    let earliest_delete = current_block_number + delete_protection_period;
    let contract_addr_human = env.contract.address;

    if msg.lower_swap_limit > msg.upper_swap_limit || msg.lower_swap_limit <= msg.swap_fee {
        return Err(StdError::generic_err(
            "inconsistent swap fee and swap limits",
        ));
    }

    ac_set_owner(&mut deps.storage, &env.message.sender)?;
    ac_add_role(&mut deps.storage, &env.message.sender, &AccessRole::Admin)?;

    let state = State {
        supply: msg.deposit, // Uint128::zero(), // TMP(LR)
        refunds_fees_accrued: Uint128::zero(),
        next_swap_id: 0,
        sealed_reverse_swap_id: 0,
        relay_eon: 0,
        upper_swap_limit: msg.upper_swap_limit,
        lower_swap_limit: msg.lower_swap_limit,
        cap: msg.cap,
        swap_fee: msg.swap_fee,
        paused_since_block,
        earliest_delete,
        denom: env.message.sent_funds[0].denom.clone(), // TMP(LR)
        contract_addr_human, // optimization FIXME(LR) not needed any more (version 0.10.0)
    };

    config(&mut deps.storage).save(&state)?;

    Ok(InitResponse::default())
}

/* ***************************************************
 * ******************    Actions    ******************
 * ***************************************************/

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let state = config_read(&deps.storage).load()?;

    match msg {
        HandleMsg::Swap { destination } => {
            let amount = amount_from_funds(&env.message.sent_funds);
            try_swap(deps, &env, &state, amount, destination)
        }
        HandleMsg::ReverseSwap {
            rid,
            to,
            sender,
            origin_tx_hash,
            amount,
            relay_eon,
        } => try_reverse_swap(
            deps,
            &env,
            &state,
            rid,
            to,
            sender,
            origin_tx_hash,
            amount,
            relay_eon,
        ),
        HandleMsg::Refund {
            id,
            to,
            amount,
            relay_eon,
        } => try_refund(deps, &env, &state, id, to, amount, relay_eon),
        HandleMsg::Pause { since_block } => try_pause(deps, &env, &state, since_block),
        HandleMsg::NewRelayEon {} => try_new_relay_eon(deps, &env, &state),
        HandleMsg::FreezeFunds { amount } => try_freeze_funds(deps, &env, &state, amount),
        HandleMsg::UnFreezeFunds { amount } => try_unfreeze_funds(deps, &env, &state, amount),
        HandleMsg::SetCap { amount } => try_set_cap(deps, &env, &state, amount),
        HandleMsg::SetLimits {
            swap_min,
            swap_max,
            swap_fee,
        } => try_set_limits(deps, &env, &state, swap_min, swap_max, swap_fee),
        HandleMsg::GrantRole { role, address } => try_grant_role(deps, &env, role, address),
        HandleMsg::RevokeRole { role, address } => try_revoke_role(deps, &env, role, address),
        HandleMsg::RenounceRole { role } => try_renouce_role(deps, &env, role),
    }
}

fn try_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    amount: Uint128,
    destination: String,
) -> StdResult<HandleResponse> {
    verify_not_paused(env, state)?;
    verify_swap_amount_limits(amount, state)?;

    let increased_supply = state.supply + amount;
    if increased_supply > state.cap {
        return Err(StdError::generic_err("Swap would exceed cap"));
    }

    let swap_id = state.next_swap_id;
    config(&mut deps.storage).update(|mut state| {
        state.supply = increased_supply;
        state.next_swap_id += 1;
        Ok(state)
    })?;

    let log = vec![
        log("action", "swap"),
        log("destination", destination),
        log("swap_id", swap_id),
        log("amount", amount),
        // NOTE(LR) fees will be deducted in destination chain
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_reverse_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    rid: u64,
    to: HumanAddr,
    sender: String,
    origin_tx_hash: String,
    amount: Uint128,
    relay_eon: u64,
) -> StdResult<HandleResponse> {
    verify_tx_relay_eon(relay_eon, state)?;
    only_relayer(env, &deps.storage)?;

    if amount > state.swap_fee {
        // NOTE(LR) when amount == fee, amount will still be consumed
        let swap_fee = state.swap_fee;
        let effective_amount = (amount - swap_fee)?;
        let to_canonical = deps.api.canonical_address(&to)?;
        let rtx =
            send_tokens_from_contract(&deps.api, &state, &to_canonical, amount, "reverse_swap")?;
        config(&mut deps.storage).update(|mut state| {
            state.supply = (state.supply - amount)?;
            //state.sealed_reverse_swap_id = rid; // TODO(LR)
            Ok(state)
        })?;

        let log = vec![
            log("action", "reverse_swap"),
            log("rid", rid),
            log("to", to),
            log("sender", sender),
            log("origin_tx_hash", origin_tx_hash),
            log("amount", effective_amount),
            log("swap_fee", swap_fee),
        ];

        let r = HandleResponse {
            messages: rtx.messages,
            log,
            data: None,
        };
        Ok(r)
    } else {
        let swap_fee = amount;
        let effective_amount = Uint128::zero();

        let log = vec![
            log("action", "reverse_swap"),
            log("rid", rid),
            log("to", to),
            log("from", sender),
            log("origin_tx_hash", origin_tx_hash),
            log("amount", effective_amount),
            log("swap_fee", swap_fee),
        ];

        let r = HandleResponse {
            messages: vec![],
            log,
            data: None,
        };
        Ok(r)
    }
}

// Refund operation
// is excuted when a swap can be finalized on the other chain
// the case when that happen include:
// - an error in the destination address: malformed wallet, invalid destination address
// - failure to finalized the swap command on the other chain:
//    + error in the contract
//    + on the dest chain: highly imporbable for ether and mostly probable for cosmos native
// Refund will rebalance the `supply`
fn try_refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    _state: &State,
    id: u64,
    to: HumanAddr,
    amount: Uint128,
    _relay_eon: u64,
) -> StdResult<HandleResponse> {
    only_relayer(env, &deps.storage)?;

    let log = vec![
        log("action", "refund"),
        log("destination", to),
        log("swap_id", id),
        log("amount", amount),
        // NOTE(LR) fees will be deducted in destination chain
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_pause<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    _state: &State,
    _since_block: u64,
) -> StdResult<HandleResponse> {
    only_relayer(env, &deps.storage)?;
    Ok(HandleResponse::default())
}

fn try_new_relay_eon<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
) -> StdResult<HandleResponse> {
    only_relayer(env, &deps.storage)?;
    let new_eon = state.relay_eon + 1;
    config(&mut deps.storage).update(|mut state| {
        state.relay_eon = new_eon; // FIXME(LR) starts from 1
        Ok(state)
    })?;

    let log = vec![
        log("action", "new_relay_eon"),
        log("new_relay_eon", new_eon),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None, // TODO(LR) what can I send in data?
    };
    Ok(r)
}

fn try_freeze_funds<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: &Env,
    _state: &State,
    _amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

fn try_unfreeze_funds<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: &Env,
    _state: &State,
    _amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

fn try_set_cap<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: &Env,
    _state: &State,
    _amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

fn try_set_limits<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: &Env,
    _state: &State,
    _swap_min: Uint128,
    _swap_max: Uint128,
    _swap_fee: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

fn try_grant_role<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    role: String,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    only_admin(&env, &deps.storage)?;

    ac_add_role(
        &mut deps.storage,
        &address,
        &AccessRole::from_str(role.as_str())?,
    )?;
    Ok(HandleResponse::default())
}

fn try_revoke_role<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    role: String,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    only_admin(&env, &deps.storage)?;

    ac_revoke_role(
        &mut deps.storage,
        &address,
        &AccessRole::from_str(role.as_str())?,
    )?;
    Ok(HandleResponse::default())
}

fn try_renouce_role<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    role: String,
) -> StdResult<HandleResponse> {
    let ac_role = &AccessRole::from_str(role.as_str())?;
    let have_role = ac_have_role(&deps.storage, &env.message.sender, ac_role).unwrap_or(false);
    if !have_role {
        return Err(StdError::unauthorized());
    }
    ac_revoke_role(&mut deps.storage, &env.message.sender, ac_role)?;
    Ok(HandleResponse::default())
}

/* ***************************************************
 * *****************    Helpers      *****************
 * ***************************************************/

fn amount_from_funds(funds: &Vec<Coin>) -> Uint128 {
    // TODO(LR) return proper error
    if funds.len() == 1
    /* && funds[0].denom == "fet" */
    {
        // TODO(LR) does cosmwas allows sending multiple funds of the same token
        funds[0].amount
    } else {
        Uint128::zero()
    }
}

fn send_tokens_from_contract<A: Api>(
    api: &A,
    state: &State,
    to_address: &CanonicalAddr,
    amount: Uint128,
    action: &str,
) -> HandleResult {
    let to_human = api.human_address(to_address)?;
    let log = vec![log("action", action), log("to", to_human.as_str())];
    let coin = Coin {
        amount,
        denom: state.denom.clone(),
    };

    let r = HandleResponse {
        messages: vec![CosmosMsg::Bank(BankMsg::Send {
            from_address: state.contract_addr_human.clone(),
            to_address: to_human,
            amount: vec![coin],
        })],
        log,
        data: None,
    };
    Ok(r)
}

/* ***************************************************
 * ***************    Verifiers      *****************
 * ***************************************************/

fn verify_tx_relay_eon(eon: u64, state: &State) -> HandleResult {
    if eon != state.relay_eon {
        Err(StdError::generic_err(
            "Tx doesn't belong to current relayEon",
        ))
    } else {
        Ok(HandleResponse::default())
    }
}

fn verify_not_paused(env: &Env, state: &State) -> HandleResult {
    if env.block.height < state.paused_since_block {
        Ok(HandleResponse::default())
    } else {
        Err(StdError::generic_err(format!(
            "Contract has been paused {}",
            state.paused_since_block
        )))
    }
}

fn verify_swap_amount_limits(amount: Uint128, state: &State) -> HandleResult {
    if amount < state.lower_swap_limit {
        Err(StdError::generic_err("Amount bellow lower limit"))
    } else if amount > state.upper_swap_limit {
        Err(StdError::generic_err("Amount exceeds upper limit"))
    } else {
        Ok(HandleResponse::default())
    }
}

/* ***************************************************
 * ************    Access Control      ***************
 * ***************************************************/

fn only_admin<S: Storage>(env: &Env, storage: &S) -> HandleResult {
    match ac_have_role(storage, &env.message.sender, &AccessRole::Admin) {
        Ok(_) => Ok(HandleResponse::default()),
        Err(err) => {
            let owner = ac_get_owner(storage).unwrap_or(HumanAddr::from(""));
            if owner == env.message.sender {
                Ok(HandleResponse::default())
            } else {
                Err(err)
            }
        }
    }
}

fn only_relayer<S: Storage>(env: &Env, storage: &S) -> HandleResult {
    match ac_have_role(storage, &env.message.sender, &AccessRole::Relayer) {
        Ok(_) => Ok(HandleResponse::default()),
        Err(err) => Err(err),
    }
}

/* ***************************************************
 * *****************    Queries      *****************
 * ***************************************************/

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::HasRole { role, address } => to_binary(&query_role(deps, role, address)?),
    }
}

fn query_role<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    _role: u64,
    _address: HumanAddr,
) -> StdResult<RoleResponse> {
    Ok(RoleResponse { has_role: true })
}

/* ***************************************************
 * ******************    Tests      ******************
 * ***************************************************/

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::{
        coins, from_binary, Api, Coin, Extern, HumanAddr, InitResponse, Querier, ReadonlyStorage,
        StdError,
    };

    use crate::contract::{handle, init, query};
    use crate::msg::{HandleMsg, InitMsg, QueryMsg, Uint128};
    use crate::state::{config_read, State};

    pub const DEFAULT_CREATOR: &str = "creator";
    pub const DEFAULT_DENUM: &str = "fet";
    pub const DEFAULT_CAP: u128 = 100000u128;
    pub const DEFAULT_DEPOSIT: u128 = 10000u128;
    pub const DEFAULT_SWAP_UPPER_LIMIT: u128 = 1000u128;
    pub const DEFAULT_SWAP_LOWER_LIMIT: u128 = 10u128;
    pub const DEFAULT_SWAP_FEE: u128 = 9u128;

    macro_rules! cu128 {
        ($val:expr) => {
            // FIXME(LR) be more explicit of allowed expression
            Uint128::from($val)
        };
    }

    fn _mock_init(
        mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>,
        cap: Option<Uint128>,
        deposit: Option<Uint128>,
        upper_swap_limit: Option<Uint128>,
        lower_swap_limit: Option<Uint128>,
        swap_fee: Option<Uint128>,
    ) {
        let msg = InitMsg {
            cap: cap.unwrap_or(cu128!(DEFAULT_CAP)),
            deposit: deposit.unwrap_or(cu128!(DEFAULT_DEPOSIT)),
            upper_swap_limit: upper_swap_limit.unwrap_or(cu128!(DEFAULT_SWAP_UPPER_LIMIT)),
            lower_swap_limit: lower_swap_limit.unwrap_or(cu128!(DEFAULT_SWAP_LOWER_LIMIT)),
            swap_fee: swap_fee.unwrap_or(cu128!(DEFAULT_SWAP_FEE)),
            paused_since_block: None,
            delete_protection_period: None,
        };

        let env = mock_env(DEFAULT_CREATOR, &coins(1000, DEFAULT_DENUM));
        let _res = init(&mut deps, env, msg).expect("contract failed to handle InitMsg");
    }

    fn mock_init(mut deps: &mut Extern<MockStorage, MockApi, MockQuerier>) {
        _mock_init(&mut deps, None, None, None, None, None); // FIXME(LR) use variadic parameters
    }

    #[test]
    fn proper_init() {
        let mut deps = mock_dependencies(20, &[]);

        mock_init(&mut deps);

        let env = mock_env(DEFAULT_CREATOR, &coins(1000, DEFAULT_DENUM));
        let creator = HumanAddr::from(DEFAULT_CREATOR);
        //let contract_ha = deps.api.human_address(&env.contract.address).expect("");
        let expected_state = State {
            supply: cu128!(DEFAULT_DEPOSIT),
            refunds_fees_accrued: Uint128::from(0u128),
            next_swap_id: 0,
            sealed_reverse_swap_id: 0,
            relay_eon: 0,
            upper_swap_limit: cu128!(DEFAULT_SWAP_UPPER_LIMIT),
            lower_swap_limit: cu128!(DEFAULT_SWAP_LOWER_LIMIT),
            cap: cu128!(DEFAULT_CAP),
            swap_fee: cu128!(DEFAULT_SWAP_FEE),
            paused_since_block: u64::MAX,
            earliest_delete: env.block.height,
            denom: DEFAULT_DENUM.to_string(),
            contract_addr_human: env.contract.address.clone(),
        };

        let state = config_read(&deps.storage).load().expect("");

        assert_eq!(state, expected_state);
    }
}
