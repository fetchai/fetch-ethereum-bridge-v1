use cosmwasm_std::{AnyMsg, CosmosMsg, Env, StdError};
use prost::Message;

use crate::msg::Uint128;

#[derive(Clone, PartialEq, Message)]
pub struct Coin {
    #[prost(string, tag = "1")]
    pub denom: String,
    #[prost(string, tag = "2")]
    pub amount: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct MsgMint {
    #[prost(string, tag = "1")]
    pub sender: String,

    #[prost(message, tag = "2")]
    pub amount: Option<Coin>,

    #[prost(string, tag = "3")]
    pub mint_to_address: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct MsgBurn {
    #[prost(string, tag = "1")]
    pub sender: String,

    #[prost(message, tag = "2")]
    pub amount: Option<Coin>,

    #[prost(string, tag = "3")]
    pub burn_from_address: String,
}

pub fn mint_tokens_to_contract(
    env: &Env,
    denom: String,
    amount: Uint128,
) -> Result<CosmosMsg, StdError> {
    let msg = MsgMint {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom,
            amount: amount.to_string(),
        }),
        mint_to_address: "".to_string(),
    };

    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    let cosmos_msg = CosmosMsg::Any(AnyMsg {
        type_url: "/osmosis.tokenfactory.v1beta1.MsgMint".to_string(),
        value: buf.into(),
    });

    Ok(cosmos_msg)
}

pub fn burn_tokens_from_contract(
    env: &Env,
    denom: String,
    amount: Uint128,
) -> Result<CosmosMsg, StdError> {
    let msg = MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(Coin {
            denom,
            amount: amount.to_string(),
        }),
        burn_from_address: "".to_string(),
    };

    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    let cosmos_msg = CosmosMsg::Any(AnyMsg {
        type_url: "/osmosis.tokenfactory.v1beta1.MsgBurn".to_string(),
        value: buf.into(),
    });

    Ok(cosmos_msg)
}
