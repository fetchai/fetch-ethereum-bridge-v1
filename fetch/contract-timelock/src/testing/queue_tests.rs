use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    Addr, Binary, CosmosMsg, Deps, DepsMut, OwnedDeps, ReplyOn, SubMsgResult, Uint128, WasmMsg,
};

use crate::contract::msg::{
    ExecuteMsg, LastMessageResponse, MessageResponse, MessagesResponse, NPendingMessagesResponse,
    PendingMessagesResponse, QueryMsg,
};
use crate::queue::NonceType;
use crate::queue::{BridgeMessage, ExecutionState};
use crate::state::StateUpdate;
use crate::testing::helpers::{
    event_names, execute_contract, execute_contract_at, init_state, ok_result, query_contract,
    reply_contract, setup_with_roles,
};

fn now() -> u64 {
    mock_env().block.time.seconds()
}

fn query_message(deps: Deps, nonce: NonceType) -> Option<BridgeMessage> {
    query_contract::<MessageResponse>(deps, QueryMsg::Message { nonce })
        .unwrap()
        .message
}

fn query_pending(deps: Deps, start_after: Option<NonceType>, limit: Option<u32>) -> Vec<NonceType> {
    let msg = QueryMsg::PendingMessages { start_after, limit };
    let response: PendingMessagesResponse = query_contract(deps, msg).unwrap();
    response
        .messages
        .into_iter()
        .map(|(nonce, _)| nonce)
        .collect()
}

fn query_n_pending(deps: Deps) -> u64 {
    query_contract::<NPendingMessagesResponse>(deps, QueryMsg::NPendingMessages {})
        .unwrap()
        .n_pending_messages
}

fn set_max_pending(deps: DepsMut, admin: &Addr, n_max_pending_messages: u64) {
    let update = StateUpdate {
        n_max_pending_messages: Some(n_max_pending_messages),
        ..Default::default()
    };
    execute_contract(deps, admin, ExecuteMsg::UpdateState(update)).unwrap();
}

fn query_nonces_and_states(deps: Deps, msg: QueryMsg) -> Vec<(NonceType, ExecutionState)> {
    let response: MessagesResponse = query_contract(deps, msg).unwrap();
    response
        .messages
        .into_iter()
        .map(|(nonce, message)| (nonce, message.state))
        .collect()
}

fn query_last_nonce(deps: Deps, msg: QueryMsg) -> Option<NonceType> {
    let response: LastMessageResponse = query_contract(deps, msg).unwrap();
    response.message.map(|(nonce, _)| nonce)
}

fn refund(to: &Addr) -> ExecuteMsg {
    ExecuteMsg::Refund {
        id: 1,
        to: to.clone(),
        amount: Uint128::new(100),
        relay_eon: 0,
    }
}

/// Refund with the given id, refunds with distinct ids do not collide.
fn refund_id(id: u64) -> ExecuteMsg {
    ExecuteMsg::Refund {
        id,
        to: MockApi::default().addr_make("recipient"),
        amount: Uint128::new(100),
        relay_eon: 0,
    }
}

fn reverse_swap(rid: u64) -> ExecuteMsg {
    ExecuteMsg::ReverseSwap {
        rid,
        to: MockApi::default().addr_make("recipient"),
        sender: "0xsender".to_string(),
        origin_tx_hash: "0xhash".to_string(),
        amount: Uint128::new(100),
        relay_eon: 0,
    }
}

fn cancel(deps: DepsMut, canceller: &Addr, nonce: NonceType) {
    execute_contract(deps, canceller, ExecuteMsg::Cancel { nonce }).unwrap();
}

/// Executes the (mature) message and records the bridge's result.
fn execute_with_result(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
    executor: &Addr,
    nonce: NonceType,
    result: SubMsgResult,
) {
    let execute = ExecuteMsg::Execute { nonce };
    let mature = now() + 1_000_000;
    execute_contract_at(deps.as_mut(), executor, execute, mature).unwrap();
    reply_contract(deps.as_mut(), nonce, result).unwrap();
}

