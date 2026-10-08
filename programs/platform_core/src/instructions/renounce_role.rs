use anchor_lang::prelude::*;

use crate::{
    constants::*,
    events::RoleRevoked,
    instructions::release_admin_seat,
    state::{PlatformConfig, Role, RoleMember},
};

/// The member gives up its own role; rent goes back to `granted_by` (D11).
/// The last Admin cannot renounce (I2 · Gotcha #7).
#[derive(Accounts)]
#[instruction(role: Role)]
pub struct RenounceRole<'info> {
    pub member: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [ROLE_SEED, &[role as u8], member.key().as_ref()],
        bump = role_member.bump,
        close = granted_by,
    )]
    pub role_member: Account<'info, RoleMember>,

    /// CHECK: rent receiver only — must equal `role_member.granted_by`.
    #[account(mut, address = role_member.granted_by)]
    pub granted_by: UncheckedAccount<'info>,
}

pub fn handle_renounce_role(ctx: Context<RenounceRole>, role: Role) -> Result<()> {
    if role == Role::Admin {
        release_admin_seat(&mut ctx.accounts.config)?;
    }

    let member = ctx.accounts.member.key();
    emit!(RoleRevoked {
        role,
        member,
        by: member,
    });
    Ok(())
}
