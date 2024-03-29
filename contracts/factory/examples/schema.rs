use cosmwasm_schema::write_api;
use factory::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

//run cargo schema to generate
fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
