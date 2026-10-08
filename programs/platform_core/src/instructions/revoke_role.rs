use anchor_lang::prelude::*;

use crate::{
    constants::*,
    events::RoleRevoked,
    instructions::release_admin_seat,
    state::{PlatformConfig, Role, RoleMember},
};

/// Admin closes `RoleMember(role, member)`; rent goes back to its `granted_by` (D11).
/// Revoking an Admin is refused if it is the last one (I2 · Gotcha #7).
#[derive(Accounts)]
#[instruction(role: Role, member: Pubkey)]
pub struct RevokeRole<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[Role::Admin as u8], admin.key().as_ref()],
        bump = admin_role.bump,
    )]
    pub admin_role: Account<'info, RoleMember>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [ROLE_SEED, &[role as u8], member.as_ref()],
        bump = role_member.bump,
        close = granted_by,
    )]
    pub role_member: Account<'info, RoleMember>,

    /// CHECK: rent receiver only — must equal `role_member.granted_by`.
    #[account(mut, address = role_member.granted_by)]
    pub granted_by: UncheckedAccount<'info>,
}

pub fn handle_revoke_role(ctx: Context<RevokeRole>, role: Role, member: Pubkey) -> Result<()> {
    if role == Role::Admin {
        release_admin_seat(&mut ctx.accounts.config)?;
    }

    emit!(RoleRevoked {
        role,
        member,
        by: ctx.accounts.admin.key(),
    });
    Ok(())
}
