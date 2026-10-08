use anchor_lang::prelude::*;

/// ⚠️ Never reorder — `role as u8` is a PDA seed.
/// ⚠️ INVARIANT: every explicit value MUST equal its variant index. The IDL carries no
/// discriminant values, so `declare_program!` consumers encode the *index*; a gap or
/// out-of-order value would silently desync them. Append only, next value = next index
/// (guarded by `enum_bytes_frozen_v1` + `enum_byte_equals_declaration_index` and the
/// compile-time `enum_tripwire` in tests/test_layout.rs).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
#[borsh(use_discriminant = true)] // encoded byte == written value (== variant index)
#[repr(u8)]
pub enum Role {
    Admin = 0,
    GameRegistrar = 1,
    Moderator = 2,
    FeeManager = 3,
    AllowlistManager = 4,
}

/// ⚠️ Never reorder — the relayer filters `GameEntry` with memcmp(offset 10, 0x00 = Active).
/// ⚠️ INVARIANT: every explicit value MUST equal its variant index. The IDL carries no
/// discriminant values, so `declare_program!` consumers encode the *index*; a gap or
/// out-of-order value would silently desync them. Append only, next value = next index
/// (guarded by `enum_bytes_frozen_v1` + `enum_byte_equals_declaration_index` and the
/// compile-time `enum_tripwire` in tests/test_layout.rs).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
#[borsh(use_discriminant = true)] // encoded byte == written value (== variant index)
#[repr(u8)]
pub enum GameStatus {
    /// Normal operation.
    Active = 0,
    /// No new rooms / pools / ad configs / bids. Claims, refunds, settles and
    /// cancels of things already started must still work (spec rev 3 #7 · R12).
    Suspended = 1,
}

/// Which fee `set_fee` updates (X7).
/// ⚠️ INVARIANT: every explicit value MUST equal its variant index. The IDL carries no
/// discriminant values, so `declare_program!` consumers encode the *index*; a gap or
/// out-of-order value would silently desync them. Append only, next value = next index
/// (guarded by `enum_bytes_frozen_v1` + `enum_byte_equals_declaration_index` and the
/// compile-time `enum_tripwire` in tests/test_layout.rs).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
#[borsh(use_discriminant = true)] // encoded byte == written value (== variant index)
#[repr(u8)]
pub enum FeeKind {
    /// `PlatformConfig.boost_fee_bps` — rewards-pool.
    Boost = 0,
    /// `PlatformConfig.ad_fee_bps` — ad-pool.
    Ad = 1,
}
