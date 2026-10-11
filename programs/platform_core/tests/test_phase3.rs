//! Phase 3 — default ads (02 §1 #14–16 · §2.7) + Q8 Pausable acceptance.
// `TransactionResult` is litesvm's type; its large Err variant is not ours to box.
#![allow(clippy::result_large_err)]
mod common;

use {
    anchor_lang::{Discriminator, Space},
    common::*,
    platform_core::{
        constants::*,
        state::{AllowedMint, DefaultAd, Role},
    },
    solana_signer::Signer,
};

const ACCOUNT_NOT_INITIALIZED: u32 = 3012;
const ACCOUNT_DISCRIMINATOR_MISMATCH: u32 = 3002;
const UNAUTHORIZED: u32 = 6000;
const INVALID_AD_SIZE: u32 = 6006;
const CREATIVE_FIELD_TOO_LONG: u32 = 6007;
const INVALID_CLICK_URL: u32 = 6008;
const INVALID_IMAGE_HASH: u32 = 6009;

#[test]
fn moderator_creates_default_ad_per_size() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let moderator = holder(&mut env, &admin, Role::Moderator);
    set_time(&mut env.svm, 1_760_000_000);

    let ix = ix_create_default_ad(
        &moderator.pubkey(),
        Role::Moderator,
        300,
        250,
        creative("a"),
    );
    assert_ok(&send(&mut env.svm, &[ix], &[&moderator]));

    let pda = default_ad_pda(300, 250);
    let ad: DefaultAd = fetch(&env.svm, &pda);
    assert_eq!(ad.version, ACCOUNT_VERSION);
    assert_eq!((ad.width, ad.height, ad.enabled), (300, 250, true));
    assert_eq!(ad.updated_by, moderator.pubkey());
    assert_eq!(ad.updated_at, 1_760_000_000);
    assert_eq!(ad.creative, creative("a"));
    assert_eq!(ad._reserved, [0u8; 32]);

    // Account is reserved at full size (01 §7) and the fixed head sits where documented.
    let raw = env.svm.get_account(&pda).unwrap();
    assert_eq!(raw.data.len(), 8 + DefaultAd::INIT_SPACE);
    assert_eq!(&raw.data[..8], DefaultAd::DISCRIMINATOR);
    assert_eq!(&raw.data[10..12], &300u16.to_le_bytes());
    assert_eq!(&raw.data[12..14], &250u16.to_le_bytes());
    assert_eq!(raw.data[14], 1, "enabled");
    assert_eq!(&raw.data[15..47], moderator.pubkey().as_ref());

    // A different size is a different account; the same size cannot be created twice.
    let ix = ix_create_default_ad(&moderator.pubkey(), Role::Moderator, 728, 90, creative("b"));
    assert_ok(&send(&mut env.svm, &[ix], &[&moderator]));
    let ix = ix_create_default_ad(
        &moderator.pubkey(),
        Role::Moderator,
        300,
        250,
        creative("c"),
    );
    assert_failed(&send(&mut env.svm, &[ix], &[&moderator]));
}

#[test]
fn admin_may_manage_default_ads_other_roles_may_not() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let ix = ix_create_default_ad(&admin.pubkey(), Role::Admin, 320, 50, creative("x"));
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));

    for role in [
        Role::FeeManager,
        Role::GameRegistrar,
        Role::AllowlistManager,
    ] {
        let h = holder(&mut env, &admin, role);
        let ix = ix_create_default_ad(&h.pubkey(), role, 160, 600, creative("y"));
        assert_custom_err(&send(&mut env.svm, &[ix], &[&h]), UNAUTHORIZED);
        let ix = ix_set_default_ad_enabled(&h.pubkey(), role, &default_ad_pda(320, 50), false);
        assert_custom_err(&send(&mut env.svm, &[ix], &[&h]), UNAUTHORIZED);
    }
    let outsider = funded(&mut env.svm);
    let ix = ix_create_default_ad(&outsider.pubkey(), Role::Moderator, 160, 600, creative("z"));
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&outsider]),
        ACCOUNT_NOT_INITIALIZED,
    );
}

