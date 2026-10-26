use crate::queue::NonceType;
use cosmwasm_std::StdError;

pub fn nonce_overflow_error() -> StdError {
    StdError::msg("Nonce overflow")
}

pub fn execution_time_overflow_error() -> StdError {
    StdError::msg("Execution time overflow")
}

pub fn message_not_found_error(nonce: NonceType) -> StdError {
    StdError::msg(format!("Message with nonce {nonce} not found"))
}

pub fn message_not_pending_error(nonce: NonceType) -> StdError {
    StdError::msg(format!("Message with nonce {nonce} is not pending"))
}

pub fn message_not_next_error(nonce: NonceType) -> StdError {
    StdError::msg(format!("Message with nonce {nonce} is not next to execute"))
}

pub fn message_not_mature_error(nonce: NonceType, execution_time: u64) -> StdError {
    StdError::msg(format!(
        "Message with nonce {nonce} can not be executed before {execution_time}"
    ))
}

pub fn too_many_pending_messages_error(max_pending: u64) -> StdError {
    StdError::msg(format!(
        "Can not submit message, limit of {max_pending} pending messages reached"
    ))
}

pub fn message_already_submitted_error(nonce: NonceType) -> StdError {
    StdError::msg(format!(
        "Message with the same id is already pending or executed under nonce {nonce}"
    ))
}
