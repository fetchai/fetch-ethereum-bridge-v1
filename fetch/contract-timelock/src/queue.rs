use crate::error::{
    message_not_found_error, message_not_mature_error, message_not_next_error,
    message_not_pending_error, nonce_overflow_error,
};
use crate::state::State;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Order, StdResult, Storage, Uint128};
use cw_fetch_std::events::ResponseHandler;
use cw_fetch_std::storage::{evented_item, evented_map};
use cw_storage_plus::{Bound, Item, Map};

pub type NonceType = u64;

#[evented_map(key = "nonce", value = "message", updated = "message_updated")]
const MESSAGE_QUEUE: Map<NonceType, BridgeMessage> = Map::new("message_queue");

// Two pointers into MESSAGE_QUEUE: messages before NEXT_EXECUTE_NONCE are no longer pending,
// pending messages are in [NEXT_EXECUTE_NONCE, NEXT_SUBMIT_NONCE).

// Nonce of the next submitted message
#[evented_item(value = "nonce")]
const NEXT_SUBMIT_NONCE: Item<NonceType> = Item::new("next_submit_nonce");

// Nonce of the next message to execute: the first pending message, or NEXT_SUBMIT_NONCE if none
#[evented_item(value = "nonce")]
const NEXT_EXECUTE_NONCE: Item<NonceType> = Item::new("next_execute_nonce");

#[cw_serde]
pub struct BridgeMessage {
    pub payload: BridgeMessagePayload,
    pub execution_time: u64,
    pub state: ExecutionState,
}

#[cw_serde]
pub enum ExecutionState {
    Pending,
    Executed,
    Cancelled,
    Failed,
}

#[cw_serde]
pub enum BridgeMessagePayload {
    ReverseSwap {
        rid: u64,
        to: Addr,
        sender: String,
        origin_tx_hash: String, // TOD(LR) should be [32]u8 or String
        amount: Uint128,
        relay_eon: u64,
    },
    Refund {
        id: u64,
        to: Addr,
        amount: Uint128,
        relay_eon: u64,
    },
    RefundInFull {
        id: u64,
        to: Addr,
        amount: Uint128,
        relay_eon: u64,
    },
    NewRelayEon {},
}

impl BridgeMessagePayload {
    /// Delay between submission and the earliest execution.
    pub fn lock_delay(&self, state: &State) -> u64 {
        match self {
            Self::ReverseSwap { .. } => state.reverse_swap_lock_delay,
            Self::Refund { .. } | Self::RefundInFull { .. } => state.refund_lock_delay,
            Self::NewRelayEon {} => state.new_relay_eon_lock_delay,
        }
    }

    pub fn recipient(&self) -> Option<&Addr> {
        match self {
            Self::ReverseSwap { to, .. }
            | Self::Refund { to, .. }
            | Self::RefundInFull { to, .. } => Some(to),
            Self::NewRelayEon {} => None,
        }
    }
}

/// Messages are executed strictly in nonce order. Cancelled messages are skipped, and a
/// message leaves the queue as executed or failed depending on the bridge's result.
pub struct MessageQueue;

impl MessageQueue {
    /// Queues a pending message under the next submit nonce.
    /// Returns the newly allocated nonce. Nonces are never reused.
    pub fn push(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        payload: BridgeMessagePayload,
        execution_time: u64,
    ) -> StdResult<NonceType> {
        let nonce = Self::next_submit_nonce(storage)?;
        let next_submit_nonce = nonce.checked_add(1).ok_or_else(nonce_overflow_error)?;

        let message = BridgeMessage {
            payload,
            execution_time,
            state: ExecutionState::Pending,
        };
        MESSAGE_QUEUE.update(storage, response_handler, nonce, &message)?;
        NEXT_SUBMIT_NONCE.update(storage, response_handler, &next_submit_nonce)?;

        Ok(nonce)
    }

    /// Return the message with the given nonce, if it exists.
    pub fn get(storage: &dyn Storage, nonce: NonceType) -> StdResult<Option<BridgeMessage>> {
        MESSAGE_QUEUE.may_load(storage, nonce)
    }

    /// Nonce of the next submitted message.
    pub fn next_submit_nonce(storage: &dyn Storage) -> StdResult<NonceType> {
        Ok(NEXT_SUBMIT_NONCE.may_load(storage)?.unwrap_or_default())
    }

    /// Nonce of the next message to execute.
    pub fn next_execute_nonce(storage: &dyn Storage) -> StdResult<NonceType> {
        Ok(NEXT_EXECUTE_NONCE.may_load(storage)?.unwrap_or_default())
    }

    /// Pending messages in execution order, starting from the next one to execute.
    pub fn pending(
        storage: &dyn Storage,
        start_after: Option<NonceType>,
        limit: usize,
    ) -> StdResult<Vec<(NonceType, BridgeMessage)>> {
        let next_execute_nonce = Self::next_execute_nonce(storage)?;
        let min = match start_after {
            Some(nonce) if nonce >= next_execute_nonce => Bound::exclusive(nonce),
            _ => Bound::inclusive(next_execute_nonce),
        };
        Self::pending_from(storage, min).take(limit).collect()
    }

