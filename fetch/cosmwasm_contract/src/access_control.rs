use std::str::{FromStr, from_utf8};
use cosmwasm_std::{
    HumanAddr,
    StdError,
    StdResult,
    Storage,
    ReadonlyStorage,
};
use cosmwasm_storage::{ReadonlyPrefixedStorage, PrefixedStorage};


pub static ACCESS_CONTROL_KEY: &[u8] = b"access_control";

pub const OWNER_KEY: &[u8] = b"OWNER_KEY";
pub const ADMIN_ROLE: &str = "ADMIN_ROLE";
pub const DELEGATE_ROLE: &str = "DELEGATE_ROLE";
pub const RELAYER_ROLE: &str = "RELAYER_ROLE";

#[derive(Debug, PartialEq)]
pub enum AccessRole {
    Owner,
    Admin,
    Delegate,
    Relayer,
}

impl AccessRole {
    fn value(&self) -> &str {
        match *self {
            AccessRole::Admin => ADMIN_ROLE,
            AccessRole::Delegate => DELEGATE_ROLE,
            AccessRole::Relayer => RELAYER_ROLE,
            AccessRole::Owner => "",
        }
    }
    
    fn as_bytes(&self) -> &[u8] {
        return self.value().as_bytes()
    }
    
    // FIXME(LR) what happen when FromStr treat is in scope?
    pub fn from_str(s: &str) -> Result<Self, StdError> {
        match s {
            ADMIN_ROLE => Ok(AccessRole::Admin),
            DELEGATE_ROLE => Ok(AccessRole::Delegate),
            RELAYER_ROLE => Ok(AccessRole::Relayer),
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
            DELEGATE_ROLE => Ok(AccessRole::Delegate),
            RELAYER_ROLE => Ok(AccessRole::Relayer),
            _ => Err(StdError::generic_err("Unknow role")),
        }
    }
}

pub fn access_control<S: Storage>(storage: &mut S) -> PrefixedStorage<S> {
    return PrefixedStorage::new(ACCESS_CONTROL_KEY, storage);
}

pub fn access_control_read<S: Storage>(storage: &S) -> ReadonlyPrefixedStorage<S> {
    return ReadonlyPrefixedStorage::new(ACCESS_CONTROL_KEY, storage)
}

pub fn ac_have_role<S: Storage>(storage: &S, addr: &HumanAddr, role: &AccessRole) -> StdResult<bool> {
    let ac_store = access_control_read(storage);
    let addr_roles = ReadonlyPrefixedStorage::new(addr.as_str().as_bytes(), &ac_store);
    match addr_roles.get(role.as_bytes()) {
        Some(_) => Ok(true),
        None => Err(StdError::unauthorized()),
    }
}

pub fn ac_add_role<S: Storage>(storage: &mut S, addr: &HumanAddr, role: &AccessRole) -> StdResult<bool> {
    let already_have_role = ac_have_role(storage, addr, role).unwrap_or(false);
    if already_have_role {
        return Err(StdError::generic_err("Already have role")); 
    }
    let mut ac_store = access_control(storage);
    let mut addr_roles = PrefixedStorage::new(addr.as_str().as_bytes(), &mut ac_store);
    addr_roles.set(role.as_bytes(), b"");

    Ok(true)
}

pub fn ac_revoke_role<S: Storage>(storage: &mut S, addr: &HumanAddr, role: &AccessRole) -> StdResult<bool> {
    let already_have_role = ac_have_role(storage, addr, role).unwrap_or(false);
    if !already_have_role {
        return Err(StdError::generic_err("Already doesn't have role")); 
    }
    let mut ac_store = access_control(storage);
    let mut addr_roles = PrefixedStorage::new(addr.as_str().as_bytes(), &mut ac_store);
    addr_roles.remove(role.as_bytes());

    Ok(true)
}

pub fn ac_get_owner<S: Storage>(storage: &S) -> StdResult<HumanAddr> {
    let ac_store = access_control_read(storage);
    match ac_store.get(OWNER_KEY) {
        Some(addr) => {
            match from_utf8(&addr) {
                Ok(addr_str) => Ok(HumanAddr::from(addr_str)),
                Err(_) =>  Err(StdError::invalid_utf8("Couldn't parse owner address")),
            }
        },
        None => Err(StdError::generic_err("Owner not set")),
    }
}

pub fn ac_set_owner<S: Storage>(storage: &mut S, owner: &HumanAddr) -> StdResult<bool> {
    let have_owner = ac_get_owner(storage);
    match have_owner {
        Ok(_) => Err(StdError::generic_err("Owner cannot be changed")),
        Err(_error) => { // FIXME(LR) mtch on `_error` type
            let mut ac_store = access_control(storage);
            ac_store.set(OWNER_KEY, owner.as_str().as_bytes());
            Ok(true)
        }
    }
}
