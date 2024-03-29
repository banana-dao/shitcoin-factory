use cosmwasm_std::Addr;
use cw_storage_plus::Item;

#[repr(u8)]
pub enum TopKey {
    Admin = b'a',
    Symbol = b'b',
    Denom = b'c',
    MaxSupply = b'd',
    TotalMinted = b'e',
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

pub const ADMIN: Item<Addr> = Item::new(TopKey::Admin.as_str());
pub const SYMBOL: Item<String> = Item::new(TopKey::Symbol.as_str());
pub const DENOM: Item<String> = Item::new(TopKey::Denom.as_str());
pub const MAX_SUPPLY: Item<u128> = Item::new(TopKey::MaxSupply.as_str());
pub const TOTAL_MINTED: Item<u128> = Item::new(TopKey::TotalMinted.as_str());
