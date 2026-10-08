use {
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    constants::*,
    events::TreasuryWithdrawn,
    state::{PlatformConfig, Role, RoleMember, Treasury},
};

/// Admin withdraws platform token revenue from the Treasury's ATA (C5 · D2), signed by
/// the Treasury PDA. Works for any mint, allowlisted or not — fees of a disallowed mint
/// must still be withdrawable.
#[derive(Accounts)]
pub struct WithdrawToken<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[Role::Admin as u8], admin.key().as_ref()],
        bump = admin_role.bump,
    )]
    pub admin_role: Account<'info, RoleMember>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,

    #[account(seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: Account<'info, Treasury>,

    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    /// Any token account of this mint chosen by the Admin.
    #[account(mut, token::mint = mint, token::token_program = token_program)]
    pub to: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_withdraw_token(ctx: Context<WithdrawToken>, amount: u64) -> Result<()> {
    let signer_seeds: &[&[&[u8]]] = &[&[TREASURY_SEED, &[ctx.accounts.config.treasury_bump]]];
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.treasury_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.to.to_account_info(),
                authority: ctx.accounts.treasury.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    emit!(TreasuryWithdrawn {
        mint: Some(ctx.accounts.mint.key()),
        amount,
        to: ctx.accounts.to.key(),
    });
    Ok(())
}
