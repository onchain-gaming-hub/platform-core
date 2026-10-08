pub mod allow_mint;
pub mod disallow_mint;
pub mod force_set_game_owner;
pub mod grant_role;
pub mod initialize;
pub mod register_game;
pub mod renounce_role;
pub mod revoke_role;
pub mod set_fee;
pub mod set_game_status;
pub mod transfer_game_owner;
pub mod withdraw_sol;
pub mod withdraw_token;

pub use allow_mint::*;
pub use disallow_mint::*;
pub use force_set_game_owner::*;
pub use grant_role::*;
pub use initialize::*;
pub use register_game::*;
pub use renounce_role::*;
pub use revoke_role::*;
pub use set_fee::*;
pub use set_game_status::*;
pub use transfer_game_owner::*;
pub use withdraw_sol::*;
pub use withdraw_token::*;

use anchor_lang::prelude::*;

use crate::{error::PlatformError, state::PlatformConfig};

/// Shared by `revoke_role` and `renounce_role` when the role being removed is Admin:
/// refuses to drop the last Admin (I2 · Gotcha #7), otherwise decrements the count.
pub(crate) fn release_admin_seat(config: &mut PlatformConfig) -> Result<()> {
    require!(config.admin_count > 1, PlatformError::LastAdmin);
    config.admin_count -= 1;
    Ok(())
}
