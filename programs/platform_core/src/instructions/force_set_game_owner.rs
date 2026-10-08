use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::GameOwnerChanged,
    state::{GameEntry, Role, RoleMember},
};

/// Admin-only recovery when the owner lost its key (02 §1 #9). Restricted to Admin —
/// not GameRegistrar — because it redirects all of the game's ad revenue.
#[derive(Accounts)]
pub struct ForceSetGameOwner<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[Role::Admin as u8], admin.key().as_ref()],
        bump = admin_role.bump,
    )]
    pub admin_role: Account<'info, RoleMember>,

    #[account(mut)]
    pub game_entry: Account<'info, GameEntry>,
}

pub fn handle_force_set_game_owner(
    ctx: Context<ForceSetGameOwner>,
    new_owner: Pubkey,
) -> Result<()> {
    require_keys_neq!(new_owner, Pubkey::default(), PlatformError::InvalidMember);

    let entry = &mut ctx.accounts.game_entry;
    let old = entry.owner;
    entry.owner = new_owner;
    entry.updated_at = Clock::get()?.unix_timestamp;

    emit!(GameOwnerChanged {
        game_program: entry.game_program,
        old,
        new: new_owner,
        forced: true,
    });
    Ok(())
}