#[test]
fn submit_requires_role() {
    let (mut deps, actors) = setup_with_roles();

    for sender in [&actors.admin, &actors.executor, &actors.canceller] {
        assert!(execute_contract(deps.as_mut(), sender, ExecuteMsg::NewRelayEon {}).is_err());
    }
    assert_eq!(query_message(deps.as_ref(), 0), None);
}

#[test]
fn submit_queues_pending_message_with_delay() {
    let (mut deps, actors) = setup_with_roles();
    let state = init_state(&deps.api);
    let to = deps.api.addr_make("recipient");

    let submissions = [
        (
            reverse_swap(7),
            state.reverse_swap_lock_delay,
            "rid_updated",
        ),
        (refund(&to), state.refund_lock_delay, "id_updated"),
        (
            ExecuteMsg::RefundInFull {
                id: 2,
                to: to.clone(),
                amount: Uint128::new(100),
                relay_eon: 0,
            },
            state.refund_lock_delay,
            "id_updated",
        ),
    ];

    for (expected_nonce, (msg, delay, index_event)) in submissions.into_iter().enumerate() {
        let res = execute_contract(deps.as_mut(), &actors.submitter, msg).unwrap();
        let nonce = expected_nonce.to_string();
        assert!(res
            .attributes
            .iter()
            .any(|attr| attr.key == "nonce" && attr.value == nonce));
        assert_eq!(
            event_names(&res),
            vec![
                "message_updated",
                index_event,
                "next_submit_nonce_updated",
                "n_pending_messages_updated"
            ]
        );

        let message = query_message(deps.as_ref(), expected_nonce as NonceType).unwrap();
        assert_eq!(message.execution_time, now() + delay);
        assert_eq!(message.state, ExecutionState::Pending);
    }
    assert_eq!(query_pending(deps.as_ref(), None, None), vec![0, 1, 2]);
}

#[test]
fn submit_rejects_invalid_recipient() {
    let (mut deps, actors) = setup_with_roles();

    let msg = refund(&Addr::unchecked("invalid"));
    assert!(execute_contract(deps.as_mut(), &actors.submitter, msg).is_err());
    assert_eq!(query_message(deps.as_ref(), 0), None);
}

#[test]
fn execute_dispatches_message_to_bridge() {
    let (mut deps, actors) = setup_with_roles();
    let to = deps.api.addr_make("recipient");
    execute_contract(deps.as_mut(), &actors.submitter, refund(&to)).unwrap();
    let mature = now() + init_state(&deps.api).refund_lock_delay;

    // Only the executor, only once mature
    let execute = ExecuteMsg::Execute { nonce: 0 };
    assert!(execute_contract_at(deps.as_mut(), &actors.admin, execute.clone(), mature).is_err());
    assert!(
        execute_contract_at(deps.as_mut(), &actors.executor, execute.clone(), mature - 1).is_err()
    );

    let res = execute_contract_at(deps.as_mut(), &actors.executor, execute, mature).unwrap();
    assert_eq!(res.messages.len(), 1);
    let submsg = &res.messages[0];
    assert_eq!((submsg.id, submsg.reply_on.clone()), (0, ReplyOn::Always));

    // Same JSON as the bridge's own ExecuteMsg::Refund
    let bridge_msg = format!(r#"{{"refund":{{"id":1,"to":"{to}","amount":"100","relay_eon":0}}}}"#);
    assert_eq!(
        submsg.msg,
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: init_state(&deps.api).bridge_addr.into_string(),
            msg: Binary::from(bridge_msg.as_bytes()),
            funds: vec![],
        })
    );
}

