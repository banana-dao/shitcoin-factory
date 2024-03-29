use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, ListingMsg, ListingQuery, QueryMsg};
use crate::state::{
    Config, Field,
    Field::{Chain, Exp, Logo},
    Metadata, CONFIG, DENOM_MAP, SYMBOL_MAP,
};
use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdResult,
};
use cw2::set_contract_version;
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
    let admin = config.admins.unwrap_or_default().contains(&info.sender);

    match msg {
        ExecuteMsg::Listing(msg) => match msg {
            ListingMsg::Add(listings) => execute_add_listings(
                deps,
                &info.sender,
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
        ExecuteMsg::UpdateConfig(new_config) => {
            execute_update_config(deps, &info.sender, config.owner, new_config)
        }
    }
}

fn execute_add_listings(
    deps: DepsMut,
    sender: &Addr,
    admin: bool,
    permissioned: bool,
    required_fields: &[Field],
    listings: Vec<Metadata>,
) -> Result<Response, ContractError> {
    if permissioned && !admin {
        return Err(ContractError::AddPermissioned);
    }

    // validate new listings
    for mut listing in listings {
        let denom = listing.denom.clone();
        let symbol = listing.symbol.clone();

        // we don't want to allow duplicate listings by denom or symbol as they will be used as keys
        if DENOM_MAP.has(deps.storage, denom.clone()) {
            return Err(ContractError::DuplicateListing(denom));
        }

        if SYMBOL_MAP.has(deps.storage, symbol.clone()) {
            return Err(ContractError::DuplicateListing(symbol));
        }

        // check for required fields
        if required_fields.contains(&Exp) && listing.exp.is_none() {
            return Err(ContractError::MissingField(Exp));
        }

        if required_fields.contains(&Logo) && listing.logo.is_none() {
            return Err(ContractError::MissingField(Logo));
        }

        if required_fields.contains(&Chain) && listing.chain.is_none() {
            return Err(ContractError::MissingField(Chain));
        }

        listing.set_author(sender.clone());

        DENOM_MAP.save(deps.storage, denom.clone(), &listing)?;
        SYMBOL_MAP.save(deps.storage, symbol, &denom)?;
    }

    Ok(Response::new().add_attribute("action", "assetlist_add_listings"))
}

fn execute_update_listings(
    deps: DepsMut,
    sender: &Addr,
    admin: bool,
    permissioned: bool,
    required_fields: &[Field],
    updates: Vec<Metadata>,
) -> Result<Response, ContractError> {
    // remove must be permissionless in order for creators to edit their own listings
    if permissioned && !admin {
        return Err(ContractError::RemovePermissioned);
    }

    // validate updated listings
    for mut update in updates {
        // make sure the denom is listed
        let Ok(current_listing) = DENOM_MAP.load(deps.storage, update.denom.clone()) else {
            return Err(ContractError::ListingNotFound(update.denom.clone()));
        };

        // make sure the sender is the creator of the listing or an admin
        if current_listing.get_author() != sender && !admin {
            return Err(ContractError::Unauthorized);
        }

        // make sure the new symbol is not already in use for a different denom
        if current_listing.symbol != update.symbol
            && SYMBOL_MAP.has(deps.storage, update.symbol.clone())
        {
            return Err(ContractError::DuplicateListing(update.symbol.clone()));
        }

        // check for required fields
        if required_fields.contains(&Exp) && update.exp.is_none() {
            return Err(ContractError::MissingField(Exp));
        }

        if required_fields.contains(&Logo) && update.logo.is_none() {
            return Err(ContractError::MissingField(Logo));
        }

        if required_fields.contains(&Chain) && update.chain.is_none() {
            return Err(ContractError::MissingField(Chain));
        }

        update.set_author(sender.clone());

        DENOM_MAP.save(deps.storage, update.denom.clone(), &update)?;
        SYMBOL_MAP.save(deps.storage, update.symbol.clone(), &update.denom)?;
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
        if listing.get_author() != sender && !admin {
            return Err(ContractError::Unauthorized);
        }

        // remove the listing by denom and symbol
        DENOM_MAP.remove(deps.storage, denom.clone());
        SYMBOL_MAP.remove(deps.storage, listing.symbol);
    }

    Ok(Response::new().add_attribute("action", "assetlist_remove_listings"))
}

fn execute_update_config(
    deps: DepsMut,
    sender: &Addr,
    owner: Option<Addr>,
    config: Config,
) -> Result<Response, ContractError> {
    // only the owner can update the config
    if sender != owner.clone().unwrap() {
        return Err(ContractError::NotOwner);
    }

    // set the new owner or default to the current owner
    let new_owner = config.owner.or(owner).unwrap();
    deps.api.addr_validate(new_owner.as_str())?;

    // validate the new admins and make the new owner an admin
    let mut admins = config.admins.unwrap_or_default();
    for address in &admins {
        deps.api.addr_validate(address.as_str())?;
    }
    admins.push(new_owner.clone());

    CONFIG.save(
        deps.storage,
        &Config {
            add_permissioned: config.add_permissioned,
            remove_permissioned: config.remove_permissioned,
            required_fields: config.required_fields,
            admins: Some(admins),
            owner: Some(new_owner),
        },
    )?;

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
                to_json_binary(&query_all_listings(deps, start_after, limit)?)
            }
        },
        QueryMsg::Config => to_json_binary(&CONFIG.load(deps.storage)?),
    }
}

fn query_listings_by_denom(deps: Deps, denoms: &[String]) -> StdResult<Vec<Metadata>> {
    let mut data = vec![];
    for denom in denoms {
        match DENOM_MAP.load(deps.storage, denom.to_string()) {
            Ok(denom_data) => data.push(denom_data),
            Err(_) => {
                return Err(cosmwasm_std::StdError::GenericErr {
                    msg: format!("Listing not found for {denom}"),
                })
            }
        }
    }

    Ok(data)
}

fn query_listings_by_symbol(deps: Deps, symbols: &[String]) -> StdResult<Vec<Metadata>> {
    let mut data = vec![];
    for symbol in symbols {
        match SYMBOL_MAP.load(deps.storage, symbol.to_string()) {
            Ok(denom) => match DENOM_MAP.load(deps.storage, denom) {
                Ok(denom_data) => data.push(denom_data),
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
) -> StdResult<Vec<Metadata>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(Bound::exclusive);
    let listings: Vec<Metadata> = DENOM_MAP
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .filter_map(Result::ok)
        .map(|(_, listing)| listing)
        .collect();

    Ok(listings)
}
