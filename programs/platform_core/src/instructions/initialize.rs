use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::Initialized,
    program::PlatformCore,
    state::{PlatformConfig, Role, RoleMember, Treasury},
};

/// One-time bootstrap (02 §2.1 · X6). Only the program's upgrade authority may call it,
/// which blocks front-running of the `[CONFIG_SEED]` PDA (Gotcha #6). The Admin role goes
/// to `first_admin`, so a cold upgrade-authority key never has to act as Admin (C6).
#[derive(Accounts)]
#[instruction(boost_fee_bps: u16, ad_fee_bps: u16, first_admin: Pubkey)]
pub struct Initialize<'info> {
    /// Upgrade authority of this program; pays rent for the three accounts.
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        constraint = program.programdata_address()? == Some(program_data.key())
            @ PlatformError::Unauthorized,
    )]
    pub program: Program<'info, PlatformCore>,

    #[account(
        constraint = program_data.upgrade_authority_address == Some(authority.key())
            @ PlatformError::Unauthorized,
    )]
    pub program_data: Account<'info, ProgramData>,

    #[account(
        init,
        payer = authority,
        space = 8 + PlatformConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = authority,
        space = 8 + Treasury::INIT_SPACE,
        seeds = [TREASURY_SEED],
        bump,
    )]
    pub treasury: Account<'info, Treasury>,

    #[account(
        init,
        payer = authority,
        space = 8 + RoleMember::INIT_SPACE,
        seeds = [ROLE_SEED, &[Role::Admin as u8], first_admin.as_ref()],
        bump,
    )]
    pub admin_role: Account<'info, RoleMember>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(
    ctx: Context<Initialize>,
    boost_fee_bps: u16,
    ad_fee_bps: u16,
    first_admin: Pubkey,
) -> Result<()> {
    require!(boost_fee_bps <= MAX_FEE_BPS, PlatformError::FeeTooHigh);
    require!(ad_fee_bps <= MAX_FEE_BPS, PlatformError::FeeTooHigh);
    // An all-zero first Admin would lock the platform forever (nobody can sign for it).
    require_keys_neq!(first_admin, Pubkey::default(), PlatformError::InvalidMember);

    let now = Clock::get()?.unix_timestamp;
    let authority = ctx.accounts.authority.key();

    let config = &mut ctx.accounts.config;
    config.version = ACCOUNT_VERSION;
    config.bump = ctx.bumps.config;
    config.boost_fee_bps = boost_fee_bps;
    config.ad_fee_bps = ad_fee_bps;
    config.admin_count = 1;
    config.treasury_bump = ctx.bumps.treasury;
    config.initialized_at = now;
    config.updated_at = now;

    let treasury = &mut ctx.accounts.treasury;
    treasury.version = ACCOUNT_VERSION;
    treasury.bump = ctx.bumps.treasury;

    let admin_role = &mut ctx.accounts.admin_role;
    admin_role.version = ACCOUNT_VERSION;
    admin_role.bump = ctx.bumps.admin_role;
    admin_role.role = Role::Admin;
    admin_role.member = first_admin;
    admin_role.granted_by = authority;
    admin_role.granted_at = now;

    emit!(Initialized {
        authority,
        first_admin,
        boost_fee_bps,
        ad_fee_bps,
    });
    Ok(())
}
