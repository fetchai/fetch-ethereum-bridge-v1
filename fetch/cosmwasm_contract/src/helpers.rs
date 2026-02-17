use cosmwasm_std::{attr, AnyMsg, Api, CanonicalAddr, CosmosMsg, Response, StdError};
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
    #[prost(string, tag="1")]
    pub sender: String,

    #[prost(message, tag="2")]
    pub amount: Option<Coin>,

    #[prost(string, tag="3")]
    pub mint_to_address: String,
}



pub fn mint_tokens_from_contract(
    api: &dyn Api,
    contract_addr: &CanonicalAddr, // who mints (usually contract)
    to_address: &CanonicalAddr,
    denom: String,
    amount: Uint128,
    action: &str,
) -> Result<Response, StdError> {
    let sender = api.addr_humanize(contract_addr)?;
    let recipient = api.addr_humanize(to_address)?;

    let to_human = api.addr_humanize(to_address)?;
    let attrs = vec![attr("action", action), attr("to", to_human.as_str())];

    // build protobuf message
    let msg = MsgMint {
        sender: sender.to_string(),
        amount: Some(Coin {
            denom,
            amount: amount.to_string(), // protobuf expects string
        }),
        mint_to_address: recipient.to_string(),
    };

    // encode protobuf
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| StdError::generic_err(e.to_string()))?;

    // wrap into Cosmos Any
    let cosmos_msg = CosmosMsg::Any(AnyMsg {
        type_url: "/osmosis.tokenfactory.v1beta1.MsgMint".to_string(),
        value: buf.into(),
    });

    Ok(Response::new()
        .add_attributes(attrs)
        .add_message(cosmos_msg))
}
