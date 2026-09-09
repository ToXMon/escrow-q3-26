use crate::{
    constants::{STATE, VAULT_SEED},
    error::ErrorCode,
    state::VaultState,
};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        close = user,
        seeds = [STATE, user.key().as_ref()],
        bump = vault_state.state_bump
    )]
    pub vault_state: Account<'info, VaultState>,
    #[account(
        mut,
        seeds = [VAULT_SEED, user.key().as_ref()],
        bump = vault_state.vault_bump,
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Close<'info> {
    pub fn close(&mut self) -> Result<()> {
        let rent = Rent::get()?.minimum_balance(self.vault.data_len());
        require!(self.vault.lamports() <= rent, ErrorCode::VaultNotEmpty);

        let amount = self.vault.lamports();
        if amount > 0 {
            let seeds: &[&[u8]] = &[
                VAULT_SEED,
                self.user.key.as_ref(),
                &[self.vault_state.vault_bump],
            ];
            let accounts = Transfer {
                from: self.vault.to_account_info(),
                to: self.user.to_account_info(),
            };
            let signer_seeds = [seeds];
            let context =
                CpiContext::new_with_signer(self.system_program.key(), accounts, &signer_seeds);
            transfer(context, amount)?;
        }
        Ok(())
    }
}
