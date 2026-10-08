//! platform-core — shared program of blockchain-games-hub.
//! Design: specs/designs/ (01 layout · 02 instructions · 03 consumer integration).
//!
//! Phase 1: bootstrap + RBAC + fee (specs/designs/04-implementation-plan.md).

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

declare_id!("AJnPSYsn7J2muHfKztRtpZz5kBP94dWkNZLr9aMhu18G");

#[program]
pub mod platform_core {
    use super::*;

    /// One-time bootstrap. Signer must be the program's upgrade authority.
    pub fn initialize(
        ctx: Context<Initialize>,
        boost_fee_bps: u16,
        ad_fee_bps: u16,
        first_admin: Pubkey,
    ) -> Result<()> {
        instructions::initialize::handle_initialize(ctx, boost_fee_bps, ad_fee_bps, first_admin)
    }

    /// Admin only.
    pub fn grant_role(ctx: Context<GrantRole>, role: Role, member: Pubkey) -> Result<()> {
        instructions::grant_role::handle_grant_role(ctx, role, member)
    }

    /// Admin only. Cannot remove the last Admin.
    pub fn revoke_role(ctx: Context<RevokeRole>, role: Role, member: Pubkey) -> Result<()> {
        instructions::revoke_role::handle_revoke_role(ctx, role, member)
    }

    /// The member drops its own role. The last Admin cannot renounce.
    pub fn renounce_role(ctx: Context<RenounceRole>, role: Role) -> Result<()> {
        instructions::renounce_role::handle_renounce_role(ctx, role)
    }

    /// FeeManager | Admin (signer acts under `auth_role_kind`).
    pub fn set_fee(
        ctx: Context<SetFee>,
        auth_role_kind: Role,
        kind: FeeKind,
        fee_bps: u16,
    ) -> Result<()> {
        instructions::set_fee::handle_set_fee(ctx, auth_role_kind, kind, fee_bps)
    }
}
