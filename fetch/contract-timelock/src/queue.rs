use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Order, StdResult, Storage, Uint128};
use cw_fetch_std::events::ResponseHandler;
use cw_storage_plus::Map;
use crate::error::global_nonce_overflow_error;
use crate::evented_storage::evented_map;
use crate::nonce::{GlobalNonce, NonceType};

#[evented_map(key = "id", value = "message", updated = "message_queued", removed = "message_removed")]
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

        MESSAGE_QUEUE.update(storage, response_handler, id, message)?;
        GlobalNonce::update_nonce(storage, response_handler, next_nonce)?;

        Ok(id)
    }

    /// Remove a queued message by ID. Does nothing if that ID is absent.
    /// Returns whether a message was removed.
    pub fn remove(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        id: NonceType,
    ) -> StdResult<bool> {
        MESSAGE_QUEUE.remove(storage, response_handler, id)
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
    use crate::evented_storage::events;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::to_json_string;

    fn message(execution_time: u64) -> BridgeMessage {
        BridgeMessage {
            payload: BridgeMessagePayload::NewRelayEon {},
            execution_time,
        }
    }

    fn attrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
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
        assert!(MessageQueue::remove(deps.as_mut().storage, &mut response_handler, 0).unwrap());
        assert_eq!(MessageQueue::get_first(deps.as_ref().storage).unwrap(), Some((1, second.clone())));

        // Removing an unknown ID is a no-op.
        assert!(!MessageQueue::remove(deps.as_mut().storage, &mut response_handler, 100).unwrap());
        assert_eq!(MessageQueue::get(deps.as_ref().storage, 100).unwrap(), None);

        assert_eq!(MessageQueue::get_all(deps.as_ref().storage).unwrap(), vec![(1, second), (2, third)]);

        let events = events(response_handler);
        assert_eq!(events.iter().filter(|(name, _)| name == "message_queued").count(), 3);
        assert_eq!(events.iter().filter(|(name, _)| name == "message_removed").count(), 1);
        assert_eq!(events.iter().filter(|(name, _)| name == "global_nonce_updated").count(), 3);
    }

    #[test]
    fn push_and_remove_events_are_compatible() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();
        let value = BridgeMessage {
            payload: BridgeMessagePayload::Refund {
                id: 7,
                to: Addr::unchecked("fetch1recipient"),
                amount: Uint128::new(1000),
                relay_eon: 3,
            },
            execution_time: 99,
        };
        let json = to_json_string(&value).unwrap();

        MessageQueue::push(deps.as_mut().storage, &mut response_handler, &value).unwrap();
        MessageQueue::remove(deps.as_mut().storage, &mut response_handler, 0).unwrap();

        // Same names, attributes and order as the hand-written events this replaced.
        assert_eq!(
            events(response_handler),
            vec![
                ("message_queued".to_string(), attrs(&[("id", "0"), ("message", &json)])),
                ("global_nonce_updated".to_string(), attrs(&[("nonce", "1")])),
                ("message_removed".to_string(), attrs(&[("id", "0")])),
            ]
        );
    }

    #[test]
    fn storage_layout_is_unchanged() {
        let mut deps = mock_dependencies();
        let plain: Map<u64, BridgeMessage> = Map::new("message_queue");

        plain.save(deps.as_mut().storage, 5, &message(1)).unwrap();
        assert_eq!(MessageQueue::get(deps.as_ref().storage, 5).unwrap(), Some(message(1)));

        MessageQueue::push(deps.as_mut().storage, &mut ResponseHandler::default(), &message(2)).unwrap();
        assert_eq!(plain.load(deps.as_ref().storage, 0).unwrap(), message(2));
    }

    #[test]
    fn removed_ids_are_not_reused() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();
        let value = message(0);

        let id = MessageQueue::push(deps.as_mut().storage, &mut response_handler, &value).unwrap();
        MessageQueue::remove(deps.as_mut().storage, &mut response_handler, id).unwrap();
        assert_eq!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &value).unwrap(), id + 1);
    }

    #[test]
    fn nonce_overflow_does_not_insert() {
        let mut deps = mock_dependencies();
        GlobalNonce::update_nonce(deps.as_mut().storage, &mut ResponseHandler::default(), u64::MAX).unwrap();

        let mut response_handler = ResponseHandler::default();
        assert!(MessageQueue::push(deps.as_mut().storage, &mut response_handler, &message(0)).is_err());
        assert!(MessageQueue::get_all(deps.as_ref().storage).unwrap().is_empty());
        assert_eq!(GlobalNonce::get_nonce(deps.as_ref().storage).unwrap(), u64::MAX);
        assert!(events(response_handler).is_empty());
    }
}
