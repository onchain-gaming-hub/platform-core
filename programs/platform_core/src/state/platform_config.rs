use anchor_lang::prelude::*;

/// PDA `[CONFIG_SEED]` — single instance. Layout: specs/designs/01 §2 (97 bytes).
///
/// No `max_fee_bps` (X1 — ceiling is `MAX_FEE_BPS` in code), no `treasury`
/// (X2 — derive from `TREASURY_SEED`), no `moderator` (rev 3 #5 — RBAC role).
#[account]
#[derive(InitSpace, Debug)]
pub struct PlatformConfig {
    /// offset 8
    pub version: u8,
    /// offset 9
    pub bump: u8,
    /// offset 10 — rewards-pool snapshots this at `open_pool`. ≤ MAX_FEE_BPS.
    pub boost_fee_bps: u16,
    /// offset 12 — ad-pool snapshots this at the first bid of a slot. ≤ MAX_FEE_BPS.
    pub ad_fee_bps: u16,
    /// offset 14 — number of live `RoleMember(Admin, *)`; never reaches 0.
    pub admin_count: u16,
    /// offset 16 — consumers use `bump = config.treasury_bump`.
    pub treasury_bump: u8,
    /// offset 17
    pub initialized_at: i64,
    /// offset 25 — last time either fee changed.
    pub updated_at: i64,
    /// offset 33
    pub _reserved: [u8; 64],
}
