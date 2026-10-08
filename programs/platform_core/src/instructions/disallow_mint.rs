use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::MintDisallowed,
    state::{AllowedMint, Role, RoleMember},
};

/// AllowlistManager | Admin removes a mint from the boost allowlist. Closes
/// `AllowedMint` (rent → `added_by`) but leaves the Treasury ATA untouched: it may still
/// hold fees, and boosts made before the disallow must stay claimable (03 rule 3).
#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct DisallowMint<'info> {
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::AllowlistManager | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    /// `Account<AllowedMint>` (owner + discriminator) is enough: platform-core only
    /// creates it at `[BOOST_MINT_SEED, mint]`.
    #[account(mut, close = added_by)]
    pub allowed_mint: Account<'info, AllowedMint>,

    /// CHECK: rent receiver only — must equal `allowed_mint.added_by`.
    #[account(mut, address = allowed_mint.added_by)]
    pub added_by: UncheckedAccount<'info>,
}

pub fn handle_disallow_mint(ctx: Context<DisallowMint>, _auth_role_kind: Role) -> Result<()> {
    emit!(MintDisallowed {
        mint: ctx.accounts.allowed_mint.mint,
    });
    Ok(())
}
