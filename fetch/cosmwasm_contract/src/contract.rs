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
        admin: env.message.sender.clone(),
        relayer: env.message.sender.clone(),
        denom: env.message.sent_funds[0].denom.clone(), // TMP(LR)
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
            from,
            origin_tx_hash,
            amount,
            relay_eon,
        } => try_reverse_swap(
            deps,
            &env,
            &state,
            rid,
            to,
            from,
            origin_tx_hash,
            amount,
            relay_eon,
        ),
        HandleMsg::Refund {
            id: _,
            to: _,
            amount: _,
            relay_eon: _,
        } => Ok(HandleResponse::default()),
        HandleMsg::Pause { since_block: _ } => Ok(HandleResponse::default()),
        HandleMsg::FreezeFunds { amount: _ } => Ok(HandleResponse::default()),
        HandleMsg::UnFreezeFunds { amount: _ } => Ok(HandleResponse::default()),
        HandleMsg::SetCap { amount: _ } => Ok(HandleResponse::default()),
        HandleMsg::SetLimits {
            swap_min: _,
            swap_max: _,
            swap_fee: _,
        } => Ok(HandleResponse::default()),
        HandleMsg::GrantRole {
            role: _,
            address: _,
        } => Ok(HandleResponse::default()),
        HandleMsg::RevokeRole {
            role: _,
            address: _,
        } => Ok(HandleResponse::default()),
        HandleMsg::RenounceRole {
            role: _,
            address: _,
        } => Ok(HandleResponse::default()),
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
    verify_swap_amount(amount, state)?;

    let swap_id = state.next_swap_id;
    config(&mut deps.storage).update(|mut state| {
        state.supply += amount;
        state.next_swap_id += 1;
        Ok(state)
    })?;

    let log = vec![
        log("action", "swap"),
        log("destination", destination),
        log("swap_id", swap_id),
        log("amount", amount), // TOFIX(LR) how about fees?
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
    from: String,
    origin_tx_hash: Uint128,
    amount: Uint128,
    relay_eon: u64,
) -> StdResult<HandleResponse> {
    verify_tx_relay_eon(relay_eon, state)?;
    only_relayer(env, state)?;

    if amount > state.swap_fee {
        // TOFIX(LR) when amount == fee, amount will still be consumed
        let swap_fee = state.swap_fee;
        let effective_amount = (amount - swap_fee)?;
        let to_canonical = deps.api.canonical_address(&to)?;
        let rtx = send_tokens(
            &deps.api,
            &env,
            &state,
            &to_canonical,
            amount,
            "reverse_swap",
        )?;
        config(&mut deps.storage).update(|mut state| {
            state.supply = (state.supply - amount)?;
            state.sealed_reverse_swap_id += 1; // TOFIX(LR) after exec should be == rid
                                               //state.sealed_reverse_swap_id = rid;
            Ok(state)
        })?;

        let log = vec![
            log("action", "reverse_swap"),
            log("rid", rid),
            log("to", to),
            log("from", from),
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
            log("from", from),
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

fn send_tokens<A: Api>(
    api: &A,
    env: &Env,
    state: &State,
    to_address: &CanonicalAddr,
    amount: Uint128,
    action: &str,
) -> HandleResult {
    let from_address = &env.contract.address;
    let from_human = api.human_address(&from_address)?;
    let to_human = api.human_address(to_address)?;
    let log = vec![log("action", action), log("to", to_human.as_str())];
    let coin = Coin {
        amount,
        denom: state.denom.clone(),
    };

    let r = HandleResponse {
        messages: vec![CosmosMsg::Bank(BankMsg::Send {
            from_address: from_human,
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

fn verify_swap_amount(amount: Uint128, state: &State) -> HandleResult {
    if amount < state.lower_swap_limit {
        Err(StdError::generic_err("Amount bellow lower limit"))
    } else if amount > state.upper_swap_limit {
        Err(StdError::generic_err("Amount exceeds upper limit"))
    } else if (state.supply + amount) > state.cap {
        Err(StdError::generic_err("Swap would exceed cap"))
    } else {
        Ok(HandleResponse::default())
    }
}

/* ***************************************************
 * ************    Access Control      ***************
 * ***************************************************/

fn only_relayer(env: &Env, state: &State) -> HandleResult {
    if env.message.sender != state.relayer {
        Err(StdError::unauthorized())
    } else {
        Ok(HandleResponse::default())
    }
}

/*
fn perform_transfer<T: Storage>(
    store: &mut T,
    from: &CanonicalAddr,
    to: &CanonicalAddr,
    amount: u128,
) -> StdResult<()> {
    let mut balances_store = PrefixedStorage::new(PREFIX_BALANCES, store);

    let mut from_balance = read_u128(&balances_store, from.as_slice())?;
    if from_balance < amount {
        return Err(StdError::generic_err(format!(
            "Insufficient funds: balance={}, required={}",
            from_balance, amount
        )));
    }
    from_balance -= amount;
    balances_store.set(from.as_slice(), &from_balance.to_be_bytes());

    let mut to_balance = read_u128(&balances_store, to.as_slice())?;
    to_balance += amount;
    balances_store.set(to.as_slice(), &to_balance.to_be_bytes());

    Ok(())
}


// Converts 16 bytes value into u128
// Errors if data found that is not 16 bytes
pub fn bytes_to_u128(data: &[u8]) -> StdResult<u128> {
    match data[0..16].try_into() {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 16 byte expected.",
        )),
    }
}


// Reads 16 byte storage value into u128
// Returns zero if key does not exist. Errors if data found that is not 16 bytes
pub fn read_u128<S: ReadonlyStorage>(store: &S, key: &[u8]) -> StdResult<u128> {
    let result = store.get(key);
    match result {
        Some(data) => bytes_to_u128(&data),
        None => Ok(0u128),
    }
}
*/

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
