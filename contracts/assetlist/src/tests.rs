use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, Field},
};
use cosmwasm_std::{Addr, Coin};
use osmosis_test_tube::{Account, Module, OsmosisTestApp, SigningAccount, Wasm};

struct TestEnv {
    app: OsmosisTestApp,
    contract_addr: String,
    admin: SigningAccount,
    users: Vec<SigningAccount>,
}

fn instantiate_contract() -> TestEnv {
    let app = OsmosisTestApp::new();

    let admin = app
        .init_account(&[Coin::new(1_000_000_000_000, "uosmo")])
        .unwrap();

    let users: Vec<SigningAccount> = app
        .init_accounts(&[Coin::new(1_000_000_000, "uosmo")], 2)
        .unwrap();

    let mut test_env = TestEnv {
        app,
        contract_addr: String::default(),
        admin,
        users,
    };

    let wasm_byte_code =
        std::fs::read("../../target/wasm32-unknown-unknown/release/assetlist.wasm")
            .unwrap_or_else(|_| panic!("could not read wasm file - run `cargo wasm` first"));

    let wasm = Wasm::new(&test_env.app);

    let code_id = wasm
        .store_code(&wasm_byte_code, None, &test_env.admin)
        .unwrap()
        .data
        .code_id;

    let contract_addr = wasm
        .instantiate(
            code_id,
            &InstantiateMsg(Config {
                add_permissioned: None,
                remove_permissioned: None,
                required_fields: vec![Field::Exp, Field::Logo, Field::Chain].into(),
                admins: None,
                owner: None,
            }),
            Some(&test_env.admin.address()),
            Some("test"),
            &[],
            &test_env.admin,
        )
        .unwrap()
        .data
        .address;

    test_env.contract_addr = contract_addr.clone();

    test_env
}

#[test]
fn test_instantiate() {
    let _ = instantiate_contract();
}
