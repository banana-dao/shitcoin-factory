use crate::msg::{
    ExecuteMsg, InstantiateMsg, MintableResponse, QueryMsg, Receiver, TokenInfoResponse,
};
use cosmwasm_std::{Coin, Uint128};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::bank::v1beta1::QueryBalanceRequest,
        osmosis::tokenfactory::v1beta1::QueryDenomAuthorityMetadataRequest,
    },
    Account, Bank, Module, OsmosisTestApp, SigningAccount, TokenFactory, Wasm,
};

struct TestEnv {
    app: OsmosisTestApp,
    contract_addr: String,
    admin: SigningAccount,
    users: Vec<SigningAccount>,
    denom: String,
}

struct Modules<'a> {
    bank: Bank<'a, OsmosisTestApp>,
    wasm: Wasm<'a, OsmosisTestApp>,
    tf: TokenFactory<'a, OsmosisTestApp>,
}

fn get_modules(test_env: &'_ TestEnv) -> Modules<'_> {
    Modules {
        bank: Bank::new(&test_env.app),
        wasm: Wasm::new(&test_env.app),
        tf: TokenFactory::new(&test_env.app),
    }
}

fn instantiate_contract(initial_supply: Uint128, max_supply: Uint128) -> TestEnv {
    let app = OsmosisTestApp::new();

    let admin = app
        .init_account(&[Coin::new(1_000_000_000_000, "uosmo")])
        .unwrap();

    let users: Vec<SigningAccount> = app.init_accounts(&[], 2).unwrap();

    let mut test_env = TestEnv {
        app,
        contract_addr: String::default(),
        admin,
        users,
        denom: String::default(),
    };

    let modules = get_modules(&test_env);

    let wasm_byte_code = std::fs::read("../../target/wasm32-unknown-unknown/release/factory.wasm")
        .unwrap_or_else(|_| panic!("could not read wasm file - run `cargo wasm` first"));

    let code_id = modules
        .wasm
        .store_code(&wasm_byte_code, None, &test_env.admin)
        .unwrap()
        .data
        .code_id;

    let contract_addr = modules
        .wasm
        .instantiate(
            code_id,
            &InstantiateMsg {
                symbol: "TEST".to_string(),
                initial_supply: Some(initial_supply),
                max_supply: Some(max_supply),
                admin: None,
            },
            Some(&test_env.admin.address()),
            Some("test"),
            &[],
            &test_env.admin,
        )
        .unwrap()
        .data
        .address;

    test_env.contract_addr = contract_addr.clone();
    test_env.denom = format!("factory/{}/tfa/TEST", contract_addr);

    test_env
}

#[test]
fn test_instantiate() {
    // instantiate the contract with 100 initial supply
    let test_env = instantiate_contract(Uint128::from(1_00u128), Uint128::from(1_000u128));

    let balance = get_modules(&test_env)
        .bank
        .query_balance(&QueryBalanceRequest {
            address: test_env.contract_addr.clone(),
            denom: test_env.denom.clone(),
        })
        .unwrap()
        .balance
        .unwrap()
        .amount
        .parse::<u128>()
        .unwrap();

    assert_eq!(balance, 1_00u128);

    // instantiate the contract with 0 initial supply
    let test_env = instantiate_contract(Uint128::from(0u128), Uint128::from(1_000u128));

    let balance = get_modules(&test_env)
        .bank
        .query_balance(&QueryBalanceRequest {
            address: test_env.contract_addr.clone(),
            denom: test_env.denom.clone(),
        })
        .unwrap()
        .balance
        .unwrap()
        .amount
        .parse::<u128>()
        .unwrap();

    assert_eq!(balance, 0u128);
}

#[test]
fn mint_burn() {
    let test_env = instantiate_contract(Uint128::from(1_00u128), Uint128::from(300u128));

    let modules = get_modules(&test_env);

    // mint 100 each tokens to 2 test addresses
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Mint(vec![
                Receiver {
                    address: test_env.users[0].address(),
                    amount: Uint128::from(100u128),
                },
                Receiver {
                    address: test_env.users[1].address(),
                    amount: Uint128::from(100u128),
                },
            ]),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the minted total is now 300
    let res: TokenInfoResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::TokenInfo)
        .unwrap();

    assert_eq!(res.minted, Uint128::from(300u128));

    // try to mint 1 more tokens, should fail
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(1u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // check mintable query

    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(res.cap_reached);

    // burn 100 tokens from the initial mint
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Burn(Uint128::from(100u128)),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the supply is now 200
    let res: TokenInfoResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::TokenInfo)
        .unwrap();

    assert_eq!(res.current_supply, Uint128::from(200u128));

    // recheck mintable query. cap should still be reached as burning does not increase the mintable amount

    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(res.cap_reached);
}

