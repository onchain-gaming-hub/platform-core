use anchor_lang::prelude::*;

use crate::constants::{AD_CLICK_URL_MAX_LEN, AD_IMAGE_URL_MAX_LEN, AD_TEXT_MAX_LEN};

/// Image + text + click URL. ⚠️ Must match `ad-pool::AdCreative` field-for-field,
/// including every `max_len` (bytes, not characters) — FE renders both the same way.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq, InitSpace)]
pub struct AdCreative {
    /// Object URL in R2 (key `ads/<sha256>.<ext>` — Q3).
    #[max_len(AD_IMAGE_URL_MAX_LEN)]
    pub image_url: String,
    /// FE must check the fetched image hashes to this before showing it.
    pub image_sha256: [u8; 32],
    /// Fallback shown instead of the image (A13). Thai ≈ 40 characters.
    #[max_len(AD_TEXT_MAX_LEN)]
    pub text: String,
    #[max_len(AD_CLICK_URL_MAX_LEN)]
    pub click_url: String,
}

/// PDA `[DEFAULT_AD_SEED, width.to_le_bytes(), height.to_le_bytes()]` — one per size.
/// Layout: specs/designs/01 §7 (651 bytes reserved).
///
/// ⚠️ `creative` is variable-length, so everything after offset 55 (including
/// `_reserved`) has no fixed offset. Never memcmp past 55; extend via `AdCreative`
/// (coordinated with ad-pool) or a new account type (review R3).
#[account]
#[derive(InitSpace, Debug)]
pub struct DefaultAd {
    /// offset 8
    pub version: u8,
    /// offset 9
    pub bump: u8,
    /// offset 10 — duplicates the seed.
    pub width: u16,
    /// offset 12 — duplicates the seed.
    pub height: u16,
    /// offset 14 — false = FE shows nothing (account is kept, not closed).
    pub enabled: bool,
    /// offset 15
    pub updated_by: Pubkey,
    /// offset 47
    pub updated_at: i64,
    /// offset 55 — variable length (max 564).
    pub creative: AdCreative,
    /// variable offset
    pub _reserved: [u8; 32],
}
