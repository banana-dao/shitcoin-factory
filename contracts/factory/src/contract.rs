use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, Receiver};
use crate::state::{ADMIN, DENOM, MAX_SUPPLY, SYMBOL, TOTAL_MINTED};
use bech32::{decode, encode};
use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use osmosis_std::types::cosmos::{bank::v1beta1::BankQuerier, base::v1beta1::Coin};
use osmosis_std::types::osmosis::tokenfactory::v1beta1::{
    MsgBurn, MsgChangeAdmin, MsgCreateDenom, MsgMint, TokenfactoryQuerier,
};

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // use the sender as the admin, unless one is provided. validate the admin address
    let admin = msg.admin.unwrap_or(info.sender.clone());
    deps.api.addr_validate(admin.as_str())?;

    let initial_supply = msg.initial_supply.unwrap_or(Uint128::zero());
    let max_supply = msg.max_supply.unwrap_or(Uint128::zero());

    // sanity check on supply amounts. max must be >= initial, unless it is 0 (for uncapped)
    if initial_supply > max_supply && !max_supply.is_zero() {
        return Err(ContractError::SupplyCap);
    }

    // tokenfactory denoms are in the format "factory/{creator_address}/{subdenom}".
    // we add the custom subspace '/tfa/' to identify it as created by this contract
    let subdenom = format!("tfa/{}", msg.symbol);
    let denom = format!(
        "factory/{}/{}",
        env.contract.address.clone().into_string(),
        subdenom
    );

    ADMIN.save(deps.storage, &admin)?;
    DENOM.save(deps.storage, &denom)?;
    SYMBOL.save(deps.storage, &msg.symbol)?;
    MAX_SUPPLY.save(deps.storage, &max_supply.u128())?;
    TOTAL_MINTED.save(deps.storage, &initial_supply.u128())?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let create_msg: CosmosMsg = MsgCreateDenom {
        sender: env.contract.address.clone().into_string(),
        subdenom,
    }
    .into();

    // if initial supply is zero, we are done
    if initial_supply.is_zero() {
        return Ok(Response::new()
            .add_message(create_msg)
            .add_attribute("action", "instantiate")
            .add_attribute("action", "create_denom"));
    };

    // otherwise mint the initial supply to the contract address
    let mint_msg: CosmosMsg = MsgMint {
        sender: env.contract.address.clone().into_string(),
        amount: Some(Coin {
            denom,
            amount: initial_supply.to_string(),
        }),
        mint_to_address: env.contract.address.into_string(),
    }
    .into();

    Ok(Response::new()
        .add_message(create_msg)
        .add_message(mint_msg)
        .add_attribute("action", "instantiate")
        .add_attribute("action", "create_denom")
        .add_attribute("initial_mint", initial_supply.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    // only admin can execute
    if info.sender != ADMIN.load(deps.storage)? {
        return Err(ContractError::Unauthorized);
    }
    let contract = env.contract.address;

    match msg {
        ExecuteMsg::Mint(receivers) => execute_mint(deps, &contract, &receivers),
        ExecuteMsg::Burn(amount) => execute_burn(deps, contract, &amount),
        ExecuteMsg::Send(receivers) => execute_transfer(deps, &receivers),
        ExecuteMsg::UpdateSupply(new_max) => execute_update_supply(deps, &new_max),
        ExecuteMsg::Revoke => execute_revoke(deps, contract),
    }
}

fn execute_mint(
    deps: DepsMut,
    contract: &Addr,
    receivers: &[Receiver],
) -> Result<Response, ContractError> {
    let denom = DENOM.load(deps.storage)?;
    let max_supply = MAX_SUPPLY.load(deps.storage)?;
    let total_minted = TOTAL_MINTED.load(deps.storage)?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attributes: Vec<cosmwasm_std::Attribute> = vec![];
    let mut total_to_mint: u128 = 0;

    for (i, receiver) in receivers.iter().enumerate() {
        let amount = receiver.amount;
        let address = &receiver.address;
        if amount.is_zero() || deps.api.addr_validate(address.as_str()).is_err() {
            return Err(ContractError::MintInvalid(i));
        }
        total_to_mint += amount.u128();
        let msg: CosmosMsg = MsgMint {
            sender: contract.clone().to_string(),
            amount: Some(Coin {
                denom: denom.clone(),
                amount: amount.to_string(),
            }),
            mint_to_address: address.clone(),
        }
        .into();
        msgs.push(msg);
        attributes.push(cosmwasm_std::Attribute {
            key: String::from("recipient"),
            value: address.to_string(),
        });
        attributes.push(cosmwasm_std::Attribute {
            key: String::from("amount"),
            value: amount.to_string(),
        });
    }

    // check if attempting to mint more than max supply, unless max supply is 0
    if max_supply < total_to_mint + total_minted && max_supply != 0 {
        return Err(ContractError::SupplyCap);
    }

    // update the total minted amount
    TOTAL_MINTED.save(deps.storage, &(total_to_mint + total_minted))?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "mint")
        .add_attributes(attributes)
        .add_attribute("total_minted", total_minted.to_string()))
}

