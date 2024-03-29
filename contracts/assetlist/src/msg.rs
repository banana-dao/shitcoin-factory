use crate::state::{Config, Metadata};
use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg(pub Config);

#[cw_serde]
pub enum ExecuteMsg {
    Listing(ListingMsg),
    UpdateConfig(Config),
}

#[cw_serde]
pub enum ListingMsg {
    // Adds listings to the assetlist
    Add(Vec<(String, Metadata)>),
    // Update existing listings
    Update(Vec<(String, Metadata)>),
    // Removes listings from the assetlist by denom. Must be done by the listing creator or an admin
    Remove(Vec<String>),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Vec<(String, Metadata)>)]
    Listing(ListingQuery),
    #[returns(Config)]
    Config,
}

#[cw_serde]
pub enum ListingQuery {
    // Returns metadata for a list of denoms
    Denom(Vec<String>),
    // Returns metadata for a list of symbols
    Symbol(Vec<String>),
    // Returns a paginated list of all listings
    All {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}