#[test]
fn reply_records_execution_result() {
    let (mut deps, actors) = setup_with_roles();
    for id in 0..2 {
        execute_contract(deps.as_mut(), &actors.submitter, refund_id(id)).unwrap();
    }
    let mature = now() + init_state(&deps.api).refund_lock_delay;

    execute_contract_at(
        deps.as_mut(),
        &actors.executor,
        ExecuteMsg::Execute { nonce: 0 },
        mature,
    )
    .unwrap();
    let res = reply_contract(deps.as_mut(), 0, ok_result()).unwrap();
    assert_eq!(
        event_names(&res),
        vec!["message_updated", "n_pending_messages_updated"]
    );
    assert_eq!(
        query_message(deps.as_ref(), 0).unwrap().state,
        ExecutionState::Executed
    );

    execute_contract_at(
        deps.as_mut(),
        &actors.executor,
        ExecuteMsg::Execute { nonce: 1 },
        mature,
    )
    .unwrap();
    let res = reply_contract(
        deps.as_mut(),
        1,
        SubMsgResult::Err("eon mismatch".to_string()),
    )
    .unwrap();
    assert!(res
        .attributes
        .iter()
        .any(|attr| attr.key == "error" && attr.value == "eon mismatch"));
    assert_eq!(
        query_message(deps.as_ref(), 1).unwrap().state,
        ExecutionState::Failed
    );

    // A failed message is final
    assert!(execute_contract_at(
        deps.as_mut(),
        &actors.executor,
        ExecuteMsg::Execute { nonce: 1 },
        mature
    )
    .is_err());
    assert!(query_pending(deps.as_ref(), None, None).is_empty());
}

#[test]
fn execution_is_in_order_and_skips_cancelled() {
    let (mut deps, actors) = setup_with_roles();
    for id in 0..3 {
        execute_contract(deps.as_mut(), &actors.submitter, refund_id(id)).unwrap();
    }
    let mature = now() + init_state(&deps.api).refund_lock_delay;
    let execute = |nonce| ExecuteMsg::Execute { nonce };

    // Only the canceller can cancel
    assert!(execute_contract(
        deps.as_mut(),
        &actors.admin,
        ExecuteMsg::Cancel { nonce: 1 }
    )
    .is_err());
    execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 1 },
    )
    .unwrap();
    assert_eq!(
        query_message(deps.as_ref(), 1).unwrap().state,
        ExecutionState::Cancelled
    );
    assert!(execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 1 }
    )
    .is_err());

    // Out of order
    assert!(execute_contract_at(deps.as_mut(), &actors.executor, execute(2), mature).is_err());

    execute_contract_at(deps.as_mut(), &actors.executor, execute(0), mature).unwrap();
    reply_contract(deps.as_mut(), 0, ok_result()).unwrap();

    assert!(execute_contract_at(deps.as_mut(), &actors.executor, execute(1), mature).is_err());
    execute_contract_at(deps.as_mut(), &actors.executor, execute(2), mature).unwrap();
    reply_contract(deps.as_mut(), 2, ok_result()).unwrap();

    assert!(query_pending(deps.as_ref(), None, None).is_empty());
    // Executed messages can not be cancelled
    assert!(execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 0 }
    )
    .is_err());
}

#[test]
fn pause_blocks_submit_and_execute_but_not_cancel() {
    let (mut deps, actors) = setup_with_roles();
    for id in 0..2 {
        execute_contract(deps.as_mut(), &actors.submitter, refund_id(id)).unwrap();
    }
    let pause = ExecuteMsg::Pause {
        since_block: mock_env().block.height,
    };
    execute_contract(deps.as_mut(), &actors.admin, pause).unwrap();
    let mature = now() + init_state(&deps.api).refund_lock_delay;

    for msg in [refund_id(2), ExecuteMsg::NewRelayEon {}] {
        assert!(execute_contract(deps.as_mut(), &actors.submitter, msg).is_err());
    }
    assert!(execute_contract_at(
        deps.as_mut(),
        &actors.executor,
        ExecuteMsg::Execute { nonce: 0 },
        mature
    )
    .is_err());
    execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 0 },
    )
    .unwrap();

    execute_contract(deps.as_mut(), &actors.admin, ExecuteMsg::Resume {}).unwrap();
    execute_contract_at(
        deps.as_mut(),
        &actors.executor,
        ExecuteMsg::Execute { nonce: 1 },
        mature,
    )
    .unwrap();
}

#[test]
fn pending_messages_query_is_paginated() {
    let (mut deps, actors) = setup_with_roles();
    set_max_pending(deps.as_mut(), &actors.admin, 40);
    for id in 0..40 {
        execute_contract(deps.as_mut(), &actors.submitter, refund_id(id)).unwrap();
    }
    execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 0 },
    )
    .unwrap();
    execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 2 },
    )
    .unwrap();

    assert_eq!(query_pending(deps.as_ref(), None, Some(3)), vec![1, 3, 4]);
    assert_eq!(query_pending(deps.as_ref(), Some(3), Some(2)), vec![4, 5]);
    // Default and maximum page sizes
    assert_eq!(query_pending(deps.as_ref(), None, None).len(), 10);
    assert_eq!(query_pending(deps.as_ref(), None, Some(100)).len(), 30);

    assert_eq!(query_message(deps.as_ref(), 40), None);
}

