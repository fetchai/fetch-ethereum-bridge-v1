use crate::msg::MigrateMsg;
use crate::state::{State, CONFIG};
use cosmwasm_std::{entry_point, DepsMut, Env, Response, StdResult};

// version info for migration info
pub const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn migrate(mut deps: DepsMut, _env: Env, msg: MigrateMsg) -> StdResult<Response> {
    // Skip checks if forced migration
    if msg.forced != Some(true) {
        let ver = cw2::get_contract_version(deps.storage)?;
        // ensure we are migrating from an allowed contract
        if ver.contract != CONTRACT_NAME {
            return Err(error::different_contract_type_error());
        }
        // note: better to do proper semver compare, but string compare *usually* works
        #[allow(clippy::cmp_owned)]
        if ver.version >= CONTRACT_VERSION.to_string() {
            return Err(error::wrong_contract_version_error(
                &ver.version,
                CONTRACT_VERSION,
            ));
        }
    }

    // Ensure correct contract state
    if !CONFIG.load(deps.storage).is_err() {
        return Err(error::state_storage_error());
    }

    // set the new version
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // do any desired state migrations...

    Ok(Response::new())
}

pub mod error {
    use cosmwasm_std::StdError;

    pub fn different_contract_type_error() -> StdError {
        StdError::generic_err("Can only upgrade from same type.")
    }

    pub fn wrong_contract_version_error(current_version: &str, new_version: &str) -> StdError {
        StdError::generic_err(format!(
            "Cannot upgrade from {} to {}.",
            current_version, new_version
        ))
    }

    pub fn state_storage_error() -> StdError {
        StdError::generic_err("Contract STATE storage not correctly initialised")
    }
}
