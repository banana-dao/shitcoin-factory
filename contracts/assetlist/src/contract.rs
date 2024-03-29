use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, ListingMsg, ListingQuery, MigrateMsg, QueryMsg};
use crate::state::Listing;
use crate::state::{
    Config, Field,
    Field::{Chain, Exp, Logo},
    Metadata, CONFIG, DENOM_MAP, SYMBOL_MAP,
};
use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdError, StdResult, Uint128,
};
use cw2::{get_contract_version, set_contract_version};
use cw_storage_plus::Bound;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Pagination for queries
const MAX_PAGE_LIMIT: u32 = 250;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut admins = msg.0.admins.unwrap_or_default();

    for address in &admins {
        deps.api.addr_validate(address.as_str())?;
    }
    admins.push(info.sender.clone());

    CONFIG.save(
        deps.storage,
        &Config {
            add_permissioned: msg.0.add_permissioned,
            remove_permissioned: msg.0.remove_permissioned,
            required_fields: msg.0.required_fields,
            fee: msg.0.fee,
            admins: Some(admins),
            owner: Some(msg.0.owner.unwrap_or(info.sender)),
        },
    )?;

    Ok(Response::new().add_attribute("action", "assetlist_instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let admin = config
        .admins
        .clone()
        .unwrap_or_default()
        .contains(&info.sender);

    match msg {
        ExecuteMsg::Listing(msg) => match msg {
            ListingMsg::Add(listings) => execute_add_listings(
                deps,
                &info.sender,
                &info.funds,
                config.fee,
                admin,
                config.add_permissioned.unwrap_or_default(),
                &config.required_fields.unwrap_or_default(),
                listings,
            ),
            ListingMsg::Update(updates) => execute_update_listings(
                deps,
                &info.sender,
                admin,
                config.add_permissioned.unwrap_or_default(),
                &config.required_fields.unwrap_or_default(),
                updates,
            ),
            ListingMsg::Remove(denoms) => execute_remove_listings(
                deps,
                &info.sender,
                admin,
                config.remove_permissioned.unwrap_or_default(),
                denoms,
            ),
        },
        ExecuteMsg::UpdateConfig(mut new_config) => {
            execute_update_config(deps, &info.sender, config, &mut new_config)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_add_listings(
    deps: DepsMut,
    sender: &Addr,
    funds: &[Coin],
    fee: Option<Vec<Coin>>,
    admin: bool,
    permissioned: bool,
    required_fields: &[Field],
    new_listings: Vec<(String, Metadata)>,
) -> Result<Response, ContractError> {
    if permissioned && !admin {
        return Err(ContractError::AddPermissioned);
    }

    // validate that the sender has paid the fee if required. Admins are exempt
    if !admin && fee.is_some() {
        if funds.is_empty() {
            return Err(ContractError::MissingFee);
        }

        // for simplicity, although we can accept multiple fee coins we will only allow one to be used per tx
        if funds.len() > 1 {
            return Err(ContractError::MultipleFees);
        }

        if let Some(fee_token) = fee
            .unwrap()
            .iter()
            .find(|coin| coin.denom == funds[0].denom)
        {
            if fee_token.amount * Uint128::from(new_listings.len() as u128) > funds[0].amount {
                return Err(ContractError::InsufficientFee);
            }
        } else {
            return Err(ContractError::InvalidFee);
        }
    }

    // validate new listings
    for listing in new_listings {
        let denom = listing.0;
        let metadata = listing.1.clone();

        // we don't want to allow duplicate listings by denom or symbol as they will be used as keys
        if DENOM_MAP.has(deps.storage, denom.clone()) {
            return Err(ContractError::DuplicateListing(denom));
        }

        if SYMBOL_MAP.has(deps.storage, metadata.symbol.clone()) {
            return Err(ContractError::DuplicateListing(metadata.symbol));
        }

        check_required_fields(required_fields, &metadata)?;

        DENOM_MAP.save(
            deps.storage,
            denom.clone(),
            &Listing {
                author: if admin {
                    None
                } else {
                    Some(sender.to_string())
                },
                metadata: metadata.clone(),
            },
        )?;
        SYMBOL_MAP.save(deps.storage, metadata.symbol, &denom)?;
    }

    Ok(Response::new().add_attribute("action", "assetlist_add_listings"))
}

fn execute_update_listings(
    deps: DepsMut,
    sender: &Addr,
    admin: bool,
    permissioned: bool,
    required_fields: &[Field],
    updated_listings: Vec<(String, Metadata)>,
) -> Result<Response, ContractError> {
    // remove must be permissionless in order for creators to edit their own listings
    if permissioned && !admin {
        return Err(ContractError::RemovePermissioned);
    }

    // validate updated listings
    for update in updated_listings {
        // make sure the denom is listed
        let denom = update.0.clone();
        let metadata = update.1.clone();

        let Ok(current_listing) = DENOM_MAP.load(deps.storage, denom.clone()) else {
            return Err(ContractError::ListingNotFound(denom));
        };

        // make sure the sender is the creator of the listing or an admin
        if current_listing.author.unwrap_or_default() != *sender && !admin {
            return Err(ContractError::Unauthorized);
        }

        // make sure the new symbol is not already in use for a different denom
        if current_listing.metadata.symbol != metadata.symbol
            && SYMBOL_MAP.has(deps.storage, metadata.symbol.clone())
        {
            return Err(ContractError::DuplicateListing(metadata.symbol));
        }

        check_required_fields(required_fields, &metadata)?;

        DENOM_MAP.save(
            deps.storage,
            denom.clone(),
            &Listing {
                author: if admin {
                    None
                } else {
                    Some(sender.to_string())
                },
                metadata: metadata.clone(),
            },
        )?;

        SYMBOL_MAP.save(deps.storage, metadata.symbol, &denom)?;
    }

    Ok(Response::new().add_attribute("action", "assetlist_update_listings"))
}

fn execute_remove_listings(
    deps: DepsMut,
    sender: &Addr,
    admin: bool,
    permissioned: bool,
    denoms: Vec<String>,
) -> Result<Response, ContractError> {
    if permissioned && !admin {
        return Err(ContractError::RemovePermissioned);
    }

    for denom in denoms {
        let Ok(listing) = DENOM_MAP.load(deps.storage, denom.clone()) else {
            return Err(ContractError::ListingNotFound(denom));
        };

        // make sure the sender is the creator of the listing or an admin
        if listing.author.unwrap_or_default() != *sender && !admin {
            return Err(ContractError::Unauthorized);
        }

        // remove the listing by denom and symbol
        DENOM_MAP.remove(deps.storage, denom.clone());
        SYMBOL_MAP.remove(deps.storage, listing.metadata.symbol);
    }

    Ok(Response::new().add_attribute("action", "assetlist_remove_listings"))
}

fn execute_update_config(
    deps: DepsMut,
    sender: &Addr,
    old_config: Config,
    new_config: &mut Config,
) -> Result<Response, ContractError> {
    // only the owner can update the config
    if sender != old_config.owner.clone().unwrap() {
        return Err(ContractError::NotOwner);
    }

    // set the new owner or default to the current owner
    new_config.owner = match new_config.owner.clone() {
        Some(owner) => {
            deps.api.addr_validate(owner.as_str())?;
            Some(owner)
        }
        None => old_config.owner.clone(),
    };

    // validate the new admins. if provided, will overwrite the current list
    // (an empty list will clear all admins)
    if let Some(mut admins) = new_config.admins.clone() {
        for address in &admins {
            deps.api.addr_validate(address.as_str())?;
        }
        // add the owner to the list of admins if it has changed
        admins.push(new_config.owner.clone().unwrap());
        new_config.admins = Some(admins);
    } else {
        new_config.admins = old_config.admins.clone();
    }

    // if other fields are None, they will not be updated
    new_config.add_permissioned = new_config
        .add_permissioned
        .take()
        .or(old_config.add_permissioned);

    new_config.remove_permissioned = new_config
        .remove_permissioned
        .take()
        .or(old_config.remove_permissioned);

    new_config.required_fields = new_config
        .required_fields
        .take()
        .or(old_config.required_fields);

    new_config.fee = new_config.fee.take().or(old_config.fee);

    CONFIG.save(deps.storage, new_config)?;

    Ok(Response::new().add_attribute("action", "assetlist_update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Listing(listing_query) => match listing_query {
            ListingQuery::Denom(denoms) => to_json_binary(&query_listings_by_denom(deps, &denoms)?),
            ListingQuery::Symbol(symbols) => {
                to_json_binary(&query_listings_by_symbol(deps, &symbols)?)
            }
            ListingQuery::All { start_after, limit } => {
                to_json_binary(&query_all_listings(deps, start_after, limit))
            }
        },
        QueryMsg::Config => to_json_binary(&CONFIG.load(deps.storage)?),
    }
}

fn query_listings_by_denom(deps: Deps, denoms: &[String]) -> StdResult<Vec<(String, Metadata)>> {
    let mut data = vec![];
    for denom in denoms {
        match DENOM_MAP.load(deps.storage, denom.to_string()) {
            Ok(denom_data) => data.push((denom.clone(), denom_data.metadata)),
            Err(_) => {
                return Err(cosmwasm_std::StdError::GenericErr {
                    msg: format!("Listing not found for {denom}"),
                })
            }
        }
    }

    Ok(data)
}

fn query_listings_by_symbol(deps: Deps, symbols: &[String]) -> StdResult<Vec<(String, Metadata)>> {
    let mut data = vec![];
    for symbol in symbols {
        match SYMBOL_MAP.load(deps.storage, symbol.to_string()) {
            Ok(denom) => match DENOM_MAP.load(deps.storage, denom.clone()) {
                Ok(denom_data) => data.push((denom, denom_data.metadata)),
                Err(_) => {
                    return Err(cosmwasm_std::StdError::GenericErr {
                        msg: format!("Listing not found for {symbol}"),
                    })
                }
            },
            Err(_) => {
                return Err(cosmwasm_std::StdError::GenericErr {
                    msg: format!("Listing not found for {symbol}"),
                })
            }
        }
    }

    Ok(data)
}

fn query_all_listings(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Vec<(String, Metadata)> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(Bound::exclusive);

    DENOM_MAP
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .filter_map(Result::ok)
        .map(|(denom, listing)| (denom, listing.metadata))
        .collect()

   // listings
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type"));
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}

fn check_required_fields(
    required_fields: &[Field],
    metadata: &Metadata,
) -> Result<(), ContractError> {
    for field in required_fields {
        match field {
            Exp => {
                if metadata.exp.is_none() {
                    return Err(ContractError::MissingField(Exp));
                }
            }
            Logo => {
                if metadata.logo.is_none() {
                    return Err(ContractError::MissingField(Logo));
                }
            }
            Chain => {
                if metadata.chain.is_none() {
                    return Err(ContractError::MissingField(Chain));
                }
            }
        }
    }

    Ok(())
}
