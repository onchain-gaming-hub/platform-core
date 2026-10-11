//! Events. Emitted with `emit!` (Q7) — logs can be truncated,
//! so consumers treat an event as a "refresh" signal; the account is the truth.

use anchor_lang::prelude::*;

use crate::state::{FeeKind, GameStatus, Role};

#[event]
pub struct Initialized {
    pub authority: Pubkey,
    pub first_admin: Pubkey,
    pub boost_fee_bps: u16,
    pub ad_fee_bps: u16,
}

#[event]
pub struct RoleGranted {
    pub role: Role,
    pub member: Pubkey,
    pub by: Pubkey,
}

#[event]
pub struct RoleRevoked {
    pub role: Role,
    pub member: Pubkey,
    pub by: Pubkey,
}

#[event]
pub struct FeeChanged {
    pub kind: FeeKind,
    pub old_bps: u16,
    pub new_bps: u16,
    pub by: Pubkey,
}

#[event]
pub struct GameRegistered {
    pub game_program: Pubkey,
    pub owner: Pubkey,
    pub game_authority: Pubkey,
}

#[event]
pub struct GameStatusChanged {
    pub game_program: Pubkey,
    pub old: GameStatus,
    pub new: GameStatus,
}

#[event]
pub struct GameOwnerChanged {
    pub game_program: Pubkey,
    pub old: Pubkey,
    pub new: Pubkey,
    pub forced: bool,
}

#[event]
pub struct MintAllowed {
    pub mint: Pubkey,
    pub token_program: Pubkey,
}

#[event]
pub struct MintDisallowed {
    pub mint: Pubkey,
}

#[event]
pub struct TreasuryWithdrawn {
    /// `None` = SOL.
    pub mint: Option<Pubkey>,
    pub amount: u64,
    pub to: Pubkey,
}

#[event]
pub struct DefaultAdSet {
    pub width: u16,
    pub height: u16,
    pub enabled: bool,
}
