use anchor_lang::prelude::*;

use super::GameStatus;

/// PDA `[GAME_SEED, game_program]`. Never closed (I4) — only `Suspended`.
/// Layout: specs/designs/01 §4 (220 bytes).
#[account]
#[derive(InitSpace, Debug)]
pub struct GameEntry {
    /// offset 8
    pub version: u8,
    /// offset 9
    pub bump: u8,
    /// offset 10 — ⚠️ relayer memcmp: 0x00 = Active. Never move.
    pub status: GameStatus,
    /// offset 11 — relayer reads this with dataSlice(11, 32).
    pub game_program: Pubkey,
    /// offset 43 — the single source of "game owner" in the hub (rev 3 #6).
    pub owner: Pubkey,
    /// offset 75 — `find_program_address([GAME_AUTHORITY_SEED], game_program)` (X5).
    pub game_authority: Pubkey,
    /// offset 107
    pub game_authority_bump: u8,
    /// offset 108
    pub registered_by: Pubkey,
    /// offset 140
    pub registered_at: i64,
    /// offset 148 — last status / owner change.
    pub updated_at: i64,
    /// offset 156
    pub _reserved: [u8; 64],
}
