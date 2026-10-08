use anchor_lang::prelude::*;

use crate::{error::PlatformError, events::GameOwnerChanged, state::GameEntry};

/// Current owner hands the game to a new owner; both must sign so revenue can never be
/// redirected to a key nobody controls (EVM analogue: two-step ownership transfer).
#[derive(Accounts)]
pub struct TransferGameOwner<'info> {
    #[account(address = game_entry.owner @ PlatformError::Unauthorized)]
    pub owner: Signer<'info>,

    pub new_owner: Signer<'info>,

    #[account(mut)]
    pub game_entry: Account<'info, GameEntry>,
}

pub fn handle_transfer_game_owner(ctx: Context<TransferGameOwner>) -> Result<()> {
    let new = ctx.accounts.new_owner.key();
    let entry = &mut ctx.accounts.game_entry;
    let old = entry.owner;
    entry.owner = new;
    entry.updated_at = Clock::get()?.unix_timestamp;

    emit!(GameOwnerChanged {
        game_program: entry.game_program,
        old,
        new,
        forced: false,
    });
    Ok(())
}
