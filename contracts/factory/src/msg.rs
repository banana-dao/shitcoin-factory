use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub symbol: String,
    pub initial_supply: Option<Uint128>,
    pub max_supply: Option<Uint128>,
    pub admin: Option<Addr>,
}

#[cw_serde]
pub enum ExecuteMsg {
    // Mints tokens to a recipient account(s)
    Mint(Vec<Receiver>),
    // Transfers tokens from the contract to a recipient account(s)
    Send(Vec<Receiver>),
    // Burns tokens held by the contract
    Burn(Uint128),
    // Updates the max mintable supply of the token
    UpdateSupply(Uint128),
    // Transfers token admin to a null address, preventing future minting
    Revoke,
}

#[cw_serde]
pub struct Receiver {
    pub address: String,
    pub amount: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the token denom and supply information
    #[returns(TokenInfoResponse)]
    TokenInfo,
    /// Returns the token mintable status
    #[returns(MintableResponse)]
    Mintable,
}

#[cw_serde]
pub struct TokenInfoResponse {
    pub symbol: String,
    pub denom: String,
    pub current_supply: Uint128,
    pub max_supply: Uint128,
    pub minted: Uint128,
    pub burned: Uint128,
}

#[cw_serde]
pub struct MintableResponse {
    pub cap_reached: bool,
    pub revoked: bool,
}
