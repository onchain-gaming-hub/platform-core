//! Golden layout tests — every size, offset, discriminator and enum byte must match
//! docs/account-layout.md. If one of these fails, a consumer
//! (rewards-pool, ad-pool, lobby, relayer memcmp) would read the account wrong.

use {
    anchor_lang::{prelude::Pubkey, AccountSerialize, AnchorSerialize, Discriminator, Space},
    platform_core::{
        constants::*,
        state::{
            AdCreative, AllowedMint, DefaultAd, FeeKind, GameEntry, GameStatus, PlatformConfig,
            Role, RoleMember, Treasury,
        },
    },
    sha2::{Digest, Sha256},
};

const DISC: usize = 8;

fn serialize<T: AccountSerialize>(acct: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    acct.try_serialize(&mut buf).unwrap();
    buf
}

/// Borsh bytes of a non-account type (Borsh 1.x has no `try_to_vec`).
fn to_bytes<T: AnchorSerialize>(value: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    value.serialize(&mut buf).unwrap();
    buf
}

fn pk(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn expected_discriminator(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("account:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}

fn i64_at(b: &[u8], off: usize) -> i64 {
    i64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

fn pk_at(b: &[u8], off: usize) -> Pubkey {
    Pubkey::new_from_array(b[off..off + 32].try_into().unwrap())
}

// ---------------------------------------------------------------- sizes

#[test]
fn account_sizes_match_design() {
    assert_eq!(DISC + PlatformConfig::INIT_SPACE, 97, "PlatformConfig");
    assert_eq!(DISC + RoleMember::INIT_SPACE, 115, "RoleMember");
    assert_eq!(DISC + GameEntry::INIT_SPACE, 220, "GameEntry");
    assert_eq!(DISC + AllowedMint::INIT_SPACE, 147, "AllowedMint");
    assert_eq!(DISC + Treasury::INIT_SPACE, 42, "Treasury");
    assert_eq!(
        DISC + DefaultAd::INIT_SPACE,
        651,
        "DefaultAd (fully reserved)"
    );
    assert_eq!(
        AdCreative::INIT_SPACE,
        564,
        "AdCreative = (4+200) + 32 + (4+120) + (4+200)"
    );
}

// ---------------------------------------------------------- discriminators

#[test]
fn discriminators_are_anchor_default() {
    // A changed struct name or a custom discriminator would silently break every consumer.
    assert_eq!(
        PlatformConfig::DISCRIMINATOR,
        expected_discriminator("PlatformConfig")
    );
    assert_eq!(
        RoleMember::DISCRIMINATOR,
        expected_discriminator("RoleMember")
    );
    assert_eq!(
        GameEntry::DISCRIMINATOR,
        expected_discriminator("GameEntry")
    );
    assert_eq!(
        AllowedMint::DISCRIMINATOR,
        expected_discriminator("AllowedMint")
    );
    assert_eq!(Treasury::DISCRIMINATOR, expected_discriminator("Treasury"));
    assert_eq!(
        DefaultAd::DISCRIMINATOR,
        expected_discriminator("DefaultAd")
    );
}

// ------------------------------------------------------------- enum bytes
//
// `borsh(use_discriminant = true)` writes each variant's *explicit value*; IDL /
// `declare_program!` consumers write the *variant index*. They must be the same byte.
// The arrays below list variants in DECLARATION ORDER and the expected byte is the
// array position — never a hand-typed number — so a variant whose value differs from
// its index (e.g. `Retired = 5` declared 3rd) fails the test.

/// v1 CONTRACT — golden bytes of every variant that exists at first deploy.
/// ⚠️ Never edit or reorder these rows, only append. Renumbering an existing variant
/// would orphan every `RoleMember` PDA of that role (the byte is a seed) and break
/// every consumer decoder — even if value == index still holds.
const ROLE_GOLDEN_V1: &[(Role, u8)] = &[
    (Role::Admin, 0),
    (Role::GameRegistrar, 1),
    (Role::Moderator, 2),
    (Role::FeeManager, 3),
    (Role::AllowlistManager, 4),
];
const GAME_STATUS_GOLDEN_V1: &[(GameStatus, u8)] =
    &[(GameStatus::Active, 0), (GameStatus::Suspended, 1)];
const FEE_KIND_GOLDEN_V1: &[(FeeKind, u8)] = &[(FeeKind::Boost, 0), (FeeKind::Ad, 1)];

#[test]
fn enum_bytes_frozen_v1() {
    for &(r, byte) in ROLE_GOLDEN_V1 {
        assert_eq!(to_bytes(&r), vec![byte], "Borsh byte of {r:?}");
        assert_eq!(r as u8, byte, "PDA seed byte of {r:?}");
    }
    for &(s, byte) in GAME_STATUS_GOLDEN_V1 {
        assert_eq!(
            to_bytes(&s),
            vec![byte],
            "{s:?} (relayer memcmp: Active = 0x00)"
        );
    }
    for &(k, byte) in FEE_KIND_GOLDEN_V1 {
        assert_eq!(to_bytes(&k), vec![byte], "{k:?}");
    }
}

const ROLES: [Role; 5] = [
    Role::Admin,
    Role::GameRegistrar,
    Role::Moderator,
    Role::FeeManager,
    Role::AllowlistManager,
];
const GAME_STATUSES: [GameStatus; 2] = [GameStatus::Active, GameStatus::Suspended];
const FEE_KINDS: [FeeKind; 2] = [FeeKind::Boost, FeeKind::Ad];

/// Compile-time trip-wire: adding a variant breaks these exhaustive matches, forcing
/// the author to also append it (in declaration order) to the arrays above.
#[allow(dead_code)]
fn enum_tripwire(r: Role, s: GameStatus, k: FeeKind) {
    match r {
        Role::Admin
        | Role::GameRegistrar
        | Role::Moderator
        | Role::FeeManager
        | Role::AllowlistManager => {}
    }
    match s {
        GameStatus::Active | GameStatus::Suspended => {}
    }
    match k {
        FeeKind::Boost | FeeKind::Ad => {}
    }
}

#[test]
fn enum_byte_equals_declaration_index() {
    for (i, r) in ROLES.iter().enumerate() {
        assert_eq!(to_bytes(r), vec![i as u8], "Borsh byte of {r:?}");
        assert_eq!(*r as u8, i as u8, "PDA seed byte of {r:?}");
    }
    for (i, s) in GAME_STATUSES.iter().enumerate() {
        assert_eq!(to_bytes(s), vec![i as u8], "{s:?}");
    }
    for (i, k) in FEE_KINDS.iter().enumerate() {
        assert_eq!(to_bytes(k), vec![i as u8], "{k:?}");
    }
}

// ---------------------------------------------------------------- offsets

#[test]
fn platform_config_offsets() {
    let b = serialize(&PlatformConfig {
        version: 1,
        bump: 0xFE,
        boost_fee_bps: 100,
        ad_fee_bps: 0x1234,
        admin_count: 0xBEEF,
        treasury_bump: 0xFD,
        initialized_at: 0x0102_0304_0506_0708,
        updated_at: -42,
        _reserved: [0xAA; 64],
    });
    assert_eq!(b.len(), 97);
    assert_eq!(&b[..8], PlatformConfig::DISCRIMINATOR);
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xFE);
    assert_eq!(u16_at(&b, 10), 100);
    assert_eq!(u16_at(&b, 12), 0x1234);
    assert_eq!(u16_at(&b, 14), 0xBEEF);
    assert_eq!(b[16], 0xFD);
    assert_eq!(i64_at(&b, 17), 0x0102_0304_0506_0708);
    assert_eq!(i64_at(&b, 25), -42);
    assert!(b[33..97].iter().all(|&x| x == 0xAA));
}

#[test]
fn role_member_offsets() {
    let b = serialize(&RoleMember {
        version: 1,
        bump: 0xFE,
        role: Role::FeeManager,
        member: pk(0x11),
        granted_by: pk(0x22),
        granted_at: 7,
        _reserved: [0xAA; 32],
    });
    assert_eq!(b.len(), 115);
    assert_eq!(&b[..8], RoleMember::DISCRIMINATOR);
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xFE);
    assert_eq!(b[10], Role::FeeManager as u8);
    assert_eq!(pk_at(&b, 11), pk(0x11));
    assert_eq!(pk_at(&b, 43), pk(0x22));
    assert_eq!(i64_at(&b, 75), 7);
    assert!(b[83..115].iter().all(|&x| x == 0xAA));
}

#[test]
fn game_entry_offsets_including_relayer_memcmp() {
    let b = serialize(&GameEntry {
        version: 1,
        bump: 0xFE,
        status: GameStatus::Active,
        game_program: pk(0x11),
        owner: pk(0x22),
        game_authority: pk(0x33),
        game_authority_bump: 0xFD,
        registered_by: pk(0x44),
        registered_at: 5,
        updated_at: 6,
        _reserved: [0xAA; 64],
    });
    assert_eq!(b.len(), 220, "relayer filters with dataSize = 220");
    assert_eq!(&b[..8], GameEntry::DISCRIMINATOR, "relayer memcmp offset 0");
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xFE);
    assert_eq!(b[10], 0x00, "relayer memcmp offset 10 = Active");
    assert_eq!(pk_at(&b, 11), pk(0x11), "relayer dataSlice(11, 32)");
    assert_eq!(pk_at(&b, 43), pk(0x22));
    assert_eq!(pk_at(&b, 75), pk(0x33));
    assert_eq!(b[107], 0xFD);
    assert_eq!(pk_at(&b, 108), pk(0x44));
    assert_eq!(i64_at(&b, 140), 5);
    assert_eq!(i64_at(&b, 148), 6);
    assert!(b[156..220].iter().all(|&x| x == 0xAA));

    let suspended = serialize(&GameEntry {
        status: GameStatus::Suspended,
        ..game_entry_zero()
    });
    assert_eq!(suspended[10], 0x01);
}

fn game_entry_zero() -> GameEntry {
    GameEntry {
        version: 1,
        bump: 0,
        status: GameStatus::Active,
        game_program: Pubkey::default(),
        owner: Pubkey::default(),
        game_authority: Pubkey::default(),
        game_authority_bump: 0,
        registered_by: Pubkey::default(),
        registered_at: 0,
        updated_at: 0,
        _reserved: [0; 64],
    }
}

#[test]
fn allowed_mint_offsets() {
    let b = serialize(&AllowedMint {
        version: 1,
        bump: 0xFE,
        mint: pk(0x11),
        token_program: pk(0x22),
        decimals: 9,
        added_by: pk(0x33),
        added_at: 8,
        _reserved: [0xAA; 32],
    });
    assert_eq!(b.len(), 147);
    assert_eq!(&b[..8], AllowedMint::DISCRIMINATOR);
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xFE);
    assert_eq!(pk_at(&b, 10), pk(0x11));
    assert_eq!(pk_at(&b, 42), pk(0x22));
    assert_eq!(b[74], 9);
    assert_eq!(pk_at(&b, 75), pk(0x33));
    assert_eq!(i64_at(&b, 107), 8);
    assert!(b[115..147].iter().all(|&x| x == 0xAA));
}

