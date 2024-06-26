use assetlist::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_schema::write_api;

//run cargo schema to generate
fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
