//! platform-core — shared program of blockchain-games-hub.
//! Design: specs/designs/ (01 layout · 02 instructions · 03 consumer integration).
//!
//! Phase 2: + game registry · boost mint allowlist · treasury withdraw
//! (specs/designs/04-implementation-plan.md).

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod mint_policy;
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

    /// GameRegistrar | Admin + the game owner co-signs.
    pub fn register_game(ctx: Context<RegisterGame>, auth_role_kind: Role) -> Result<()> {
        instructions::register_game::handle_register_game(ctx, auth_role_kind)
    }

    /// GameRegistrar | Admin.
    pub fn set_game_status(
        ctx: Context<SetGameStatus>,
        auth_role_kind: Role,
        status: GameStatus,
    ) -> Result<()> {
        instructions::set_game_status::handle_set_game_status(ctx, auth_role_kind, status)
    }

    /// Current owner + new owner both sign.
    pub fn transfer_game_owner(ctx: Context<TransferGameOwner>) -> Result<()> {
        instructions::transfer_game_owner::handle_transfer_game_owner(ctx)
    }

    /// Admin only — recovery when the owner lost its key.
    pub fn force_set_game_owner(ctx: Context<ForceSetGameOwner>, new_owner: Pubkey) -> Result<()> {
        instructions::force_set_game_owner::handle_force_set_game_owner(ctx, new_owner)
    }

    /// AllowlistManager | Admin. Also creates the Treasury ATA for the mint.
    pub fn allow_mint(ctx: Context<AllowMint>, auth_role_kind: Role) -> Result<()> {
        instructions::allow_mint::handle_allow_mint(ctx, auth_role_kind)
    }

    /// AllowlistManager | Admin. Treasury ATA is kept.
    pub fn disallow_mint(ctx: Context<DisallowMint>, auth_role_kind: Role) -> Result<()> {
        instructions::disallow_mint::handle_disallow_mint(ctx, auth_role_kind)
    }

    /// Admin only. Keeps Treasury rent-exempt.
    pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        instructions::withdraw_sol::handle_withdraw_sol(ctx, amount)
    }

    /// Admin only. From the Treasury ATA of `mint`.
    pub fn withdraw_token(ctx: Context<WithdrawToken>, amount: u64) -> Result<()> {
        instructions::withdraw_token::handle_withdraw_token(ctx, amount)
    }
}
