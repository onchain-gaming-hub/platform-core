//! platform-core — shared program of blockchain-games-hub.
//! Design: specs/designs/ (01 layout · 02 instructions · 03 consumer integration).
//!
//! Phase 0: account layouts, constants, errors and events only — no instructions yet
//! (specs/designs/04-implementation-plan.md).

pub mod constants;
pub mod error;
pub mod events;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use events::*;
pub use state::*;

declare_id!("AJnPSYsn7J2muHfKztRtpZz5kBP94dWkNZLr9aMhu18G");

#[program]
pub mod platform_core {}