#[test]
fn submit_is_limited_by_max_pending_messages() {
    let (mut deps, actors) = setup_with_roles();
    let max_pending = init_state(&deps.api).n_max_pending_messages;
    let submit = |deps: DepsMut, id| execute_contract(deps, &actors.submitter, refund_id(id));

    for n_pending in 1..=max_pending {
        submit(deps.as_mut(), n_pending).unwrap();
        assert_eq!(query_n_pending(deps.as_ref()), n_pending);
    }
    assert!(submit(deps.as_mut(), 100).is_err());
    assert_eq!(query_message(deps.as_ref(), max_pending), None);

    // Cancelled, executed and failed messages free up space
    let mature = now() + init_state(&deps.api).refund_lock_delay;
    execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 0 },
    )
    .unwrap();
    for (nonce, result) in [(1, ok_result()), (2, SubMsgResult::Err("err".to_string()))] {
        let execute = ExecuteMsg::Execute { nonce };
        execute_contract_at(deps.as_mut(), &actors.executor, execute, mature).unwrap();
        // Pending until the result is recorded
        assert_eq!(query_n_pending(deps.as_ref()), max_pending - nonce);
        reply_contract(deps.as_mut(), nonce, result).unwrap();
    }
    assert_eq!(query_n_pending(deps.as_ref()), max_pending - 3);
    for id in 100..103 {
        submit(deps.as_mut(), id).unwrap();
    }
    assert!(submit(deps.as_mut(), 103).is_err());

    // Lowering the limit below the count only blocks new submissions
    set_max_pending(deps.as_mut(), &actors.admin, 1);
    assert!(submit(deps.as_mut(), 103).is_err());
    execute_contract(
        deps.as_mut(),
        &actors.canceller,
        ExecuteMsg::Cancel { nonce: 3 },
    )
    .unwrap();

    set_max_pending(deps.as_mut(), &actors.admin, max_pending + 1);
    submit(deps.as_mut(), 103).unwrap();
    assert_eq!(query_n_pending(deps.as_ref()), max_pending);

    // Zero blocks all submissions
    set_max_pending(deps.as_mut(), &actors.admin, 0);
    assert!(submit(deps.as_mut(), 104).is_err());
}

