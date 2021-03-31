use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, CanonicalAddr, Coin, CosmosMsg, Env, Extern,
    HandleResponse, HandleResult, HumanAddr, InitResponse, Querier, StdError, StdResult, Storage,
};

use crate::access_control::{ac_add_role, ac_have_role, ac_revoke_role, AccessRole};
use crate::error::{
    ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE, ERR_ACCESS_CONTROL_ONLY_ADMIN,
    ERR_ACCESS_CONTROL_ONLY_RELAYER, ERR_ALREADY_REFUNDED, ERR_CAP_EXCEEDED, ERR_CONTRACT_PAUSED,
    ERR_EON, ERR_INVALID_SWAP_ID, ERR_RA_ALLOWANCE_EXCEEDED, ERR_SUPPLY_EXCEEDED,
    ERR_SWAP_LIMITS_INCONSISTENT, ERR_SWAP_LIMITS_VIOLATED, ERR_UNRECOGNIZED_DENOM,
};
use crate::msg::{
    CapResponse, DenomResponse, HandleMsg, InitMsg, PausedSinceBlockResponse, QueryMsg,
    RelayEonResponse, ReverseAggregatedAllowanceResponse, RoleResponse, SupplyResponse,
    SwapMaxResponse, Uint128,
};
use crate::state::{config, config_read, refunds_add, refunds_have, State};

pub const DEFAULT_DENOM: &str = "afet";

/* ***************************************************
 * **************    Initialization      *************
 * ***************************************************/

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let env_message_sender = deps.api.human_address(&env.message.sender)?;
    let current_block_number = env.block.height;

    let mut paused_since_block_public_api = msg.paused_since_block.unwrap_or(u64::MAX);
    if paused_since_block_public_api < current_block_number {
        paused_since_block_public_api = current_block_number;
    }
    let paused_since_block_relayer_api = paused_since_block_public_api;

    let contract_addr_human = deps.api.human_address(&env.contract.address)?;

    if msg.lower_swap_limit > msg.upper_swap_limit || msg.lower_swap_limit <= msg.swap_fee {
        return Err(StdError::generic_err(ERR_SWAP_LIMITS_INCONSISTENT));
    }

    let denom = msg.denom.unwrap_or(DEFAULT_DENOM.to_string());

    ac_add_role(&mut deps.storage, &env_message_sender, &AccessRole::Admin)?;

    let state = State {
        supply: Uint128::zero(),
        fees_accrued: Uint128::zero(),
        next_swap_id: 0,
        sealed_reverse_swap_id: 0,
        relay_eon: 0,
        upper_swap_limit: msg.upper_swap_limit,
        lower_swap_limit: msg.lower_swap_limit,
        reverse_aggregated_allowance: msg.reverse_aggregated_allowance,
        reverse_aggregated_allowance_approver_cap: msg.reverse_aggregated_allowance_approver_cap,
        cap: msg.cap,
        swap_fee: msg.swap_fee,
        paused_since_block_public_api,
        paused_since_block_relayer_api,
        denom,
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
            let amount = amount_from_funds(&env.message.sent_funds, state.denom.clone())?;
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
        HandleMsg::RefundInFull {
            id,
            to,
            amount,
            relay_eon,
        } => try_refund_in_full(deps, &env, &state, id, to, amount, relay_eon),
        HandleMsg::PausePublicApi { since_block } => try_pause_public_api(deps, &env, since_block),
        HandleMsg::PauseRelayerApi { since_block } => {
            try_pause_relayer_api(deps, &env, since_block)
        }
        HandleMsg::NewRelayEon {} => try_new_relay_eon(deps, &env, &state),
        HandleMsg::Deposit {} => try_deposit(deps, &env, &state),
        HandleMsg::Withdraw {
            amount,
            destination,
        } => try_withdraw(deps, &env, &state, amount, destination),
        HandleMsg::WithdrawFees {
            amount,
            destination,
        } => try_withdraw_fees(deps, &env, &state, amount, destination),
        HandleMsg::SetCap { amount } => try_set_cap(deps, &env, amount),
        HandleMsg::SetReverseAggregatedAllowance { amount } => {
            try_set_reverse_aggregated_allowance(deps, &env, &state, amount)
        }
        HandleMsg::SetReverseAggregatedAllowanceApproverCap { amount } => {
            try_set_reverse_aggregated_allowance_approver_cap(deps, &env, amount)
        }
        HandleMsg::SetLimits {
            swap_min,
            swap_max,
            swap_fee,
        } => try_set_limits(deps, &env, swap_min, swap_max, swap_fee),
        HandleMsg::GrantRole { role, address } => try_grant_role(deps, &env, role, address),
        HandleMsg::RevokeRole { role, address } => try_revoke_role(deps, &env, role, address),
        HandleMsg::RenounceRole { role } => try_renounce_role(deps, &env, role),
    }
}

