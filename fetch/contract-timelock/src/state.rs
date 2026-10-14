use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, StdResult, Storage};
use cw_fetch_std::events::ResponseHandler;
use cw_storage_plus::Item;

use crate::evented_storage::evented_item;

#[evented_item(value = "state")]
const STATE: Item<State> = Item::new("state");

#[cw_serde]
pub struct State {
    pub bridge_addr: Addr,
    pub reverse_swap_lock_delay: u64,
    pub refund_lock_delay: u64,
    pub new_relay_eon_lock_delay: u64,
}

/// Partial update of [`State`]: a missing or `null` field keeps its current value.
#[cw_serde]
#[derive(Default)]
pub struct StateUpdate {
    pub bridge_addr: Option<Addr>,
    pub reverse_swap_lock_delay: Option<u64>,
    pub refund_lock_delay: Option<u64>,
    pub new_relay_eon_lock_delay: Option<u64>,
}

impl State {
    pub fn load(storage: &dyn Storage) -> StdResult<State> {
        STATE.load(storage)
    }

    /// Validates and stores the state.
    pub fn save(
        &self,
        storage: &mut dyn Storage,
        api: &dyn Api,
        response_handler: &mut ResponseHandler,
    ) -> StdResult<()> {
        api.addr_validate(self.bridge_addr.as_str())?;
        STATE.update(storage, response_handler, self)
    }

    /// Applies `update` to the stored state and returns the result.
    pub fn update(
        storage: &mut dyn Storage,
        api: &dyn Api,
        response_handler: &mut ResponseHandler,
        update: StateUpdate,
    ) -> StdResult<State> {
        // Exhaustive destructuring and struct literal: adding a field to either struct
        // fails to compile until it is handled here.
        let StateUpdate {
            bridge_addr,
            reverse_swap_lock_delay,
            refund_lock_delay,
            new_relay_eon_lock_delay,
        } = update;
        let current = Self::load(storage)?;
        let state = State {
            bridge_addr: bridge_addr.unwrap_or(current.bridge_addr),
            reverse_swap_lock_delay: reverse_swap_lock_delay
                .unwrap_or(current.reverse_swap_lock_delay),
            refund_lock_delay: refund_lock_delay.unwrap_or(current.refund_lock_delay),
            new_relay_eon_lock_delay: new_relay_eon_lock_delay
                .unwrap_or(current.new_relay_eon_lock_delay),
        };

        state.save(storage, api, response_handler)?;
        Ok(state)
    }

    pub fn is_state_valid(storage: &dyn Storage) -> bool {
        // Check if state exists and can be correctly serialised
        matches!(STATE.may_load(storage), Ok(Some(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::msg::ExecuteMsg;
    use crate::evented_storage::events;
    use cosmwasm_std::testing::{mock_dependencies, MockApi};
    use cosmwasm_std::{from_json, to_json_string};

    fn state(api: &MockApi) -> State {
        State {
            bridge_addr: api.addr_make("bridge"),
            reverse_swap_lock_delay: 10,
            refund_lock_delay: 20,
            new_relay_eon_lock_delay: 30,
        }
    }

    fn state_updated(state: &State) -> Vec<(String, Vec<(String, String)>)> {
        let json = to_json_string(state).unwrap();
        vec![(
            "state_updated".to_string(),
            vec![("state".to_string(), json)],
        )]
    }

    #[test]
    fn save_and_load() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();
        let initial = state(&deps.api);

        assert!(!State::is_state_valid(&deps.storage));
        initial
            .save(&mut deps.storage, &deps.api, &mut response_handler)
            .unwrap();

        assert!(State::is_state_valid(&deps.storage));
        assert_eq!(State::load(&deps.storage).unwrap(), initial);
        assert_eq!(events(response_handler), state_updated(&initial));
    }

    #[test]
    fn save_rejects_invalid_bridge_addr() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();
        let invalid = State {
            bridge_addr: Addr::unchecked("invalid"),
            ..state(&deps.api)
        };

        assert!(invalid
            .save(&mut deps.storage, &deps.api, &mut response_handler)
            .is_err());
        assert!(!State::is_state_valid(&deps.storage));
        assert!(events(response_handler).is_empty());
    }

    #[test]
    fn update_changes_only_provided_fields() {
        let mut deps = mock_dependencies();
        let initial = state(&deps.api);
        initial
            .save(
                &mut deps.storage,
                &deps.api,
                &mut ResponseHandler::default(),
            )
            .unwrap();

        let mut response_handler = ResponseHandler::default();
        let new_bridge = deps.api.addr_make("new_bridge");
        let update = StateUpdate {
            bridge_addr: Some(new_bridge.clone()),
            refund_lock_delay: Some(25),
            ..Default::default()
        };
        let updated =
            State::update(&mut deps.storage, &deps.api, &mut response_handler, update).unwrap();

        let expected = State {
            bridge_addr: new_bridge,
            refund_lock_delay: 25,
            ..initial
        };
        assert_eq!(updated, expected);
        assert_eq!(State::load(&deps.storage).unwrap(), expected);
        assert_eq!(events(response_handler), state_updated(&expected));
    }

    #[test]
    fn failed_update_keeps_state() {
        let mut deps = mock_dependencies();
        let initial = state(&deps.api);
        initial
            .save(
                &mut deps.storage,
                &deps.api,
                &mut ResponseHandler::default(),
            )
            .unwrap();

        let mut response_handler = ResponseHandler::default();
        let update = StateUpdate {
            bridge_addr: Some(Addr::unchecked("invalid")),
            refund_lock_delay: Some(25),
            ..Default::default()
        };

        assert!(
            State::update(&mut deps.storage, &deps.api, &mut response_handler, update).is_err()
        );
        assert_eq!(State::load(&deps.storage).unwrap(), initial);
        assert!(events(response_handler).is_empty());
    }

    #[test]
    fn update_requires_initialised_state() {
        let mut deps = mock_dependencies();
        let update = StateUpdate::default();
        assert!(State::update(
            &mut deps.storage,
            &deps.api,
            &mut ResponseHandler::default(),
            update
        )
        .is_err());
    }

    #[test]
    fn update_state_msg_omitted_and_null_fields_are_none() {
        let msg: ExecuteMsg =
            from_json(r#"{"update_state":{"refund_lock_delay":25,"bridge_addr":null}}"#).unwrap();
        assert_eq!(
            msg,
            ExecuteMsg::UpdateState(StateUpdate {
                refund_lock_delay: Some(25),
                ..Default::default()
            })
        );
    }
}
