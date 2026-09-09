use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The amount must be greater than zero")]
    InvalidAmount,
    #[msg("The vault must retain rent and the amount must fit")]
    InsufficientFunds,
    #[msg("The vault must be empty before closing")]
    VaultNotEmpty,
}
