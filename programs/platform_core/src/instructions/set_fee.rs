use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::FeeChanged,
    state::{FeeKind, PlatformConfig, Role, RoleMember},
};

/// FeeManager | Admin updates one fee (X7). `auth_role_kind` is the role the signer
/// acts under (R1): it only selects the PDA to check, and cannot be forged because the
/// seeds must match a `RoleMember` that platform-core actually created (I6).
#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct SetFee<'info> {
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::FeeManager | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,
}

pub fn handle_set_fee(
    ctx: Context<SetFee>,
    _auth_role_kind: Role,
    kind: FeeKind,
    fee_bps: u16,
) -> Result<()> {
    require!(fee_bps <= MAX_FEE_BPS, PlatformError::FeeTooHigh);

    let config = &mut ctx.accounts.config;
    let slot = match kind {
        FeeKind::Boost => &mut config.boost_fee_bps,
        FeeKind::Ad => &mut config.ad_fee_bps,
    };
    let old_bps = *slot;
    *slot = fee_bps;
    config.updated_at = Clock::get()?.unix_timestamp;

    emit!(FeeChanged {
        kind,
        old_bps,
        new_bps: fee_bps,
        by: ctx.accounts.signer.key(),
    });
    Ok(())
}