#[test]
fn creative_validation() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let mut n = 0u16;
    let mut try_create = |env: &mut Env, w: u16, h: u16, c| {
        n += 1;
        let ix = ix_create_default_ad(&admin.pubkey(), Role::Admin, w + n, h, c);
        send(&mut env.svm, &[ix], &[&admin])
    };

    let res = try_create(&mut env, 0, 0, creative("ok")); // width = n ≥ 1, height 0
    assert_custom_err(&res, INVALID_AD_SIZE);
    let ix = ix_create_default_ad(&admin.pubkey(), Role::Admin, 0, 90, creative("ok"));
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), INVALID_AD_SIZE);

    let mut c = creative("long");
    c.image_url = format!("https://{}", "a".repeat(AD_IMAGE_URL_MAX_LEN as usize));
    assert_custom_err(&try_create(&mut env, 100, 100, c), CREATIVE_FIELD_TOO_LONG);
    let mut c = creative("long");
    c.click_url = format!("https://{}", "a".repeat(AD_CLICK_URL_MAX_LEN as usize));
    assert_custom_err(&try_create(&mut env, 100, 100, c), CREATIVE_FIELD_TOO_LONG);
    // `text` is limited in BYTES: Thai is 3 bytes per character → 40 chars fit, 41 don't.
    let mut c = creative("thai");
    c.text = "ก".repeat(40);
    assert_eq!(c.text.len(), 120);
    assert_ok(&try_create(&mut env, 100, 100, c));
    let mut c = creative("thai");
    c.text = "ก".repeat(41);
    assert_custom_err(&try_create(&mut env, 100, 100, c), CREATIVE_FIELD_TOO_LONG);

    // Exactly at every limit fits the reserved space.
    let max = platform_core::state::AdCreative {
        image_url: "a".repeat(AD_IMAGE_URL_MAX_LEN as usize),
        image_sha256: [1; 32],
        text: "b".repeat(AD_TEXT_MAX_LEN as usize),
        click_url: format!(
            "https://{}",
            "c".repeat(AD_CLICK_URL_MAX_LEN as usize - "https://".len())
        ),
    };
    assert_ok(&try_create(&mut env, 100, 100, max));

    for bad in ["http://hub.example", "", "javascript:alert(1)", "HTTPS://x"] {
        let mut c = creative("url");
        c.click_url = bad.into();
        assert_custom_err(&try_create(&mut env, 100, 100, c), INVALID_CLICK_URL);
    }
    let mut c = creative("hash");
    c.image_sha256 = [0; 32];
    assert_custom_err(&try_create(&mut env, 100, 100, c), INVALID_IMAGE_HASH);
}

#[test]
fn update_and_toggle_keep_the_account() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let moderator = holder(&mut env, &admin, Role::Moderator);
    let ix = ix_create_default_ad(&admin.pubkey(), Role::Admin, 300, 250, creative("v1"));
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
    let pda = default_ad_pda(300, 250);

    let ix = ix_update_default_ad(&moderator.pubkey(), Role::Moderator, &pda, creative("v2"));
    assert_ok(&send(&mut env.svm, &[ix], &[&moderator]));
    let ad: DefaultAd = fetch(&env.svm, &pda);
    assert_eq!(ad.creative, creative("v2"));
    assert_eq!(ad.updated_by, moderator.pubkey());

    // Update validates too.
    let mut bad = creative("v3");
    bad.click_url = "http://x".into();
    let ix = ix_update_default_ad(&moderator.pubkey(), Role::Moderator, &pda, bad);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&moderator]), INVALID_CLICK_URL);

    let ix = ix_set_default_ad_enabled(&moderator.pubkey(), Role::Moderator, &pda, false);
    assert_ok(&send(&mut env.svm, &[ix], &[&moderator]));
    let ad: DefaultAd = fetch(&env.svm, &pda);
    assert!(!ad.enabled);
    assert_eq!(ad.creative, creative("v2"), "creative kept while disabled");
    // Updating while disabled keeps it disabled.
    let ix = ix_update_default_ad(&moderator.pubkey(), Role::Moderator, &pda, creative("v4"));
    assert_ok(&send(&mut env.svm, &[ix], &[&moderator]));
    assert!(!fetch::<DefaultAd>(&env.svm, &pda).enabled);

    let ix = ix_set_default_ad_enabled(&admin.pubkey(), Role::Admin, &pda, true);
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
    assert!(fetch::<DefaultAd>(&env.svm, &pda).enabled);
}

#[test]
fn update_rejects_other_platform_accounts() {
    let mut env = setup();
    let admin = init_default(&mut env);
    for wrong in [
        config_pda(),
        treasury_pda(),
        role_pda(Role::Admin, &admin.pubkey()),
    ] {
        let ix = ix_update_default_ad(&admin.pubkey(), Role::Admin, &wrong, creative("x"));
        assert_custom_err(
            &send(&mut env.svm, &[ix], &[&admin]),
            ACCOUNT_DISCRIMINATOR_MISMATCH,
        );
        let ix = ix_set_default_ad_enabled(&admin.pubkey(), Role::Admin, &wrong, false);
        assert_custom_err(
            &send(&mut env.svm, &[ix], &[&admin]),
            ACCOUNT_DISCRIMINATOR_MISMATCH,
        );
    }
}

#[test]
fn pausable_mint_is_allowed_q8() {
    // Decision 2026-10-09: Pausable behaves like a freeze authority (can stop, cannot steal).
    let mut env = setup();
    let admin = init_default(&mut env);
    for decimals in [6u8, 0u8] {
        let mint = create_mint(
            &mut env.svm,
            &admin,
            &TOKEN_2022_PROGRAM,
            decimals,
            &[TestExt::Pausable],
        );
        let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_2022_PROGRAM);
        assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
        let am: AllowedMint = fetch(&env.svm, &allowed_mint_pda(&mint));
        assert_eq!(am.mint, mint);
    }
}
