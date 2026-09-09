use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
    TransferChecked,
};

use crate::{error::ErrorCode, state::Escrow, ESCROW_SEED};

#[derive(Accounts)]
pub struct Take<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,
    /// CHECK: The escrow constraint below verifies this is the maker.
    #[account(mut)]
    pub maker: UncheckedAccount<'info>,
    #[account(mint::token_program = token_program)]
    pub mint_a: InterfaceAccount<'info, Mint>,
    #[account(mint::token_program = token_program)]
    pub mint_b: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        close = maker,
        has_one = maker,
        has_one = mint_a,
        has_one = mint_b,
        seeds = [ESCROW_SEED, escrow.maker.as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(mut)]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> Take<'info> {
    pub fn take_and_close(&mut self) -> Result<()> {
        require!(self.escrow.receive > 0, ErrorCode::InvalidAmount);
        require_keys_eq!(self.taker_ata_a.mint, self.mint_a.key());
        require_keys_eq!(self.taker_ata_a.owner, self.taker.key());
        require_keys_eq!(self.taker_ata_b.mint, self.mint_b.key());
        require_keys_eq!(self.taker_ata_b.owner, self.taker.key());
        require_keys_eq!(self.maker_ata_b.mint, self.mint_b.key());
        require_keys_eq!(self.maker_ata_b.owner, self.maker.key());
        require_keys_eq!(self.vault.mint, self.mint_a.key());
        require_keys_eq!(self.vault.owner, self.escrow.key());

        let signer_seeds: &[&[u8]] = &[
            ESCROW_SEED,
            self.escrow.maker.as_ref(),
            &self.escrow.seed.to_le_bytes(),
            &[self.escrow.bump],
        ];

        let pay_accounts = TransferChecked {
            from: self.taker_ata_b.to_account_info(),
            mint: self.mint_b.to_account_info(),
            to: self.maker_ata_b.to_account_info(),
            authority: self.taker.to_account_info(),
        };
        transfer_checked(
            CpiContext::new(self.token_program.key(), pay_accounts),
            self.escrow.receive,
            self.mint_b.decimals,
        )?;

        let release_accounts = TransferChecked {
            from: self.vault.to_account_info(),
            mint: self.mint_a.to_account_info(),
            to: self.taker_ata_a.to_account_info(),
            authority: self.escrow.to_account_info(),
        };
        transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.key(),
                release_accounts,
                &[signer_seeds],
            ),
            self.vault.amount,
            self.mint_a.decimals,
        )?;

        let close_accounts = CloseAccount {
            account: self.vault.to_account_info(),
            destination: self.maker.to_account_info(),
            authority: self.escrow.to_account_info(),
        };
        close_account(CpiContext::new_with_signer(
            self.token_program.key(),
            close_accounts,
            &[signer_seeds],
        ))
    }
}
