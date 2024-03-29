use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

#[repr(u8)]
pub enum TopKey {
    Config = b'a',
    DenomMap = b'b',
    SymbolMap = b'c',
}

impl TopKey {
    const fn as_str(&self) -> &str {
        let array_ref = unsafe { std::mem::transmute::<_, &[u8; 1]>(self) };
        match core::str::from_utf8(array_ref) {
            Ok(a) => a,
            Err(_) => panic!("Non-utf8 enum value found. Use a-z, A-Z and 0-9"),
        }
    }
}

pub const CONFIG: Item<Config> = Item::new(TopKey::Config.as_str());
pub const DENOM_MAP: Map<String, Metadata> = Map::new(TopKey::DenomMap.as_str());
// maps symbols to denoms, to allow reverse lookup without iterating over or re-storing all metadata
pub const SYMBOL_MAP: Map<String, String> = Map::new(TopKey::SymbolMap.as_str());

#[cw_serde]
pub struct Config {
    // When true only admins can add listings
    pub add_permissioned: Option<bool>,
    // When true only admins can remove. When false, users can remove their own listings
    pub remove_permissioned: Option<bool>,
    // The fields that are required for each listing
    pub required_fields: Option<Vec<Field>>,
    // Admins who can manage the asset list. The contract owner will be assigned automatically
    pub admins: Option<Vec<Addr>>,
    // The owner of the contract. Defaults to the instantiator
    pub owner: Option<Addr>,
}

#[cw_serde]
pub enum Field {
    Exp,
    Logo,
    Chain,
}

#[cw_serde]
pub struct Metadata {
    pub denom: String,
    pub symbol: String,
    pub exp: Option<u32>,
    pub logo: Option<String>,
    pub chain: Option<String>,
    author: Addr,
}

impl Metadata {
    pub fn get_author(&self) -> Addr {
        self.author.clone()
    }

    pub fn set_author(&mut self, author: Addr) {
        self.author = author;
    }
}
