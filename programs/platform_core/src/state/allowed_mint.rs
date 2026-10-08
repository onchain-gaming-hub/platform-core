use anchor_lang::prelude::*;

/// PDA `[BOOST_MINT_SEED, mint]`. Existing = mint may be boosted (rev 3 #3 · Q4).
/// Covers SPL Token and Token-2022 — `token_program` says which.
/// Layout: specs/designs/01 §5 (147 bytes).
#[account]
#[derive(InitSpace, Debug)]
pub struct AllowedMint {
    /// offset 8
    pub version: u8,
    /// offset 9
    pub bump: u8,
    /// offset 10
    pub mint: Pubkey,
    /// offset 42 — Token or Token-2022; consumers derive the Treasury ATA with it.
    pub token_program: Pubkey,
    /// offset 74 — snapshot for `transfer_checked`.
    pub decimals: u8,
    /// offset 75 — receives this account's rent back on `disallow_mint`.
    pub added_by: Pubkey,
    /// offset 107
    pub added_at: i64,
    /// offset 115
    pub _reserved: [u8; 32],
}
