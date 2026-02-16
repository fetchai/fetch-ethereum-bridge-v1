use cosmwasm_std::{Addr, StdError, StdResult, Storage};
use std::str::FromStr;
use cosmwasm_std::storage_keys::to_length_prefixed_nested;
use crate::error::{ERR_ACCESS_CONTROL_ALREADY_HAS_ROLE, ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE};

pub static ACCESS_CONTROL_KEY: &[u8] = b"access_control";

pub const ADMIN_ROLE: &str = "ADMIN_ROLE";
pub const APPROVER_ROLE: &str = "APPROVER_ROLE";
pub const MONITOR_ROLE: &str = "MONITOR_ROLE";
pub const RELAYER_ROLE: &str = "RELAYER_ROLE";

#[derive(Debug, PartialEq)]
pub enum AccessRole {
    Admin,
    Relayer,
    Approver,
    Monitor,
}

impl AccessRole {
    fn value(&self) -> &str {
        match *self {
            AccessRole::Admin => ADMIN_ROLE,
            AccessRole::Relayer => RELAYER_ROLE,
            AccessRole::Approver => APPROVER_ROLE,
            AccessRole::Monitor => MONITOR_ROLE,
        }
    }
    fn as_bytes(&self) -> &[u8] {
        return self.value().as_bytes();
    }
    // FIXME(LR) what happen when FromStr trait is in scope?
    pub fn from_str(s: &str) -> Result<Self, StdError> {
        #![allow(clippy::should_implement_trait)]
        match s {
            ADMIN_ROLE => Ok(AccessRole::Admin),
            RELAYER_ROLE => Ok(AccessRole::Relayer),
            APPROVER_ROLE => Ok(AccessRole::Approver),
            MONITOR_ROLE => Ok(AccessRole::Monitor),
            _ => Err(StdError::generic_err("Unknow role")),
        }
    }
}

// FIXME(LR) cannot use fn if Trait not in scope, rather annoying
impl FromStr for AccessRole {
    type Err = StdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ADMIN_ROLE => Ok(AccessRole::Admin),
            RELAYER_ROLE => Ok(AccessRole::Relayer),
            APPROVER_ROLE => Ok(AccessRole::Approver),
            MONITOR_ROLE => Ok(AccessRole::Monitor),
            _ => Err(StdError::generic_err("Unknow role")),
        }
    }
}

/// Builds the exact same storage key as:
/// PrefixedStorage::multilevel(storage, &[ACCESS_CONTROL_KEY, addr.as_bytes()]).<op>(role_bytes)
fn ac_storage_key(addr: &Addr, role: &AccessRole) -> Vec<u8> {
    let mut key = to_length_prefixed_nested(&[ACCESS_CONTROL_KEY, addr.as_bytes()]);
    key.extend_from_slice(role.as_bytes());
    key
}


pub fn ac_have_role(storage: &dyn Storage, addr: &Addr, role: &AccessRole) -> StdResult<bool> {
    let key = ac_storage_key(addr, role);
    Ok(matches!(storage.get(&key).as_deref(), Some([1])))
}

pub fn ac_add_role(storage: &mut dyn Storage, addr: &Addr, role: &AccessRole) -> StdResult<bool> {
    if ac_have_role(storage, addr, role).unwrap_or(false) {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_ALREADY_HAS_ROLE));
    }

    let key = ac_storage_key(addr, role);
    storage.set(&key, &[1]);
    Ok(true)
}


pub fn ac_revoke_role(
    storage: &mut dyn Storage,
    addr: &Addr,
    role: &AccessRole,
) -> StdResult<bool> {
    if !ac_have_role(storage, addr, role).unwrap_or(false) {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE));
    }

    let key = ac_storage_key(addr, role);
    storage.remove(&key);
    Ok(true)
}