//! Seeds and limits shared with every consumer program.
//! ⚠️ Interface — never change a seed value after deploy (docs/account-layout.md §9).

use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const TREASURY_SEED: &[u8] = b"treasury";

/// `[ROLE_SEED, &[role as u8], member]`
#[constant]
pub const ROLE_SEED: &[u8] = b"role";

/// `[GAME_SEED, game_program]`
#[constant]
pub const GAME_SEED: &[u8] = b"game";

/// `[BOOST_MINT_SEED, mint]`
#[constant]
pub const BOOST_MINT_SEED: &[u8] = b"boost_mint";

/// `[DEFAULT_AD_SEED, width.to_le_bytes(), height.to_le_bytes()]`
#[constant]
pub const DEFAULT_AD_SEED: &[u8] = b"default_ad";

/// Seed of the PDA signer inside each *game* program (pattern R1), not of platform-core.
#[constant]
pub const GAME_AUTHORITY_SEED: &[u8] = b"game_authority";

/// Fee ceiling enforced in code (C2 · X1) — 2000 bps = 20%.
#[constant]
pub const MAX_FEE_BPS: u16 = 2_000;

#[constant]
pub const BPS_DENOMINATOR: u16 = 10_000;

/// Layout version written into byte 8 of every account at `init`.
#[constant]
pub const ACCOUNT_VERSION: u8 = 1;

/// `AdCreative` string limits in bytes — must match `ad-pool::AdCreative` (01 §7).
#[constant]
pub const AD_IMAGE_URL_MAX_LEN: u16 = 200;

#[constant]
pub const AD_TEXT_MAX_LEN: u16 = 120;

#[constant]
pub const AD_CLICK_URL_MAX_LEN: u16 = 200;
