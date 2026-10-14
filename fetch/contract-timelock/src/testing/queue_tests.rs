use cosmwasm_std::testing::mock_env;
use cosmwasm_std::{Addr, Binary, CosmosMsg, Deps, ReplyOn, SubMsgResult, Uint128, WasmMsg};

use crate::contract::msg::{ExecuteMsg, MessageResponse, PendingMessagesResponse, QueryMsg};
use crate::queue::NonceType;
use crate::queue::{BridgeMessage, ExecutionState};
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

fn refund(to: &Addr) -> ExecuteMsg {
    ExecuteMsg::Refund {
        id: 1,
        to: to.clone(),
        amount: Uint128::new(100),
        relay_eon: 0,
    }
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
            ExecuteMsg::ReverseSwap {
                rid: 7,
                to: to.clone(),
                sender: "0xsender".to_string(),
                origin_tx_hash: "0xhash".to_string(),
                amount: Uint128::new(100),
                relay_eon: 0,
            },
            state.reverse_swap_lock_delay,
        ),
        (refund(&to), state.refund_lock_delay),
        (
            ExecuteMsg::RefundInFull {
                id: 2,
                to: to.clone(),
                amount: Uint128::new(100),
                relay_eon: 0,
            },
            state.refund_lock_delay,
        ),
        (ExecuteMsg::NewRelayEon {}, state.new_relay_eon_lock_delay),
    ];

    for (expected_nonce, (msg, delay)) in submissions.into_iter().enumerate() {
        let res = execute_contract(deps.as_mut(), &actors.submitter, msg).unwrap();
        let nonce = expected_nonce.to_string();
        assert!(res
            .attributes
            .iter()
            .any(|attr| attr.key == "nonce" && attr.value == nonce));
        assert_eq!(
            event_names(&res),
            vec!["message_updated", "next_submit_nonce_updated"]
        );

        let message = query_message(deps.as_ref(), expected_nonce as NonceType).unwrap();
        assert_eq!(message.execution_time, now() + delay);
        assert_eq!(message.state, ExecutionState::Pending);
    }
    assert_eq!(query_pending(deps.as_ref(), None, None), vec![0, 1, 2, 3]);
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
    for _ in 0..2 {
        execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).unwrap();
    }
    let mature = now() + init_state(&deps.api).new_relay_eon_lock_delay;

    execute_contract_at(
        deps.as_mut(),
        &actors.executor,
        ExecuteMsg::Execute { nonce: 0 },
        mature,
    )
    .unwrap();
    let res = reply_contract(deps.as_mut(), 0, ok_result()).unwrap();
    assert_eq!(event_names(&res), vec!["message_updated"]);
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
    for _ in 0..3 {
        execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).unwrap();
    }
    let mature = now() + init_state(&deps.api).new_relay_eon_lock_delay;
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
    for _ in 0..2 {
        execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).unwrap();
    }
    let pause = ExecuteMsg::Pause {
        since_block: mock_env().block.height,
    };
    execute_contract(deps.as_mut(), &actors.admin, pause).unwrap();
    let mature = now() + init_state(&deps.api).new_relay_eon_lock_delay;

    assert!(
        execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).is_err()
    );
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
    for _ in 0..40 {
        execute_contract(deps.as_mut(), &actors.submitter, ExecuteMsg::NewRelayEon {}).unwrap();
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
