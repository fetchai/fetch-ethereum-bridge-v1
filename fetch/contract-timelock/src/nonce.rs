use cosmwasm_std::{StdResult, Storage};
use cw_fetch_std::events::{IntoEvent, ResponseHandler};
use cw_storage_plus::Item;

pub type NonceType = u64;


pub struct GlobalNonceUpdatedEvent {
    pub nonce: NonceType,
}

impl IntoEvent for GlobalNonceUpdatedEvent {
    fn event_name(&self) -> &str {
        "global_nonce_updated"
    }

    fn event_attributes(&self) -> Vec<(String, String)> {
        vec![("nonce".to_string(), self.nonce.to_string())]
    }
}


// This is global map which keeps the next available nonce
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
        GLOBAL_NONCE.save(storage, &nonce)?;

        // Add update event
        response_handler.add_event(GlobalNonceUpdatedEvent { nonce });

        Ok(())
    }
}