fn try_swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    amount: Uint128,
    destination: String,
) -> StdResult<HandleResponse> {
    verify_not_paused_public_api(env, state)?;
    verify_swap_amount_limits(amount, state)?;

    let increased_supply = state.supply + amount;
    if increased_supply > state.cap {
        return Err(StdError::generic_err(ERR_CAP_EXCEEDED));
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
    only_relayer(env, &deps.storage, &deps.api)?;
    verify_tx_relay_eon(relay_eon, state)?;
    verify_not_paused_relayer_api(env, state)?;
    verify_aggregated_reverse_allowance(amount, state)?;

    if amount > state.supply {
        return Err(StdError::generic_err(ERR_SUPPLY_EXCEEDED));
    }

    if amount > state.swap_fee {
        // NOTE(LR) when amount == fee, amount will still be consumed
        // FIXME(LR) not fair for user IMO
        let swap_fee = state.swap_fee;
        let effective_amount = (amount - swap_fee)?;
        let to_canonical = deps.api.canonical_address(&to)?;
        let rtx = send_tokens_from_contract(
            &deps.api,
            &state,
            &to_canonical,
            effective_amount,
            "reverse_swap",
        )?;
        config(&mut deps.storage).update(|mut state| {
            state.supply = (state.supply - amount)?;
            state.reverse_aggregated_allowance = (state.reverse_aggregated_allowance - amount)?;
            state.fees_accrued += swap_fee;
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
        // FIXME(LR) store revese swap id similarly to refunds?

        let r = HandleResponse {
            messages: rtx.messages,
            log,
            data: None,
        };
        Ok(r)
    } else {
        // FIXME(LR) this unfair for the user IMO
        let swap_fee = amount;
        let effective_amount = Uint128::zero();
        config(&mut deps.storage).update(|mut state| {
            state.supply = (state.supply - amount)?;
            state.reverse_aggregated_allowance = (state.reverse_aggregated_allowance - amount)?;
            state.fees_accrued += swap_fee;
            //state.sealed_reverse_swap_id = rid; // TODO(LR)
            Ok(state)
        })?;

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
fn _try_refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    id: u64,
    to: HumanAddr,
    amount: Uint128,
    relay_eon: u64,
    fee: Uint128,
) -> StdResult<HandleResponse> {
    only_relayer(env, &deps.storage, &deps.api)?;
    verify_tx_relay_eon(relay_eon, state)?;
    verify_not_paused_relayer_api(env, state)?;
    verify_refund_swap_id(id, &deps.storage)?;
    verify_aggregated_reverse_allowance(amount, state)?;

    if amount > state.supply {
        return Err(StdError::generic_err(ERR_SUPPLY_EXCEEDED));
    }

    if amount > fee {
        let new_supply = (state.supply - amount)?;
        let effective_amount = (amount - fee)?;
        let to_canonical = deps.api.canonical_address(&to)?;
        let rtx = send_tokens_from_contract(
            &deps.api,
            &state,
            &to_canonical,
            effective_amount,
            "refund",
        )?;

        config(&mut deps.storage).update(|mut state| {
            state.supply = new_supply;
            state.reverse_aggregated_allowance = (state.reverse_aggregated_allowance - amount)?;
            state.fees_accrued += fee;
            Ok(state)
        })?;

        refunds_add(id, &mut deps.storage);

        let log = vec![
            log("action", "refund"),
            log("destination", to),
            log("swap_id", id),
            log("amount", effective_amount),
            log("refund_fee", fee),
        ];

        let r = HandleResponse {
            messages: rtx.messages,
            log,
            data: None,
        };
        Ok(r)
    } else {
        let refund_fee = amount;
        let new_supply = (state.supply - amount)?;
        let effective_amount = Uint128::zero();

        config(&mut deps.storage).update(|mut state| {
            state.reverse_aggregated_allowance = (state.reverse_aggregated_allowance - amount)?;
            state.supply = new_supply;
            state.fees_accrued += refund_fee;
            Ok(state)
        })?;

        refunds_add(id, &mut deps.storage);

        let log = vec![
            log("action", "refund"),
            log("destination", to),
            log("swap_id", id),
            log("amount", effective_amount),
            log("refund_fee", refund_fee),
        ];

        let r = HandleResponse {
            messages: vec![],
            log,
            data: None,
        };
        Ok(r)
    }
}

fn try_refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    id: u64,
    to: HumanAddr,
    amount: Uint128,
    relay_eon: u64,
) -> StdResult<HandleResponse> {
    _try_refund(deps, env, state, id, to, amount, relay_eon, state.swap_fee)
}

fn try_refund_in_full<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    id: u64,
    to: HumanAddr,
    amount: Uint128,
    relay_eon: u64,
) -> StdResult<HandleResponse> {
    _try_refund(deps, env, state, id, to, amount, relay_eon, Uint128::zero())
}

fn try_pause_public_api<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    since_block: u64,
) -> StdResult<HandleResponse> {
    can_pause(env, &deps.storage, &deps.api, since_block)?;

    let pause_since_block = if since_block < env.block.height {
        env.block.height
    } else {
        since_block
    };
    config(&mut deps.storage).update(|mut state| {
        state.paused_since_block_public_api = pause_since_block;
        Ok(state)
    })?;

    let log = vec![
        log("action", "pause_public_api"),
        log("since_block", pause_since_block),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_pause_relayer_api<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    since_block: u64,
) -> StdResult<HandleResponse> {
    can_pause(env, &deps.storage, &deps.api, since_block)?;

    let pause_since_block = if since_block < env.block.height {
        env.block.height
    } else {
        since_block
    };
    config(&mut deps.storage).update(|mut state| {
        state.paused_since_block_relayer_api = pause_since_block;
        Ok(state)
    })?;

    let log = vec![
        log("action", "pause_relayer_api"),
        log("since_block", pause_since_block),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_new_relay_eon<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
) -> StdResult<HandleResponse> {
    only_relayer(env, &deps.storage, &deps.api)?;
    verify_not_paused_relayer_api(env, state)?;

    let new_eon = state.relay_eon + 1;
    config(&mut deps.storage).update(|mut state| {
        state.relay_eon = new_eon; // FIXME(LR) starts from 1
        Ok(state)
    })?;

    let log = vec![log("action", "new_relay_eon"), log("eon", new_eon)];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None, // TODO(LR) what can I send in data?
    };
    Ok(r)
}

fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api)?;

    let env_message_sender = deps.api.human_address(&env.message.sender)?;

    let amount = amount_from_funds(&env.message.sent_funds, state.denom.clone())?;
    config(&mut deps.storage).update(|mut state| {
        state.supply += amount;
        Ok(state)
    })?;

    let log = vec![
        log("action", "deposit"),
        log("amount", amount),
        log("sender", env_message_sender.as_str()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    amount: Uint128,
    destination: HumanAddr,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api)?;

    if amount > state.supply {
        return Err(StdError::generic_err(ERR_SUPPLY_EXCEEDED));
    }

    let new_supply = (state.supply - amount)?;
    config(&mut deps.storage).update(|mut state| {
        state.supply = new_supply;
        Ok(state)
    })?;
    let recipient = deps.api.canonical_address(&destination)?;
    let wtx = send_tokens_from_contract(&deps.api, &state, &recipient, amount, "withdraw")?;

    let log = vec![
        log("action", "withdraw"),
        log("amount", amount),
        log("destination", destination.as_str()),
    ];

    let r = HandleResponse {
        messages: wtx.messages,
        log,
        data: None,
    };
    Ok(r)
}

fn try_withdraw_fees<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    amount: Uint128,
    destination: HumanAddr,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api)?;

    if amount > state.fees_accrued {
        return Err(StdError::generic_err(ERR_SUPPLY_EXCEEDED));
    }

    let new_fees_accrued = (state.fees_accrued - amount)?;
    config(&mut deps.storage).update(|mut state| {
        state.fees_accrued = new_fees_accrued;
        Ok(state)
    })?;

    let recipient = deps.api.canonical_address(&destination)?;
    let wtx = send_tokens_from_contract(&deps.api, &state, &recipient, amount, "withdraw_fees")?;

    let log = vec![
        log("action", "withdraw_fees"),
        log("amount", amount),
        log("destination", destination.as_str()),
    ];

    let r = HandleResponse {
        messages: wtx.messages,
        log,
        data: None,
    };
    Ok(r)
}