#[test]
fn test_revoke() {
    let test_env = instantiate_contract(Uint128::from(1_00u128), Uint128::from(300u128));

    let modules = get_modules(&test_env);

    // mint 50 each tokens to 2 test addresses

    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Mint(vec![
                Receiver {
                    address: test_env.users[0].address(),
                    amount: Uint128::from(50u128),
                },
                Receiver {
                    address: test_env.users[1].address(),
                    amount: Uint128::from(50u128),
                },
            ]),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the minted total is now 200
    let res: TokenInfoResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::TokenInfo)
        .unwrap();

    assert_eq!(res.minted, Uint128::from(200u128));

    // revoke token admin
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Revoke,
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the mintable query shows that the admin has been revoked and the cap is not reached
    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(res.revoked);
    assert!(!res.cap_reached);

    // try to mint 1 more token, should fail

    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(1u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // try to burn 1 token, should fail
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Burn(Uint128::from(1u128)),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    let new_admin = modules
        .tf
        .query_denom_authority_metadata(&QueryDenomAuthorityMetadataRequest {
            denom: test_env.denom.clone(),
        })
        .unwrap()
        .authority_metadata
        .unwrap()
        .admin;

    // ensure the new admin is the expected null address
    assert_eq!(new_admin, "osmo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqmcn030");
}

#[test]
fn test_cap() {
    let test_env = instantiate_contract(Uint128::from(1_00u128), Uint128::from(300u128));

    let modules = get_modules(&test_env);

    // mint 100 each tokens to 2 test addresses
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Mint(vec![
                Receiver {
                    address: test_env.users[0].address(),
                    amount: Uint128::from(100u128),
                },
                Receiver {
                    address: test_env.users[1].address(),
                    amount: Uint128::from(100u128),
                },
            ]),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the minted total is now 300
    let res: TokenInfoResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::TokenInfo)
        .unwrap();

    assert_eq!(res.minted, Uint128::from(300u128));

    // check that the cap is reached
    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(res.cap_reached);

    // try to mint 1 more token, should fail
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(1u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // burn 100 tokens from the initial mint
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Burn(Uint128::from(100u128)),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // make sure we still can't mint
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(1u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // recheck the query
    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(res.cap_reached);

    // increase the cap to 400
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::UpdateSupply(Uint128::from(400u128)),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // try to mint 101 tokens, should fail
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(101u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // try to mint 100 tokens, should pass
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Mint(vec![Receiver {
                address: test_env.users[0].address(),
                amount: Uint128::from(100u128),
            }]),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the cap is reached again

    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(res.cap_reached);

    // try to reduce the cap to 300, should fail
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::UpdateSupply(Uint128::from(300u128)),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // change cap to 0, should pass
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::UpdateSupply(Uint128::from(0u128)),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // check that the cap is not reached
    let res: MintableResponse = modules
        .wasm
        .query(&test_env.contract_addr, &QueryMsg::Mintable)
        .unwrap();

    assert!(!res.cap_reached);

    // try to mint 100 tokens, should pass
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::Mint(vec![Receiver {
                address: test_env.users[0].address(),
                amount: Uint128::from(100u128),
            }]),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // change cap down to 500, should pass
    let _ = modules
        .wasm
        .execute(
            &test_env.contract_addr,
            &ExecuteMsg::UpdateSupply(Uint128::from(500u128)),
            &[],
            &test_env.admin,
        )
        .unwrap();

    // make sure we can't mint 1 more token
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(1u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());
}

#[test]
fn test_invalid_messages() {
    let test_env = instantiate_contract(Uint128::from(1_00u128), Uint128::from(300u128));

    let modules = get_modules(&test_env);

    // mint to invalid address
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: "invalid_address".to_string(),
            amount: Uint128::from(100u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // mint invalid amount
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Mint(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(0u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // send to invalid address
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Send(vec![Receiver {
            address: "invalid_address".to_string(),
            amount: Uint128::from(100u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());

    // send invalid amount
    let res = modules.wasm.execute(
        &test_env.contract_addr,
        &ExecuteMsg::Send(vec![Receiver {
            address: test_env.users[0].address(),
            amount: Uint128::from(0u128),
        }]),
        &[],
        &test_env.admin,
    );

    assert!(res.is_err());
}