#[test]
fn treasury_offsets() {
    let b = serialize(&Treasury {
        version: 1,
        bump: 0xFE,
        _reserved: [0xAA; 32],
    });
    assert_eq!(b.len(), 42);
    assert_eq!(&b[..8], Treasury::DISCRIMINATOR);
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xFE);
    assert!(b[10..42].iter().all(|&x| x == 0xAA));
}

fn default_ad(creative: AdCreative) -> DefaultAd {
    DefaultAd {
        version: 1,
        bump: 0xFE,
        width: 300,
        height: 250,
        enabled: true,
        updated_by: pk(0x11),
        updated_at: 9,
        creative,
        _reserved: [0xAA; 32],
    }
}

#[test]
fn default_ad_fixed_head_offsets() {
    let creative = AdCreative {
        image_url: "https://r2.example/ads/x.png".into(),
        image_sha256: [0x5A; 32],
        text: "สวัสดี".into(),
        click_url: "https://example.com".into(),
    };
    let b = serialize(&default_ad(creative.clone()));
    assert_eq!(&b[..8], DefaultAd::DISCRIMINATOR);
    assert_eq!(b[8], 1);
    assert_eq!(b[9], 0xFE);
    assert_eq!(u16_at(&b, 10), 300);
    assert_eq!(u16_at(&b, 12), 250);
    assert_eq!(b[14], 1);
    assert_eq!(pk_at(&b, 15), pk(0x11));
    assert_eq!(i64_at(&b, 47), 9);
    // creative starts at 55 — everything after is variable-offset (review R3)
    let creative_bytes = to_bytes(&creative);
    assert_eq!(&b[55..55 + creative_bytes.len()], &creative_bytes[..]);
    assert!(b[55 + creative_bytes.len()..].iter().all(|&x| x == 0xAA));
}