fn try_set_cap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api)?;

    config(&mut deps.storage).update(|mut state| {
        state.cap = amount;
        Ok(state)
    })?;

    let log = vec![log("action", "set_cap"), log("cap", amount)];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_set_reverse_aggregated_allowance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    state: &State,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api).or(
        if amount <= state.reverse_aggregated_allowance_approver_cap {
            only_approver(env, &deps.storage, &deps.api)
        } else {
            Err(StdError::generic_err(ERR_ACCESS_CONTROL_ONLY_ADMIN))
        },
    )?;

    config(&mut deps.storage).update(|mut state| {
        state.reverse_aggregated_allowance = amount;
        Ok(state)
    })?;

    let log = vec![
        log("action", "set_reverse_aggregated_allowance"),
        log("amount", amount),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_set_reverse_aggregated_allowance_approver_cap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api)?;

    config(&mut deps.storage).update(|mut state| {
        state.reverse_aggregated_allowance_approver_cap = amount;
        Ok(state)
    })?;

    let log = vec![
        log("action", "set_reverse_aggregated_allowance_approver_cap"),
        log("amount", amount),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_set_limits<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    swap_min: Uint128,
    swap_max: Uint128,
    swap_fee: Uint128,
) -> StdResult<HandleResponse> {
    only_admin(env, &deps.storage, &deps.api)?;

    if swap_min <= swap_fee || swap_min > swap_max {
        return Err(StdError::generic_err(ERR_SWAP_LIMITS_INCONSISTENT));
    }
    config(&mut deps.storage).update(|mut state| {
        state.swap_fee = swap_fee;
        state.lower_swap_limit = swap_min;
        state.upper_swap_limit = swap_max;
        Ok(state)
    })?;

    let log = vec![
        log("action", "set_limits"),
        log("swap_fee", swap_fee),
        log("swap_min", swap_min),
        log("swap_max", swap_max),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_grant_role<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    role: String,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    only_admin(&env, &deps.storage, &deps.api)?;

    ac_add_role(
        &mut deps.storage,
        &address,
        &AccessRole::from_str(role.as_str())?,
    )?;

    let log = vec![
        log("action", "grant_role"),
        log("role", role.as_str()),
        log("account", address.as_str()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_revoke_role<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    role: String,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    only_admin(&env, &deps.storage, &deps.api)?;

    ac_revoke_role(
        &mut deps.storage,
        &address,
        &AccessRole::from_str(role.as_str())?,
    )?;

    let log = vec![
        log("action", "revoke_role"),
        log("role", role.as_str()),
        log("account", address.as_str()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

fn try_renounce_role<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    role: String,
) -> StdResult<HandleResponse> {
    let env_message_sender = deps.api.human_address(&env.message.sender)?;

    let ac_role = &AccessRole::from_str(role.as_str())?;
    let have_role = ac_have_role(&deps.storage, &env_message_sender, ac_role).unwrap_or(false);
    if !have_role {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE));
    }
    ac_revoke_role(&mut deps.storage, &env_message_sender, ac_role)?;

    let log = vec![
        log("action", "renounce_role"),
        log("role", role.as_str()),
        log("account", &env_message_sender.as_str()),
    ];

    let r = HandleResponse {
        messages: vec![],
        log,
        data: None,
    };
    Ok(r)
}

/* ***************************************************
 * *****************    Helpers      *****************
 * ***************************************************/

pub fn amount_from_funds(funds: &Vec<Coin>, denom: String) -> StdResult<Uint128> {
    for coin in funds {
        if coin.denom == denom {
            return Ok(coin.amount);
        }
    }
    Err(StdError::generic_err(ERR_UNRECOGNIZED_DENOM))
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
        Err(StdError::generic_err(ERR_EON))
    } else {
        Ok(HandleResponse::default())
    }
}

pub fn verify_not_paused_public_api(env: &Env, state: &State) -> HandleResult {
    _verify_not_paused(env, state.paused_since_block_public_api)
}

pub fn verify_not_paused_relayer_api(env: &Env, state: &State) -> HandleResult {
    _verify_not_paused(env, state.paused_since_block_relayer_api)
}

fn _verify_not_paused(env: &Env, paused_since_block: u64) -> HandleResult {
    if env.block.height < paused_since_block {
        Ok(HandleResponse::default())
    } else {
        Err(StdError::generic_err(ERR_CONTRACT_PAUSED))
    }
}

fn verify_swap_amount_limits(amount: Uint128, state: &State) -> HandleResult {
    if amount < state.lower_swap_limit {
        Err(StdError::generic_err(ERR_SWAP_LIMITS_VIOLATED))
    } else if amount > state.upper_swap_limit {
        Err(StdError::generic_err(ERR_SWAP_LIMITS_VIOLATED))
    } else {
        Ok(HandleResponse::default())
    }
}

fn verify_aggregated_reverse_allowance(amount: Uint128, state: &State) -> HandleResult {
    if state.reverse_aggregated_allowance < amount {
        Err(StdError::generic_err(ERR_RA_ALLOWANCE_EXCEEDED))
    } else {
        Ok(HandleResponse::default())
    }
}

fn verify_refund_swap_id<S: Storage>(id: u64, storage: &S) -> HandleResult {
    let state = config_read(storage).load()?;
    if id >= state.next_swap_id {
        // FIXME(LR) >= ?
        return Err(StdError::generic_err(ERR_INVALID_SWAP_ID));
    }
    match refunds_have(id, storage) {
        true => Err(StdError::generic_err(ERR_ALREADY_REFUNDED)),
        false => Ok(HandleResponse::default()),
    }
}

/* ***************************************************
 * ************    Access Control      ***************
 * ***************************************************/

fn only_admin<S: Storage, A: Api>(env: &Env, storage: &S, api: &A) -> HandleResult {
    _only_role(&AccessRole::Admin, env, storage, api)
}

fn only_relayer<S: Storage, A: Api>(env: &Env, storage: &S, api: &A) -> HandleResult {
    _only_role(&AccessRole::Relayer, env, storage, api)
}

fn only_approver<S: Storage, A: Api>(env: &Env, storage: &S, api: &A) -> HandleResult {
    _only_role(&AccessRole::Approver, env, storage, api)
}

fn only_monitor<S: Storage, A: Api>(env: &Env, storage: &S, api: &A) -> HandleResult {
    _only_role(&AccessRole::Monitor, env, storage, api)
}

fn _only_role<S: Storage, A: Api>(
    role: &AccessRole,
    env: &Env,
    storage: &S,
    api: &A,
) -> HandleResult {
    let env_message_sender = api.human_address(&env.message.sender)?;

    match ac_have_role(storage, &env_message_sender, role) {
        Ok(has_role) => match has_role {
            true => Ok(HandleResponse::default()),
            false => Err(StdError::generic_err(match role {
                AccessRole::Admin => ERR_ACCESS_CONTROL_ONLY_ADMIN,
                AccessRole::Relayer => ERR_ACCESS_CONTROL_ONLY_RELAYER,
                _ => ERR_ACCESS_CONTROL_ONLY_ADMIN,
            })),
        },
        Err(err) => Err(err),
    }
}

fn can_pause<S: Storage, A: Api>(
    env: &Env,
    storage: &S,
    api: &A,
    since_block: u64,
) -> HandleResult {
    if since_block > env.block.height {
        // unpausing
        only_admin(env, storage, api)
    } else {
        // pausing
        only_monitor(env, storage, api).or(only_admin(env, storage, api))
    }
}

/* ***************************************************
 * *****************    Queries      *****************
 * ***************************************************/

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    let state = config_read(&deps.storage).load()?;
    match msg {
        QueryMsg::HasRole { role, address } => to_binary(&query_role(deps, role, address)?),
        QueryMsg::RelayEon {} => to_binary(&RelayEonResponse {
            eon: state.relay_eon,
        }),
        QueryMsg::Supply {} => to_binary(&SupplyResponse {
            amount: state.supply,
        }),
        QueryMsg::Cap {} => to_binary(&CapResponse { amount: state.cap }),
        QueryMsg::SwapMax {} => to_binary(&SwapMaxResponse {
            amount: state.upper_swap_limit,
        }),
        QueryMsg::ReverseAggregatedAllowance {} => to_binary(&ReverseAggregatedAllowanceResponse {
            amount: state.reverse_aggregated_allowance,
        }),
        QueryMsg::PausedPublicApiSince {} => to_binary(&PausedSinceBlockResponse {
            block: state.paused_since_block_public_api,
        }),
        QueryMsg::PausedRelayerApiSince {} => to_binary(&PausedSinceBlockResponse {
            block: state.paused_since_block_relayer_api,
        }),
        QueryMsg::Denom {} => to_binary(&DenomResponse {
            denom: state.denom.clone(),
        }),
        QueryMsg::FullState {} => to_binary(&state.clone()),
    }
}

fn query_role<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    role: String,
    address: HumanAddr,
) -> StdResult<RoleResponse> {
    match ac_have_role(
        &deps.storage,
        &address,
        &AccessRole::from_str(role.as_str())?,
    ) {
        Ok(has_role) => match has_role {
            true => Ok(RoleResponse { has_role: true }),
            false => Ok(RoleResponse { has_role: false }),
        },
        Err(_) => Ok(RoleResponse { has_role: false }),
    }
}