#[test]
fn same_id_is_rejected_while_pending_or_executed() {
    let (mut deps, actors) = setup_with_roles();
    let submit = |deps: DepsMut, msg| execute_contract(deps, &actors.submitter, msg);
    let refund_in_full = ExecuteMsg::RefundInFull {
        id: 1,
        to: deps.api.addr_make("recipient"),
        amount: Uint128::new(100),
        relay_eon: 0,
    };

    // Refunds and refunds in full share the id, reverse swaps have their own
    submit(deps.as_mut(), refund_id(1)).unwrap(); // 0
    assert!(submit(deps.as_mut(), refund_id(1)).is_err());
    assert!(submit(deps.as_mut(), refund_in_full.clone()).is_err());
    submit(deps.as_mut(), reverse_swap(1)).unwrap(); // 1
    assert!(submit(deps.as_mut(), reverse_swap(1)).is_err());

    // Cancelled and failed messages can be resubmitted
    cancel(deps.as_mut(), &actors.canceller, 0);
    submit(deps.as_mut(), refund_in_full).unwrap(); // 2
    execute_with_result(
        &mut deps,
        &actors.executor,
        1,
        SubMsgResult::Err("err".into()),
    );
    submit(deps.as_mut(), reverse_swap(1)).unwrap(); // 3

    // Executed messages can not
    execute_with_result(&mut deps, &actors.executor, 2, ok_result());
    assert!(submit(deps.as_mut(), refund_id(1)).is_err());
    assert_eq!(query_message(deps.as_ref(), 4), None);

    use ExecutionState::*;
    let refunds = |start_after, limit| QueryMsg::RefundMessages {
        id: 1,
        start_after,
        limit,
    };
    let reverse_swaps = |rid| QueryMsg::ReverseSwapMessages {
        rid,
        start_after: None,
        limit: None,
    };
    assert_eq!(
        query_nonces_and_states(deps.as_ref(), refunds(None, None)),
        vec![(0, Cancelled), (2, Executed)]
    );
    assert_eq!(
        query_nonces_and_states(deps.as_ref(), refunds(Some(0), None)),
        vec![(2, Executed)]
    );
    assert_eq!(
        query_nonces_and_states(deps.as_ref(), refunds(None, Some(1))),
        vec![(0, Cancelled)]
    );
    assert_eq!(
        query_nonces_and_states(deps.as_ref(), reverse_swaps(1)),
        vec![(1, Failed), (3, Pending)]
    );
    assert!(query_nonces_and_states(deps.as_ref(), reverse_swaps(2)).is_empty());

    let last_refund = |id| QueryMsg::LastRefundMessage { id };
    let last_reverse_swap = |rid| QueryMsg::LastReverseSwapMessage { rid };
    assert_eq!(query_last_nonce(deps.as_ref(), last_refund(1)), Some(2));
    assert_eq!(
        query_last_nonce(deps.as_ref(), last_reverse_swap(1)),
        Some(3)
    );
    assert_eq!(query_last_nonce(deps.as_ref(), last_refund(2)), None);
    assert_eq!(query_last_nonce(deps.as_ref(), last_reverse_swap(2)), None);
}

#[test]
fn new_relay_eon_cancels_pending_and_executes_immediately() {
    let (mut deps, actors) = setup_with_roles();
    for id in 0..3 {
        execute_contract(deps.as_mut(), &actors.submitter, refund_id(id)).unwrap();
    }
    execute_with_result(&mut deps, &actors.executor, 0, ok_result());

    // Executed right away, without waiting for maturity or the executor
    let res =
        execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).unwrap();
    assert_eq!(
        event_names(&res),
        vec![
            // Cancelling the pending messages
            "message_updated",
            "message_updated",
            "n_pending_messages_updated",
            "next_execute_nonce_updated",
            // Submitting and taking the new relay eon for execution
            "message_updated",
            "next_submit_nonce_updated",
            "n_pending_messages_updated",
            "next_execute_nonce_updated",
        ]
    );
    assert_eq!(res.messages.len(), 1);
    let submsg = &res.messages[0];
    assert_eq!((submsg.id, submsg.reply_on.clone()), (3, ReplyOn::Always));
    assert_eq!(
        submsg.msg,
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: init_state(&deps.api).bridge_addr.into_string(),
            msg: Binary::from(br#"{"new_relay_eon":{}}"#),
            funds: vec![],
        })
    );

    let states: Vec<_> = (0..4)
        .map(|nonce| query_message(deps.as_ref(), nonce).unwrap().state)
        .collect();
    use ExecutionState::*;
    assert_eq!(states, vec![Executed, Cancelled, Cancelled, Pending]);
    reply_contract(deps.as_mut(), 3, ok_result()).unwrap();
    assert_eq!(query_message(deps.as_ref(), 3).unwrap().state, Executed);
    assert_eq!(query_n_pending(deps.as_ref()), 0);
    assert!(query_pending(deps.as_ref(), None, None).is_empty());

    // The queue continues after it, cancelled ids can be resubmitted
    execute_contract(deps.as_mut(), &actors.submitter, refund_id(1)).unwrap();
    assert_eq!(query_pending(deps.as_ref(), None, None), vec![4]);

    // Not limited by the maximum of pending messages, a failure is recorded
    set_max_pending(deps.as_mut(), &actors.admin, 0);
    execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).unwrap();
    reply_contract(deps.as_mut(), 5, SubMsgResult::Err("err".into())).unwrap();
    assert_eq!(query_message(deps.as_ref(), 4).unwrap().state, Cancelled);
    assert_eq!(query_message(deps.as_ref(), 5).unwrap().state, Failed);
    assert_eq!(query_n_pending(deps.as_ref()), 0);
}
