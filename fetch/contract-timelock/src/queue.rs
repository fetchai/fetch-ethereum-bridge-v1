use crate::error::{
    message_already_submitted_error, message_not_found_error, message_not_mature_error,
    message_not_next_error, message_not_pending_error, nonce_overflow_error,
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

// Nonce of the next submitted message
#[evented_item(value = "nonce")]
const NEXT_SUBMIT_NONCE: Item<NonceType> = Item::new("next_submit_nonce");

// Ordered set of the nonces of messages waiting for execution, the first one is next to execute.
// A message leaves it when taken for execution and stays pending until the result is recorded.
#[evented_map(key = "nonce")]
const PENDING_MESSAGES: Map<NonceType, ()> = Map::new("pending");

// Number of messages in the pending state
#[evented_item(value = "n")]
const N_PENDING_MESSAGES: Item<u64> = Item::new("n_pending");

// Ordered sets of the nonces of refunds (ID) and reverse swaps (RID) by their bridge id,
// see [`MessageIndex`]
#[evented_map(key = ("id", "nonce"))]
const ID: Map<(u64, NonceType), ()> = Map::new("id");

#[evented_map(key = ("rid", "nonce"))]
const RID: Map<(u64, NonceType), ()> = Map::new("rid");

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
            Self::NewRelayEon {} => 0,
        }
    }

    pub fn index(&self) -> Option<MessageIndex> {
        match self {
            Self::ReverseSwap { rid, .. } => Some(MessageIndex::ReverseSwap(*rid)),
            Self::Refund { id, .. } | Self::RefundInFull { id, .. } => {
                Some(MessageIndex::Refund(*id))
            }
            Self::NewRelayEon {} => None,
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

/// Reverse swaps are indexed by `rid`, refunds and refunds in full share the `id` space.
/// Of the messages with the same index, at most one is pending or executed: the last one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MessageIndex {
    ReverseSwap(u64),
    Refund(u64),
}

impl MessageIndex {
    fn nonces<'a>(
        self,
        storage: &'a dyn Storage,
        min: Option<Bound<'a, NonceType>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<NonceType>> + 'a> {
        match self {
            Self::ReverseSwap(rid) => RID.keys_prefix(storage, rid, min, None, order),
            Self::Refund(id) => ID.keys_prefix(storage, id, min, None, order),
        }
    }

    fn insert(
        self,
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
    ) -> StdResult<()> {
        match self {
            Self::ReverseSwap(rid) => RID.update(storage, response_handler, (rid, nonce), &()),
            Self::Refund(id) => ID.update(storage, response_handler, (id, nonce), &()),
        }
    }
}

/// Messages are executed strictly in nonce order. Cancelled messages are skipped, and a
/// message leaves the queue as executed or failed depending on the bridge's result.
pub struct MessageQueue;

impl MessageQueue {
    /// Queues a pending message under the next submit nonce. A message with an index is
    /// rejected while the last one with the same index is pending or executed.
    /// Returns the newly allocated nonce. Nonces are never reused.
    pub fn push(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        payload: BridgeMessagePayload,
        execution_time: u64,
    ) -> StdResult<NonceType> {
        let index = payload.index();
        if let Some(index) = index {
            if let Some((last_nonce, last)) = Self::last_by_index(storage, index)? {
                if matches!(
                    last.state,
                    ExecutionState::Pending | ExecutionState::Executed
                ) {
                    return Err(message_already_submitted_error(last_nonce));
                }
            }
        }
        let nonce = Self::next_submit_nonce(storage)?;
        let next_submit_nonce = nonce.checked_add(1).ok_or_else(nonce_overflow_error)?;

        let message = BridgeMessage {
            payload,
            execution_time,
            state: ExecutionState::Pending,
        };
        MESSAGE_QUEUE.update(storage, response_handler, nonce, &message)?;
        PENDING_MESSAGES.update(storage, response_handler, nonce, &())?;
        if let Some(index) = index {
            index.insert(storage, response_handler, nonce)?;
        }
        NEXT_SUBMIT_NONCE.update(storage, response_handler, &next_submit_nonce)?;
        // Pending messages have distinct nonces below `nonce`, so `n_pending + 1` does not overflow
        let n_pending = Self::n_pending(storage)? + 1;
        N_PENDING_MESSAGES.update(storage, response_handler, &n_pending)?;

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

    /// Number of pending messages.
    pub fn n_pending(storage: &dyn Storage) -> StdResult<u64> {
        Ok(N_PENDING_MESSAGES.may_load(storage)?.unwrap_or_default())
    }

    /// Pending messages in execution order, starting from the next one to execute.
    pub fn pending(
        storage: &dyn Storage,
        start_after: Option<NonceType>,
        limit: usize,
    ) -> StdResult<Vec<(NonceType, BridgeMessage)>> {
        PENDING_MESSAGES
            .keys(
                storage,
                start_after.map(Bound::exclusive),
                None,
                Order::Ascending,
            )
            .take(limit)
            .map(|nonce| Self::load_entry(storage, nonce?))
            .collect()
    }

    /// Messages with the given index in submission order.
    pub fn by_index(
        storage: &dyn Storage,
        index: MessageIndex,
        start_after: Option<NonceType>,
        limit: usize,
    ) -> StdResult<Vec<(NonceType, BridgeMessage)>> {
        index
            .nonces(storage, start_after.map(Bound::exclusive), Order::Ascending)
            .take(limit)
            .map(|nonce| Self::load_entry(storage, nonce?))
            .collect()
    }

    /// The last submitted message with the given index.
    pub fn last_by_index(
        storage: &dyn Storage,
        index: MessageIndex,
    ) -> StdResult<Option<(NonceType, BridgeMessage)>> {
        index
            .nonces(storage, None, Order::Descending)
            .next()
            .map(|nonce| Self::load_entry(storage, nonce?))
            .transpose()
    }

    /// Checks that `nonce` is next in order and mature, and takes it out of the pending set.
    /// The message stays pending until [`Self::finish_execution`] records the result.
    pub fn start_execution(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
        now: u64,
    ) -> StdResult<BridgeMessage> {
        if Self::first_pending(storage)? != Some(nonce) {
            return Err(message_not_next_error(nonce));
        }
        let message = Self::load_pending(storage, nonce)?;
        if now < message.execution_time {
            return Err(message_not_mature_error(nonce, message.execution_time));
        }

        PENDING_MESSAGES.remove(storage, response_handler, nonce)?;
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
        PENDING_MESSAGES.remove(storage, response_handler, nonce)?;
        Ok(())
    }

    /// Cancels all pending messages.
    pub fn cancel_all(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
    ) -> StdResult<()> {
        let pending: Vec<_> = PENDING_MESSAGES
            .keys(storage, None, None, Order::Ascending)
            .collect::<StdResult<_>>()?;
        for nonce in pending {
            let mut message = MESSAGE_QUEUE.load(storage, nonce)?;
            message.state = ExecutionState::Cancelled;
            MESSAGE_QUEUE.update(storage, response_handler, nonce, &message)?;
            PENDING_MESSAGES.remove(storage, response_handler, nonce)?;
        }
        N_PENDING_MESSAGES.update(storage, response_handler, &0)
    }

    /// Nonce of the next message to execute, if any.
    fn first_pending(storage: &dyn Storage) -> StdResult<Option<NonceType>> {
        PENDING_MESSAGES
            .keys(storage, None, None, Order::Ascending)
            .next()
            .transpose()
    }

    fn load_entry(
        storage: &dyn Storage,
        nonce: NonceType,
    ) -> StdResult<(NonceType, BridgeMessage)> {
        Ok((nonce, MESSAGE_QUEUE.load(storage, nonce)?))
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
        MESSAGE_QUEUE.update(storage, response_handler, nonce, &message)?;

        // Every pending message is counted, so `n_pending - 1` does not underflow
        let n_pending = Self::n_pending(storage)? - 1;
        N_PENDING_MESSAGES.update(storage, response_handler, &n_pending)
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

    fn n_pending(storage: &dyn Storage) -> u64 {
        MessageQueue::n_pending(storage).unwrap()
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
        assert_eq!(MessageQueue::first_pending(&deps.storage).unwrap(), Some(0));
        assert_eq!(n_pending(&deps.storage), 3);

        let names: Vec<_> = response_events(response_handler)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(
            names,
            [
                "message_updated",
                "pending_messages_updated",
                "next_submit_nonce_updated",
                "n_pending_messages_updated"
            ]
            .repeat(3)
        );
    }

    #[test]
    fn n_pending_counts_pending_messages() {
        let mut storage = storage_with_messages(4);
        assert_eq!(n_pending(&storage), 4);

        // Taken for execution, but pending until the result is recorded
        let mut response_handler = ResponseHandler::default();
        MessageQueue::start_execution(&mut storage, &mut response_handler, 0, u64::MAX).unwrap();
        assert_eq!(n_pending(&storage), 4);
        MessageQueue::finish_execution(&mut storage, &mut response_handler, 0, true).unwrap();
        assert_eq!(n_pending(&storage), 3);

        execute(&mut storage, 1, false);
        assert_eq!(n_pending(&storage), 2);
        cancel(&mut storage, 3).unwrap();
        assert_eq!(n_pending(&storage), 1);

        // Rejected transitions do not change the count
        assert!(cancel(&mut storage, 3).is_err());
        assert!(cancel(&mut storage, 4).is_err());
        assert_eq!(n_pending(&storage), 1);

        cancel(&mut storage, 2).unwrap();
        assert_eq!(n_pending(&storage), 0);
        assert_eq!(
            n_pending(&storage),
            pending_nonces(&storage, None, 10).len() as u64
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
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), Some(0));

        let mut response_handler = ResponseHandler::default();
        MessageQueue::start_execution(&mut storage, &mut response_handler, 0, 100).unwrap();
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), Some(1));
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

        // Cancelling after the first pending message leaves it next
        cancel(&mut storage, 1).unwrap();
        cancel(&mut storage, 2).unwrap();
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), Some(0));

        execute(&mut storage, 0, true);
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), Some(3));
        assert_eq!(state(&storage, 0), ExecutionState::Executed);

        // Cancelling the last pending message leaves none
        cancel(&mut storage, 3).unwrap();
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), None);
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

    fn push_refund(storage: &mut dyn Storage, id: u64) -> StdResult<NonceType> {
        let payload = BridgeMessagePayload::Refund {
            id,
            to: Addr::unchecked("to"),
            amount: Uint128::new(1),
            relay_eon: 0,
        };
        MessageQueue::push(storage, &mut ResponseHandler::default(), payload, 100)
    }

    #[test]
    fn rejected_duplicate_does_not_insert() {
        let mut storage = MockStorage::new();
        push_refund(&mut storage, 1).unwrap();

        let mut response_handler = ResponseHandler::default();
        let payload = BridgeMessagePayload::RefundInFull {
            id: 1,
            to: Addr::unchecked("to"),
            amount: Uint128::new(1),
            relay_eon: 0,
        };
        assert!(MessageQueue::push(&mut storage, &mut response_handler, payload, 100).is_err());
        assert!(response_events(response_handler).is_empty());
        assert_eq!(MessageQueue::next_submit_nonce(&storage).unwrap(), 1);
        assert_eq!(n_pending(&storage), 1);
        let refunds = MessageQueue::by_index(&storage, MessageIndex::Refund(1), None, 10);
        assert_eq!(refunds.unwrap().len(), 1);
    }

    #[test]
    fn cancel_all_cancels_only_pending() {
        let mut storage = storage_with_messages(2);
        push_refund(&mut storage, 1).unwrap();
        push(&mut storage, 100);
        execute(&mut storage, 0, true);
        cancel(&mut storage, 2).unwrap();

        MessageQueue::cancel_all(&mut storage, &mut ResponseHandler::default()).unwrap();
        let states: Vec<_> = (0..4).map(|nonce| state(&storage, nonce)).collect();
        use ExecutionState::*;
        assert_eq!(states, vec![Executed, Cancelled, Cancelled, Cancelled]);
        assert_eq!(n_pending(&storage), 0);
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), None);

        // Cancelled ids can be submitted again, the queue continues after the pointer
        assert_eq!(push_refund(&mut storage, 1).unwrap(), 4);
        assert_eq!(pending_nonces(&storage, None, 10), vec![4]);

        // Nothing pending
        let mut storage = MockStorage::new();
        MessageQueue::cancel_all(&mut storage, &mut ResponseHandler::default()).unwrap();
        assert_eq!(n_pending(&storage), 0);
        assert_eq!(MessageQueue::first_pending(&storage).unwrap(), None);
    }

    #[test]
    fn pending_pagination() {
        let mut storage = storage_with_messages(6);
        execute(&mut storage, 0, true);
        cancel(&mut storage, 2).unwrap();

        assert_eq!(pending_nonces(&storage, None, 10), vec![1, 3, 4, 5]);
        assert_eq!(pending_nonces(&storage, None, 2), vec![1, 3]);
        assert_eq!(pending_nonces(&storage, Some(3), 10), vec![4, 5]);
        assert_eq!(pending_nonces(&storage, Some(0), 10), vec![1, 3, 4, 5]);
        assert_eq!(
            pending_nonces(&storage, Some(u64::MAX), 10),
            Vec::<NonceType>::new()
        );
    }
}
