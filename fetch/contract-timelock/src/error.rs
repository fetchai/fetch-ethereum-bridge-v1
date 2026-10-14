use cosmwasm_std::StdError;

pub fn global_nonce_overflow_error() -> StdError {
    StdError::msg("Global nonce overflow")
}

pub fn not_implemented_error() -> StdError {
    StdError::msg("Not implemented")
}


