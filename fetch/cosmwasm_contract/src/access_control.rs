use cosmwasm_std::{HumanAddr, ReadonlyStorage, StdError, StdResult, Storage};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use std::str::FromStr;

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
    // FIXME(LR) what happen when FromStr treat is in scope?
    pub fn from_str(s: &str) -> Result<Self, StdError> {
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

pub fn access_control<S: Storage>(storage: &mut S) -> PrefixedStorage<S> {
    return PrefixedStorage::new(storage, ACCESS_CONTROL_KEY);
}

pub fn access_control_read<S: Storage>(storage: &S) -> ReadonlyPrefixedStorage<S> {
    return ReadonlyPrefixedStorage::new(storage, ACCESS_CONTROL_KEY);
}

pub fn ac_have_role<S: Storage>(
    storage: &S,
    addr: &HumanAddr,
    role: &AccessRole,
) -> StdResult<bool> {
    let ac_store = access_control_read(storage);
    let addr_roles = ReadonlyPrefixedStorage::new(&ac_store, addr.as_str().as_bytes());
    match addr_roles.get(role.as_bytes()) {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub fn ac_add_role<S: Storage>(
    storage: &mut S,
    addr: &HumanAddr,
    role: &AccessRole,
) -> StdResult<bool> {
    let already_have_role = ac_have_role(storage, addr, role).unwrap_or(false);
    if already_have_role {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_ALREADY_HAS_ROLE));
    }
    let mut ac_store = access_control(storage);
    let mut addr_roles = PrefixedStorage::new(&mut ac_store, addr.as_str().as_bytes());
    addr_roles.set(role.as_bytes(), b"");

    Ok(true)
}

pub fn ac_revoke_role<S: Storage>(
    storage: &mut S,
    addr: &HumanAddr,
    role: &AccessRole,
) -> StdResult<bool> {
    let already_have_role = ac_have_role(storage, addr, role).unwrap_or(false);
    if !already_have_role {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE));
    }
    let mut ac_store = access_control(storage);
    let mut addr_roles = PrefixedStorage::new(&mut ac_store, addr.as_str().as_bytes());
    addr_roles.remove(role.as_bytes());

    Ok(true)
}
