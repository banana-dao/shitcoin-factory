use crate::{
    msg::{ExecuteMsg, InstantiateMsg, ListingMsg, QueryMsg},
    state::{Config, Field, Metadata},
};
use cosmwasm_std::{coin, Addr, Coin};
use osmosis_test_tube::{Account, Module, OsmosisTestApp, SigningAccount, Wasm};

struct TestEnv {
    app: OsmosisTestApp,
    contract_addr: String,
    admin: SigningAccount,
    users: Vec<SigningAccount>,
}

fn wasm(app: &OsmosisTestApp) -> Wasm<OsmosisTestApp> {
    Wasm::new(app)
}

fn instantiate_contract() -> TestEnv {
    let app = OsmosisTestApp::new();

    let admin = app
        .init_account(&[Coin::new(1_000_000_000_000, "uosmo")])
        .unwrap();

    let users: Vec<SigningAccount> = app
        .init_accounts(
            &[
                Coin::new(1_000_000_000, "uosmo"),
                Coin::new(1_000_000_000, "uatom"),
            ],
            2,
        )
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
                fee: Some(vec![Coin::new(1_000_000, "uosmo")]),
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

fn get_valid_listings() -> Vec<(String, Metadata)> {
    vec![
        (
            "uosmo".to_string(),
            Metadata {
                symbol: "OSMO".to_string(),
                exp: Some(6),
                logo: Some("https://osmosis.zone/logo.png".to_string()),
                chain: Some("osmosis-1".to_string()),
            },
        ),
        (
            "uion".to_string(),
            Metadata {
                symbol: "ION".to_string(),
                exp: Some(6),
                logo: Some("https://osmosis.zone/logo.png".to_string()),
                chain: Some("ion-1".to_string()),
            },
        ),
    ]
}

fn add_listings() -> TestEnv {
    let test_env = instantiate_contract();

    let valid_listing_msg = ListingMsg::Add(get_valid_listings());

    // missing required field
    let invalid_listing_msg = ListingMsg::Add(vec![(
        "uion".to_string(),
        Metadata {
            symbol: "ION".to_string(),
            exp: Some(6),
            logo: Some("https://osmosis.zone/logo.png".to_string()),
            chain: None,
        },
    )]);

    // try to add valid listing without fees
    let res = wasm(&test_env.app).execute(
        &test_env.contract_addr,
        &ExecuteMsg::Listing(valid_listing_msg.clone()),
        &[],
        &test_env.users[0],
    );

    assert!(res.is_err());

    // try to add valid listing with invalid fee
    let res = wasm(&test_env.app).execute(
        &test_env.contract_addr,
        &ExecuteMsg::Listing(valid_listing_msg.clone()),
        &[coin(1_000_000, "uatom")],
        &test_env.users[0],
    );

    assert!(res.is_err());

    // add valid listing with insufficient fee
    let res = wasm(&test_env.app).execute(
        &test_env.contract_addr,
        &ExecuteMsg::Listing(valid_listing_msg.clone()),
        &[coin(1_000_000, "uosmo")],
        &test_env.users[0],
    );

    assert!(res.is_err());

    // add valid listing with correct fee
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Listing(valid_listing_msg.clone()),
            &[coin(2_000_000, "uosmo")],
            &test_env.users[0],
        )
        .unwrap();

    // add invalid listing (with valid fees)
    let res = wasm(&test_env.app).execute(
        &test_env.contract_addr,
        &ExecuteMsg::Listing(invalid_listing_msg.clone()),
        &[coin(1_000_000, "uosmo")],
        &test_env.users[0],
    );

    assert!(res.is_err());

    test_env
}

#[test]
fn test_add_listings() {
    add_listings();
}

#[test]
fn test_remove_listings() {
    let test_env = add_listings();

    // user[0] should be able to delete their own listing
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Listing(ListingMsg::Remove(vec!["uosmo".to_string()])),
            &[],
            &test_env.users[0],
        )
        .unwrap();

    // user[1] should not be able to delete user[0]'s listing
    let res = wasm(&test_env.app).execute(
        &test_env.contract_addr,
        &ExecuteMsg::Listing(ListingMsg::Remove(vec!["uion".to_string()])),
        &[],
        &test_env.users[1],
    );

    assert!(res.is_err());

    // admin will be able to delete any listing
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Listing(ListingMsg::Remove(vec!["uion".to_string()])),
            &[],
            &test_env.admin,
        )
        .unwrap();
}

#[test]
fn test_admin() {
    let test_env = add_listings();

    // add user[1] as an admin
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::UpdateConfig(Config {
                add_permissioned: None,
                remove_permissioned: None,
                required_fields: None,
                fee: None,
                admins: Some(vec![Addr::unchecked(test_env.users[1].address())]),
                // no update to owner
                owner: None,
            }),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // have user[1] remove a listing
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Listing(ListingMsg::Remove(vec!["uosmo".to_string()])),
            &[],
            &test_env.users[1],
        )
        .unwrap();

    // have user[1] add a listing without a fee
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Listing(ListingMsg::Add(vec![(
                "uosmo".to_string(),
                Metadata {
                    symbol: "OSMO".to_string(),
                    exp: Some(6),
                    logo: Some("https://osmosis.zone/logo.png".to_string()),
                    chain: Some("osmosis-1".to_string()),
                },
            )])),
            &[],
            &test_env.users[1],
        )
        .unwrap();

    // remove user[1] as an admin
    let _ = wasm(&test_env.app)
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::UpdateConfig(Config {
                add_permissioned: None,
                remove_permissioned: None,
                required_fields: None,
                fee: None,
                admins: Some(vec![]),
                // no update to owner
                owner: None,
            }),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // make sure they can't delete the listing they added as admin
    let res = wasm(&test_env.app).execute(
        &test_env.contract_addr,
        &ExecuteMsg::Listing(ListingMsg::Remove(vec!["uosmo".to_string()])),
        &[],
        &test_env.users[1],
    );

    assert!(res.is_err());
}

#[test]
fn test_query() {
    let test_env = add_listings();

    let res: Vec<(String, Metadata)>  = wasm(&test_env.app)
        .query(
            &test_env.contract_addr,
            &QueryMsg::Listing(
                crate::msg::ListingQuery::Denom(vec!["uosmo".to_string(), "uion".to_string()]),
            ),
        )
        .unwrap();

    // compare the metadata from the query
    assert_eq!(res[0].1, get_valid_listings()[0].1);
    assert_eq!(res[1].1, get_valid_listings()[1].1);

    // query by symbol
    let res: Vec<(String, Metadata)> = wasm(&test_env.app)
        .query(
            &test_env.contract_addr,
            &QueryMsg::Listing(
                crate::msg::ListingQuery::Symbol(vec!["ION".to_string()]),
            ),
        )
        .unwrap();

    assert_eq!(res[0].1, get_valid_listings()[1].1);

    // query all
    let res: Vec<(String, Metadata)> = wasm(&test_env.app)
        .query(
            &test_env.contract_addr,
            &QueryMsg::Listing(
                crate::msg::ListingQuery::All {
                    start_after: None,
                    limit: None,
                },
            ),
        )
        .unwrap();

    // they are sorted here by denom, so uion comes before uosmo
    assert_eq!(res[0].1, get_valid_listings()[1].1);
    assert_eq!(res[1].1, get_valid_listings()[0].1);
}
