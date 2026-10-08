use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::GameStatusChanged,
    state::{GameEntry, GameStatus, Role, RoleMember},
};

/// GameRegistrar | Admin switches Active ↔ Suspended. Suspended blocks only *new* work
/// in consumers (rooms, pools, ad configs, bids) — never claims/refunds (rev 3 #7).
#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct SetGameStatus<'info> {
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::GameRegistrar | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    /// `Account<GameEntry>` (owner + discriminator) is enough here: platform-core only
    /// ever creates a GameEntry at its `[GAME_SEED, game_program]` PDA.
    #[account(mut)]
    pub game_entry: Account<'info, GameEntry>,
}

pub fn handle_set_game_status(
    ctx: Context<SetGameStatus>,
    _auth_role_kind: Role,
    status: GameStatus,
) -> Result<()> {
    let entry = &mut ctx.accounts.game_entry;
    let old = entry.status;
    entry.status = status;
    entry.updated_at = Clock::get()?.unix_timestamp;

    emit!(GameStatusChanged {
        game_program: entry.game_program,
        old,
        new: status,
    });
    Ok(())
}
