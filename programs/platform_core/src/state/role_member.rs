use anchor_lang::prelude::*;

use super::Role;

/// PDA `[ROLE_SEED, &[role as u8], member]`. Existing = has role; closed = revoked.
/// EVM analogue: OZ `AccessControl` `mapping(bytes32 => mapping(address => bool))`.
/// Layout: specs/designs/01 §3 (115 bytes).
#[account]
#[derive(InitSpace, Debug)]
pub struct RoleMember {
    /// offset 8
    pub version: u8,
    /// offset 9
    pub bump: u8,
    /// offset 10 — duplicates the seed so readers don't need it.
    pub role: Role,
    /// offset 11 — duplicates the seed.
    pub member: Pubkey,
    /// offset 43 — receives the rent back on revoke / renounce (D11).
    pub granted_by: Pubkey,
    /// offset 75
    pub granted_at: i64,
    /// offset 83
    pub _reserved: [u8; 32],
}