fn execute_burn(
    deps: DepsMut,
    contract: Addr,
    burn_amount: &Uint128,
) -> Result<Response, ContractError> {
    let msg: CosmosMsg = MsgBurn {
        sender: contract.clone().to_string(),
        amount: Some(Coin {
            denom: DENOM.load(deps.storage)?,
            amount: burn_amount.to_string(),
        }),
        burn_from_address: contract.into_string(),
    }
    .into();

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "burn")
        .add_attribute("amount", burn_amount.to_string()))
}

fn execute_transfer(deps: DepsMut, messages: &[Receiver]) -> Result<Response, ContractError> {
    let denom = DENOM.load(deps.storage)?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attributes: Vec<cosmwasm_std::Attribute> = vec![];

    let mut total_to_transfer = Uint128::zero();

    for (i, msg) in messages.iter().enumerate() {
        let amount = msg.amount;
        let address = &msg.address;

        if amount.is_zero() || deps.api.addr_validate(address.as_str()).is_err() {
            return Err(ContractError::TransferInvalid(i));
        }
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: address.clone(),
            amount: vec![cosmwasm_std::Coin {
                denom: denom.clone(),
                amount,
            }],
        }));
        attributes.push(cosmwasm_std::Attribute {
            key: String::from("recipient"),
            value: address.to_string(),
        });
        attributes.push(cosmwasm_std::Attribute {
            key: String::from("amount"),
            value: amount.to_string(),
        });
        total_to_transfer += amount;
    }
    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "transfer")
        .add_attributes(attributes)
        .add_attribute("total_transferred", total_to_transfer.to_string()))
}

fn execute_revoke(deps: DepsMut, contract: Addr) -> Result<Response, ContractError> {
    let sender = contract.into_string();
    let denom = DENOM.load(deps.storage)?;

    // use the contract address to deduce a burn address for whatever chain this contract is on
    let (hrp, _) = decode(&sender).unwrap();
    let null_address = encode::<bech32::Bech32>(hrp, &[0u8; 20]).unwrap();
    let msg: CosmosMsg = MsgChangeAdmin {
        sender,
        denom,
        new_admin: null_address,
    }
    .into();
    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "burn_minter"))
}

fn execute_update_supply(deps: DepsMut, new_max: &Uint128) -> Result<Response, ContractError> {
    let total_minted = TOTAL_MINTED.load(deps.storage)?;

    // make sure that the max supply is not reduced below the total minted amount, unless the new max is 0 (uncapped)
    if new_max.u128() < total_minted && !new_max.is_zero() {
        return Err(ContractError::CurrentSupply);
    }

    MAX_SUPPLY.save(deps.storage, &new_max.u128())?;
    Ok(Response::new().add_attribute("action", "update_metadata"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::TokenInfo => to_json_binary(&query_info(deps)?),
        QueryMsg::Mintable => to_json_binary(&query_mintable(deps, env)?),
    }
}

fn query_info(deps: Deps) -> StdResult<crate::msg::TokenInfoResponse> {
    let symbol = SYMBOL.load(deps.storage)?;
    let denom = DENOM.load(deps.storage)?;
    let current_supply = query_bank_supply(deps, denom.clone());
    let max_supply = MAX_SUPPLY.load(deps.storage)?;
    let minted = TOTAL_MINTED.load(deps.storage)?;
    // this is redundant. remove it?
    let burned = minted - current_supply;

    Ok(crate::msg::TokenInfoResponse {
        symbol,
        denom,
        current_supply: current_supply.into(),
        max_supply: max_supply.into(),
        minted: minted.into(),
        burned: burned.into(),
    })
}

fn query_mintable(deps: Deps, env: Env) -> StdResult<crate::msg::MintableResponse> {
    let denom = DENOM.load(deps.storage)?;
    let max_supply = MAX_SUPPLY.load(deps.storage)?;
    let total_minted = TOTAL_MINTED.load(deps.storage)?;

    let mut cap_reached = false;
    let mut revoked = false;

    // check if the max supply has been reached
    if max_supply != 0 && total_minted == max_supply {
        cap_reached = true;
    }

    // check if the admin has been revoked
    let admin = TokenfactoryQuerier::new(&deps.querier)
        .denom_authority_metadata(denom)?
        .authority_metadata
        .unwrap()
        .admin;
    if admin != env.contract.address.into_string() {
        revoked = true;
    }

    Ok(crate::msg::MintableResponse {
        cap_reached,
        revoked,
    })
}

fn query_bank_supply(deps: Deps, denom: String) -> u128 {
    return BankQuerier::new(&deps.querier)
        .supply_of(denom)
        .unwrap_or_default()
        .amount
        .unwrap_or_default()
        .amount
        .parse::<u128>()
        .unwrap_or_default();
}
