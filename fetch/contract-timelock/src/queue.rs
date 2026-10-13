use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_string, Addr, Order, StdResult, Storage, Uint128};
use cw_fetch_std::events::{IntoEvent, ResponseHandler};
use cw_storage_plus::Map;
use crate::error::global_nonce_overflow_error;
use crate::nonce::{GlobalNonce, NonceType};

const MESSAGE_QUEUE: Map<u64, BridgeMessage> =
    Map::new("message_queue");

#[cw_serde]
pub struct BridgeMessage {
    pub payload: BridgeMessagePayload,
    pub execution_time: u64,
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



pub struct MessageQueuedEvent<'a> {
    pub id: NonceType,
    pub message: &'a BridgeMessage,
}

impl IntoEvent for MessageQueuedEvent<'_> {
    fn event_name(&self) -> &str {
        "message_queued"
    }

    fn event_attributes(&self) -> Vec<(String, String)> {
        vec![
            ("id".to_string(), self.id.to_string()),
            (
                "message".to_string(),
                to_json_string(self.message).expect("BridgeMessage must serialize to JSON"),
            ),
        ]
    }
}

pub struct MessageRemovedEvent {
    pub id: NonceType,
}

impl IntoEvent for MessageRemovedEvent {
    fn event_name(&self) -> &str {
        "message_removed"
    }

    fn event_attributes(&self) -> Vec<(String, String)> {
        vec![("id".to_string(), self.id.to_string())]
    }
}

pub struct MessageQueue;

impl MessageQueue {
    /// Insert a message using the next global nonce as its ID.
    /// Returns the newly allocated ID. IDs are never reused.
    pub fn push(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        message: &BridgeMessage,
    ) -> StdResult<NonceType> {
        let id = GlobalNonce::get_nonce(storage)?;
        let next_nonce = id
            .checked_add(1)
            .ok_or_else(|| global_nonce_overflow_error())?;

        MESSAGE_QUEUE.save(storage, id, message)?;

        response_handler.add_event(MessageQueuedEvent { id, message });
        GlobalNonce::update_nonce(storage, response_handler, next_nonce)?;

        Ok(id)
    }

    /// Remove a queued message by ID. Does nothing if that ID is absent.
    pub fn remove(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        id: NonceType,
    ) {
        if MESSAGE_QUEUE.has(storage, id) {
            MESSAGE_QUEUE.remove(storage, id);
            response_handler.add_event(MessageRemovedEvent { id });
        }
    }

    /// Return the message at the given ID, if it exists.
    pub fn get(storage: &dyn Storage, id: NonceType) -> StdResult<Option<BridgeMessage>> {
        MESSAGE_QUEUE.may_load(storage, id)
    }

    /// Return the lowest existing ID and its message, skipping removed entries.
    pub fn get_first(storage: &dyn Storage) -> StdResult<Option<(NonceType, BridgeMessage)>> {
        MESSAGE_QUEUE
            .range(storage, None, None, Order::Ascending)
            .next()
            .transpose()
    }

    /// Return all pending entries in ascending ID order.
    /// The caller can wrap this Vec in PendingMessagesResponse.
    pub fn get_all(storage: &dyn Storage) -> StdResult<Vec<(NonceType, BridgeMessage)>> {
        MESSAGE_QUEUE
            .range(storage, None, None, Order::Ascending)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;

    fn message(execution_time: u64) -> BridgeMessage {
        BridgeMessage {
            payload: BridgeMessagePayload::NewRelayEon {},
            execution_time,
        }
    }

    #[test]
    fn empty_queue() {
        let deps = mock_dependencies();
        assert_eq!(MessageQueue::get_first(deps.as_ref().storage).unwrap(), None);
        assert!(MessageQueue::get_all(deps.as_ref().storage)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn push_remove_and_skip_deleted_ids() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();

        let first = message(10);
        let second = message(20);
        let third = message(30);

        assert_eq!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &first).unwrap(), 0);
        assert_eq!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &second).unwrap(), 1);
        assert_eq!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &third).unwrap(), 2);
        assert_eq!(GlobalNonce::get_nonce(deps.as_ref().storage).unwrap(), 3);

        assert_eq!(MessageQueue::get_first(deps.as_ref().storage).unwrap(), Some((0, first.clone())));
        MessageQueue::remove(deps.as_mut().storage, &mut response_handler, 0);
        assert_eq!(MessageQueue::get_first(deps.as_ref().storage).unwrap(), Some((1, second.clone())));

        // Removing an unknown ID is a no-op.
        MessageQueue::remove(deps.as_mut().storage, &mut response_handler, 100);
        assert_eq!(MessageQueue::get(deps.as_ref().storage, 100).unwrap(), None);

        assert_eq!(MessageQueue::get_all(deps.as_ref().storage).unwrap(), vec![(1, second), (2, third)]);

        let response = response_handler.into_response();
        assert_eq!(response.events.iter().filter(|event| event.ty == "message_queued").count(), 3);
        assert_eq!(response.events.iter().filter(|event| event.ty == "message_removed").count(), 1);
        assert_eq!(response.events.iter().filter(|event| event.ty == "global_nonce_updated").count(), 3);
    }

    #[test]
    fn removed_ids_are_not_reused() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();
        let value = message(0);

        let id = MessageQueue::push(deps.as_mut().storage, &mut response_handler, &value).unwrap();
        MessageQueue::remove(deps.as_mut().storage, &mut response_handler, id);
        assert_eq!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &value).unwrap(), id + 1);
    }

    #[test]
    fn nonce_overflow_does_not_insert() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();
        GlobalNonce::update_nonce(deps.as_mut().storage, &mut response_handler, u64::MAX).unwrap();
        assert!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &message(0)).is_err());
        assert!(MessageQueue::get_all(deps.as_ref().storage).unwrap().is_empty());
    }
}