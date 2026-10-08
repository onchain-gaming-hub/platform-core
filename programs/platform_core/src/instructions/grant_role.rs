use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::RoleGranted,
    state::{PlatformConfig, Role, RoleMember},
};

/// Admin creates `RoleMember(role, member)`. Re-granting an existing pair fails
/// (account already in use). Granting Admin increments `admin_count` (I2).
#[derive(Accounts)]
#[instruction(role: Role, member: Pubkey)]
pub struct GrantRole<'info> {
    /// Must hold Admin; pays rent and becomes `granted_by` (rent returns here on revoke).
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[Role::Admin as u8], admin.key().as_ref()],
        bump = admin_role.bump,
    )]
    pub admin_role: Account<'info, RoleMember>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + RoleMember::INIT_SPACE,
        seeds = [ROLE_SEED, &[role as u8], member.as_ref()],
        bump,
    )]
    pub role_member: Account<'info, RoleMember>,

    pub system_program: Program<'info, System>,
}

pub fn handle_grant_role(ctx: Context<GrantRole>, role: Role, member: Pubkey) -> Result<()> {
    require_keys_neq!(member, Pubkey::default(), PlatformError::InvalidMember);
    let admin = ctx.accounts.admin.key();

    let role_member = &mut ctx.accounts.role_member;
    role_member.version = ACCOUNT_VERSION;
    role_member.bump = ctx.bumps.role_member;
    role_member.role = role;
    role_member.member = member;
    role_member.granted_by = admin;
    role_member.granted_at = Clock::get()?.unix_timestamp;

    if role == Role::Admin {
        let config = &mut ctx.accounts.config;
        config.admin_count = config
            .admin_count
            .checked_add(1)
            .ok_or(PlatformError::MathOverflow)?;
    }

    emit!(RoleGranted {
        role,
        member,
        by: admin,
    });
    Ok(())
}
