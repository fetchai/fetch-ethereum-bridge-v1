use cosmwasm_std::StdError;

pub fn global_nonce_overflow_error() -> StdError {
    StdError::generic_err("Global nonce overflow")
}


