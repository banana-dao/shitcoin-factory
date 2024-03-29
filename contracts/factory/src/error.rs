use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Not authorized to perform this action")]
    Unauthorized,

    #[error("Cannot reduce max supply below current supply")]
    CurrentSupply,

    #[error("Cannot mint more than max supply")]
    SupplyCap,

    #[error("Invalid transfer message at index {}", .0)]
    TransferInvalid(usize),

    #[error("Invalid mint message at index {}", .0)]
    MintInvalid(usize),
}
