use cosmwasm_std::{StdResult, Storage};
use cw_fetch_std::events::ResponseHandler;
use cw_storage_plus::Item;

use crate::evented_storage::evented_item;

pub type NonceType = u64;


// This is global map which keeps the next available nonce
#[evented_item(value = "nonce")]
const GLOBAL_NONCE: Item<NonceType> = Item::new("nonce");

pub struct GlobalNonce {}

impl GlobalNonce {
    pub fn get_nonce(storage: &dyn Storage) -> StdResult<NonceType> {
        Ok(GLOBAL_NONCE.may_load(storage)?.unwrap_or_default())
    }

    pub fn update_nonce(
        storage: &mut dyn Storage,
        response_handler: &mut ResponseHandler,
        nonce: NonceType,
    ) -> StdResult<()> {
        GLOBAL_NONCE.update(storage, response_handler, &nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evented_storage::events;
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn nonce_defaults_to_zero() {
        let deps = mock_dependencies();
        assert_eq!(GlobalNonce::get_nonce(deps.as_ref().storage).unwrap(), 0);
    }

    #[test]
    fn update_nonce_saves_and_emits_event() {
        let mut deps = mock_dependencies();
        let mut response_handler = ResponseHandler::default();

        GlobalNonce::update_nonce(deps.as_mut().storage, &mut response_handler, 42).unwrap();

        assert_eq!(GlobalNonce::get_nonce(deps.as_ref().storage).unwrap(), 42);
        assert_eq!(
            events(response_handler),
            vec![(
                "global_nonce_updated".to_string(),
                vec![("nonce".to_string(), "42".to_string())]
            )]
        );
    }

    #[test]
    fn storage_layout_is_unchanged() {
        let mut deps = mock_dependencies();
        let plain: Item<NonceType> = Item::new("nonce");

        plain.save(deps.as_mut().storage, &7).unwrap();
        assert_eq!(GlobalNonce::get_nonce(deps.as_ref().storage).unwrap(), 7);

        GlobalNonce::update_nonce(deps.as_mut().storage, &mut ResponseHandler::default(), 8)
            .unwrap();
        assert_eq!(plain.load(deps.as_ref().storage).unwrap(), 8);
    }
}