#[test]
fn default_ad_max_length_fills_reserved_space_exactly() {
    let max = AdCreative {
        image_url: "a".repeat(AD_IMAGE_URL_MAX_LEN as usize),
        image_sha256: [1; 32],
        text: "b".repeat(AD_TEXT_MAX_LEN as usize),
        click_url: "c".repeat(AD_CLICK_URL_MAX_LEN as usize),
    };
    let b = serialize(&default_ad(max));
    assert_eq!(b.len(), DISC + DefaultAd::INIT_SPACE);
}

#[test]
fn ad_creative_limits_match_constants() {
    // Each field's `#[max_len]` is bound to its own constant (default_ad.rs); this pins
    // the constants' values to ad-pool's limits (01 §7). The IDL does not carry max_len.
    assert_eq!(AD_IMAGE_URL_MAX_LEN, 200);
    assert_eq!(AD_TEXT_MAX_LEN, 120);
    assert_eq!(AD_CLICK_URL_MAX_LEN, 200);
    let space =
        |img: u16, txt: u16, url: u16| 4 + img as usize + 32 + 4 + txt as usize + 4 + url as usize;
    assert_eq!(
        AdCreative::INIT_SPACE,
        space(AD_IMAGE_URL_MAX_LEN, AD_TEXT_MAX_LEN, AD_CLICK_URL_MAX_LEN)
    );
}

// -------------------------------------------------------------- constants

#[test]
fn seeds_and_limits_match_design() {
    assert_eq!(CONFIG_SEED, b"config");
    assert_eq!(TREASURY_SEED, b"treasury");
    assert_eq!(ROLE_SEED, b"role");
    assert_eq!(GAME_SEED, b"game");
    assert_eq!(BOOST_MINT_SEED, b"boost_mint");
    assert_eq!(DEFAULT_AD_SEED, b"default_ad");
    assert_eq!(GAME_AUTHORITY_SEED, b"game_authority");
    assert_eq!(MAX_FEE_BPS, 2_000);
    assert_eq!(BPS_DENOMINATOR, 10_000);
    assert_eq!(ACCOUNT_VERSION, 1);
}
