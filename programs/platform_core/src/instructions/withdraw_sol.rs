use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::TreasuryWithdrawn,
    state::{PlatformConfig, Role, RoleMember, Treasury},
};

/// Admin withdraws platform SOL revenue (C5 · D2: one Admin is enough). Treasury is
/// program-owned, so lamports are moved directly — no CPI. Never below rent-exempt (I3).
#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[Role::Admin as u8], admin.key().as_ref()],
        bump = admin_role.bump,
    )]
    pub admin_role: Account<'info, RoleMember>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,

    #[account(mut, seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: Account<'info, Treasury>,

    /// CHECK: any destination the Admin chooses (revenue wallet, multisig, …).
    #[account(mut)]
    pub to: UncheckedAccount<'info>,
}

pub fn handle_withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
    let treasury = ctx.accounts.treasury.to_account_info();
    let min_balance = Rent::get()?.minimum_balance(treasury.data_len());
    let available = treasury
        .lamports()
        .checked_sub(min_balance)
        .ok_or(PlatformError::InsufficientTreasury)?;
    require!(amount <= available, PlatformError::InsufficientTreasury);

    ctx.accounts.treasury.sub_lamports(amount)?;
    ctx.accounts.to.add_lamports(amount)?;

    emit!(TreasuryWithdrawn {
        mint: None,
        amount,
        to: ctx.accounts.to.key(),
    });
    Ok(())
}
