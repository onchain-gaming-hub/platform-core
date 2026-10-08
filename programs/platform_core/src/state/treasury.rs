use anchor_lang::prelude::*;

/// PDA `[TREASURY_SEED]` — program-owned, single instance. Holds platform SOL fees
/// directly; SPL fees live in its ATAs. Debited only by `withdraw_*` (I5).
/// Layout: specs/designs/01 §6 (42 bytes).
#[account]
#[derive(InitSpace, Debug)]
pub struct Treasury {
    /// offset 8
    pub version: u8,
    /// offset 9
    pub bump: u8,
    /// offset 10
    pub _reserved: [u8; 32],
}
