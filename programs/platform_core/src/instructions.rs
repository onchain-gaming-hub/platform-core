pub mod grant_role;
pub mod initialize;
pub mod renounce_role;
pub mod revoke_role;
pub mod set_fee;

pub use grant_role::*;
pub use initialize::*;
pub use renounce_role::*;
pub use revoke_role::*;
pub use set_fee::*;

use anchor_lang::prelude::*;

use crate::{error::PlatformError, state::PlatformConfig};

/// Shared by `revoke_role` and `renounce_role` when the role being removed is Admin:
/// refuses to drop the last Admin (I2 · Gotcha #7), otherwise decrements the count.
pub(crate) fn release_admin_seat(config: &mut PlatformConfig) -> Result<()> {
    require!(config.admin_count > 1, PlatformError::LastAdmin);
    config.admin_count -= 1;
    Ok(())
}
