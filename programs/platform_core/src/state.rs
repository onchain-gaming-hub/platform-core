//! Account layouts — the contract every consumer depends on (specs/designs/01).
//!
//! Rules for every account:
//! - byte 8 = `version`, byte 9 = `bump` (right after the 8-byte discriminator)
//! - fields a consumer/relayer may `memcmp` sit at fixed offsets near the head
//! - `_reserved` at the tail; new fields are carved out of it, never appended or reordered
//! - never rename a struct (its discriminator would change)

pub mod allowed_mint;
pub mod default_ad;
pub mod enums;
pub mod game_entry;
pub mod platform_config;
pub mod role_member;
pub mod treasury;

pub use allowed_mint::*;
pub use default_ad::*;
pub use enums::*;
pub use game_entry::*;
pub use platform_config::*;
pub use role_member::*;
pub use treasury::*;