    /// Checks that `nonce` is next in order and mature, and moves the pointer past it.
    /// The message stays pending until [`Self::finish_execution`] records the result.
    pub fn start_execution(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
        now: u64,
    ) -> StdResult<BridgeMessage> {
        let next_execute_nonce = Self::next_execute_nonce(storage)?;
        if nonce != next_execute_nonce {
            return Err(message_not_next_error(nonce, next_execute_nonce));
        }
        let message = Self::load_pending(storage, nonce)?;
        if now < message.execution_time {
            return Err(message_not_mature_error(nonce, message.execution_time));
        }

        // Existing nonces are below the next submit nonce, so `nonce + 1` does not overflow
        Self::advance(storage, response_handler, nonce + 1)?;
        Ok(message)
    }

    /// Records whether the bridge accepted a message taken by [`Self::start_execution`].
    pub fn finish_execution(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
        succeeded: bool,
    ) -> StdResult<()> {
        let state = if succeeded {
            ExecutionState::Executed
        } else {
            ExecutionState::Failed
        };
        Self::set_state(storage, response_handler, nonce, state)
    }

    /// Cancels a pending message, execution then skips it.
    pub fn cancel(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
    ) -> StdResult<()> {
        Self::set_state(storage, response_handler, nonce, ExecutionState::Cancelled)?;
        if nonce == Self::next_execute_nonce(storage)? {
            Self::advance(storage, response_handler, nonce + 1)?;
        }
        Ok(())
    }

    fn load_pending(storage: &dyn Storage, nonce: NonceType) -> StdResult<BridgeMessage> {
        let message = Self::get(storage, nonce)?.ok_or_else(|| message_not_found_error(nonce))?;
        if message.state != ExecutionState::Pending {
            return Err(message_not_pending_error(nonce));
        }
        Ok(message)
    }

    fn set_state(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
        state: ExecutionState,
    ) -> StdResult<()> {
        let mut message = Self::load_pending(storage, nonce)?;
        message.state = state;
        MESSAGE_QUEUE.update(storage, response_handler, nonce, &message)
    }

    /// Moves the execute pointer to the first pending message from `from`, or to the submit
    /// pointer if there is none.
    fn advance(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        from: NonceType,
    ) -> StdResult<()> {
        let next_execute_nonce = match Self::pending_from(storage, Bound::inclusive(from))
            .next()
            .transpose()?
        {
            Some((nonce, _)) => nonce,
            None => Self::next_submit_nonce(storage)?,
        };
        NEXT_EXECUTE_NONCE.update(storage, response_handler, &next_execute_nonce)
    }

