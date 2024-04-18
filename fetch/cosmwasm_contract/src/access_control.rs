use cosmwasm_std::{Addr, Order, StdError, StdResult, Storage};
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

pub fn ac_have_role(storage: &dyn Storage, addr: &Addr, role: &AccessRole) -> StdResult<bool> {
    let addr_roles =
        ReadonlyPrefixedStorage::multilevel(storage, &[ACCESS_CONTROL_KEY, addr.as_bytes()]);
    let role = addr_roles.get(role.as_bytes());
    match role {
        Some(x) if x == [1] => Ok(true),
        _ => Ok(false),
    }
}

pub fn ac_add_role(storage: &mut dyn Storage, addr: &Addr, role: &AccessRole) -> StdResult<bool> {
    let already_have_role = ac_have_role(storage, addr, role).unwrap_or(false);
    if already_have_role {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_ALREADY_HAS_ROLE));
    }
    let mut addr_roles =
        PrefixedStorage::multilevel(storage, &[ACCESS_CONTROL_KEY, addr.as_bytes()]);
    addr_roles.set(role.as_bytes(), &[1]);

    Ok(true)
}

pub fn ac_revoke_role(
    storage: &mut dyn Storage,
    addr: &Addr,
    role: &AccessRole,
) -> StdResult<bool> {
    let already_have_role = ac_have_role(storage, addr, role).unwrap_or(false);
    if !already_have_role {
        return Err(StdError::generic_err(ERR_ACCESS_CONTROL_DOESNT_HAVE_ROLE));
    }
    let mut addr_roles =
        PrefixedStorage::multilevel(storage, &[ACCESS_CONTROL_KEY, addr.as_bytes()]);
    addr_roles.remove(role.as_bytes());
    Ok(true)
}

pub fn unsafe_remove_all_roles(storage: &mut dyn Storage) {
    let mut roles_storage = PrefixedStorage::new(storage, ACCESS_CONTROL_KEY);
    let res: Vec<_> = roles_storage.range(None, None, Order::Ascending).collect();

    // Because address is stored as first key it is impossible to clear only specific roles without manually filtering it
    for val in res {
        roles_storage.remove(&val.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn test_unsafe_remove_all_roles() {
        let mut deps = mock_dependencies();

        let addresses = vec!["a", "b", "c", "d"];
        let roles = vec![
            AccessRole::Admin,
            AccessRole::Approver,
            AccessRole::Monitor,
            AccessRole::Relayer,
        ];

        // Add every role and address combination
        for address in &addresses {
            for role in &roles {
                ac_add_role(
                    deps.as_mut().storage,
                    &Addr::unchecked(address.to_string()),
                    role,
                )
                .unwrap();
            }
        }

        // Every role and address combination is present
        for address in &addresses {
            for role in &roles {
                assert!(ac_have_role(
                    deps.as_ref().storage,
                    &Addr::unchecked(address.to_string()),
                    role
                )
                .unwrap());
            }
        }

        // Remove all roles
        unsafe_remove_all_roles(deps.as_mut().storage);

        // Every role and address combination was removed
        for address in &addresses {
            for role in &roles {
                assert!(!ac_have_role(
                    deps.as_ref().storage,
                    &Addr::unchecked(address.to_string()),
                    role
                )
                .unwrap());
            }
        }
    }
}
