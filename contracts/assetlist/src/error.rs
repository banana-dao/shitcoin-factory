use crate::state::Field;
use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Must be owner to update config")]
    NotOwner,

    #[error("Must be an admin add new listings")]
    AddPermissioned,

    #[error("Must be an admin to edit/remove listings")]
    RemovePermissioned,

    #[error("Not authorized to edit/remove this listing")]
    Unauthorized,

    #[error("Valid fees are required to add a listing")]
    MissingFee,

    #[error("Entire fee must be paid in one of the accepted tokens")]
    MultipleFees,

    #[error("Invalid fee token")]
    InvalidFee,

    #[error("Insufficient fee paid")]
    InsufficientFee,

    #[error("Duplicate listing found for {}", 0)]
    DuplicateListing(String),

    #[error("Listing not found for {}", 0)]
    ListingNotFound(String),

    #[error("Required field {} is missing", 0)]
    MissingField(Field),
}