    fn pending_from<'a>(
        storage: &'a dyn Storage,
        min: Bound<'a, NonceType>,
    ) -> impl Iterator<Item = StdResult<(NonceType, BridgeMessage)>> + 'a {
        MESSAGE_QUEUE
            .range(storage, Some(min), None, Order::Ascending)
            .filter(|entry| {
                entry.as_ref().map_or(true, |(_, message)| {
                    message.state == ExecutionState::Pending
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, MockStorage};
    use cw_fetch_std::testing::helpers::response_events;

    fn push(storage: &mut dyn Storage, execution_time: u64) -> NonceType {
        let payload = BridgeMessagePayload::NewRelayEon {};
        MessageQueue::push(
            storage,
            &mut ResponseHandler::default(),
            payload,
            execution_time,
        )
        .unwrap()
    }

    fn state(storage: &dyn Storage, nonce: NonceType) -> ExecutionState {
        MessageQueue::get(storage, nonce).unwrap().unwrap().state
    }

    fn pending_nonces(
        storage: &dyn Storage,
        start_after: Option<NonceType>,
        limit: usize,
    ) -> Vec<NonceType> {
        let pending = MessageQueue::pending(storage, start_after, limit).unwrap();
        pending.into_iter().map(|(nonce, _)| nonce).collect()
    }

    fn execute(storage: &mut dyn Storage, nonce: NonceType, succeeded: bool) {
        let mut response_handler = ResponseHandler::default();
        MessageQueue::start_execution(storage, &mut response_handler, nonce, u64::MAX).unwrap();
        MessageQueue::finish_execution(storage, &mut response_handler, nonce, succeeded).unwrap();
    }

    fn cancel(storage: &mut dyn Storage, nonce: NonceType) -> StdResult<()> {
        MessageQueue::cancel(storage, &mut ResponseHandler::default(), nonce)
    }

    fn storage_with_messages(count: usize) -> MockStorage {
        let mut storage = MockStorage::new();
        for _ in 0..count {
            push(&mut storage, 100);
        }
        storage
    }

    #[test]
    fn push_allocates_sequential_pending_ids() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();

        for expected_nonce in 0..3 {
            let payload = BridgeMessagePayload::NewRelayEon {};
            let nonce =
                MessageQueue::push(&mut deps.storage, &mut response_handler, payload, 100).unwrap();
            assert_eq!(nonce, expected_nonce);
            assert_eq!(state(&deps.storage, nonce), ExecutionState::Pending);
        }
        assert_eq!(MessageQueue::next_submit_nonce(&deps.storage).unwrap(), 3);
        assert_eq!(MessageQueue::next_execute_nonce(&deps.storage).unwrap(), 0);

        let names: Vec<_> = response_events(response_handler)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(
            names,
            ["message_updated", "next_submit_nonce_updated"].repeat(3)
        );
    }

    #[test]
    fn nonce_overflow_does_not_insert() {
        let mut deps = mock_dependencies();
        NEXT_SUBMIT_NONCE
            .update(
                &mut deps.storage,
                &mut ResponseHandler::default(),
                &u64::MAX,
            )
            .unwrap();

        let mut response_handler = ResponseHandler::default();
        let payload = BridgeMessagePayload::NewRelayEon {};
        assert!(MessageQueue::push(&mut deps.storage, &mut response_handler, payload, 0).is_err());
        assert_eq!(MessageQueue::get(&deps.storage, u64::MAX).unwrap(), None);
        assert!(response_events(response_handler).is_empty());
    }

    #[test]
    fn execution_requires_next_id_and_maturity() {
        let mut storage = storage_with_messages(2);
        let mut response_handler = ResponseHandler::default();

        assert!(
            MessageQueue::start_execution(&mut storage, &mut response_handler, 1, 100).is_err()
        );
        assert!(MessageQueue::start_execution(&mut storage, &mut response_handler, 0, 99).is_err());
        assert!(response_events(response_handler).is_empty());
        assert_eq!(MessageQueue::next_execute_nonce(&storage).unwrap(), 0);

        let mut response_handler = ResponseHandler::default();
        MessageQueue::start_execution(&mut storage, &mut response_handler, 0, 100).unwrap();
        assert_eq!(MessageQueue::next_execute_nonce(&storage).unwrap(), 1);
        // Pending until the result is recorded
        assert_eq!(state(&storage, 0), ExecutionState::Pending);

        MessageQueue::finish_execution(&mut storage, &mut response_handler, 0, false).unwrap();
        assert_eq!(state(&storage, 0), ExecutionState::Failed);
        assert!(
            MessageQueue::finish_execution(&mut storage, &mut response_handler, 0, true).is_err()
        );
    }

    #[test]
    fn cancelled_messages_are_skipped() {
        let mut storage = storage_with_messages(4);

        // Cancelling ahead of the pointer leaves it in place
        cancel(&mut storage, 1).unwrap();
        cancel(&mut storage, 2).unwrap();
        assert_eq!(MessageQueue::next_execute_nonce(&storage).unwrap(), 0);

        execute(&mut storage, 0, true);
        assert_eq!(MessageQueue::next_execute_nonce(&storage).unwrap(), 3);
        assert_eq!(state(&storage, 0), ExecutionState::Executed);

        // Cancelling the last pending message moves the pointer past the end
        cancel(&mut storage, 3).unwrap();
        assert_eq!(MessageQueue::next_execute_nonce(&storage).unwrap(), 4);
        assert_eq!(push(&mut storage, 100), 4);
        assert_eq!(pending_nonces(&storage, None, 10), vec![4]);
    }

    #[test]
    fn only_pending_messages_can_be_cancelled() {
        let mut storage = storage_with_messages(3);
        execute(&mut storage, 0, true);
        execute(&mut storage, 1, false);
        cancel(&mut storage, 2).unwrap();

        // Executed, failed, cancelled and nonexistent
        for nonce in [0, 1, 2, 3] {
            let mut response_handler = ResponseHandler::default();
            assert!(MessageQueue::cancel(&mut storage, &mut response_handler, nonce).is_err());
            assert!(response_events(response_handler).is_empty());
        }
    }

    #[test]
    fn pending_pagination() {
        let mut storage = storage_with_messages(6);
        execute(&mut storage, 0, true);
        cancel(&mut storage, 2).unwrap();

        assert_eq!(pending_nonces(&storage, None, 10), vec![1, 3, 4, 5]);
        assert_eq!(pending_nonces(&storage, None, 2), vec![1, 3]);
        assert_eq!(pending_nonces(&storage, Some(3), 10), vec![4, 5]);
        // Never returns messages before the pointer
        assert_eq!(pending_nonces(&storage, Some(0), 10), vec![1, 3, 4, 5]);
        assert_eq!(
            pending_nonces(&storage, Some(u64::MAX), 10),
            Vec::<NonceType>::new()
        );
    }
}
