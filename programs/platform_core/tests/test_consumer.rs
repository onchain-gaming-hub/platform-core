//! Phase 3 — the 03 consumer snippets, executed by a real consumer program
//! (`tests/mock-consumer`, built from `idls/platform_core.json` via `declare_program!`).
//! Requires: `anchor build` + `cargo build-sbf --manifest-path tests/mock-consumer/Cargo.toml
//! --sbf-out-dir target/deploy` (see scripts/test.sh).
// `TransactionResult` is litesvm's type; its large Err variant is not ours to box.
#![allow(clippy::result_large_err)]
mod common;

use {
    anchor_lang::{prelude::Pubkey, solana_program::system_instruction},
    common::*,
    platform_core::state::{GameStatus, Role},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

// mock_consumer::MockError (6000 + index) — distinct program from platform-core.
const MOCK_GAME_NOT_ACTIVE: u32 = 6000;
const MOCK_NOT_GAME_AUTHORITY: u32 = 6001;
const MOCK_NOT_GAME_OWNER: u32 = 6002;
// Anchor framework errors.
const CONSTRAINT_SEEDS: u32 = 2006;
const ACCOUNT_NOT_INITIALIZED: u32 = 3012;
const ACCOUNT_NOT_SIGNER: u32 = 3010;
const CONSTRAINT_ASSOCIATED: u32 = 2009;
const CONSTRAINT_TOKEN_OWNER: u32 = 2015;

/// platform-core initialized, mock consumer loaded and registered as a game (Active).
struct World {
    env: Env,
    admin: Keypair,
    owner: Keypair,
}

fn world() -> World {
    let mut env = setup();
    load_mock_consumer(&mut env.svm);
    let admin = init_default(&mut env);
    let owner = funded(&mut env.svm);
    let ix = ix_register_game(
        &admin.pubkey(),
        Role::Admin,
        &owner.pubkey(),
        &mock_consumer::ID,
    );
    assert_ok(&send(&mut env.svm, &[ix], &[&admin, &owner]));
    World { env, admin, owner }
}

fn suspend(w: &mut World) {
    let ix = ix_set_game_status(
        &w.admin.pubkey(),
        Role::Admin,
        &mock_consumer::ID,
        GameStatus::Suspended,
    );
    assert_ok(&send(&mut w.env.svm, &[ix], &[&w.admin]));
}

// ======================================================= §2.1 + §2.2 + §2.5

#[test]
fn game_cpi_with_pda_signer_opens_pool_and_snapshots_fees() {
    let mut w = world();
    let ix = ix_set_fee(
        &w.admin.pubkey(),
        Role::Admin,
        platform_core::state::FeeKind::Ad,
        250,
    );
    assert_ok(&send(&mut w.env.svm, &[ix], &[&w.admin]));
    let payer = funded(&mut w.env.svm);

    assert_ok(&send(
        &mut w.env.svm,
        &[mix_game_open_pool(&payer.pubkey())],
        &[&payer],
    ));
    let snap: mock_consumer::Snapshot = fetch(&w.env.svm, &mock_snapshot_pda(&mock_consumer::ID));
    assert_eq!(snap.game_program, mock_consumer::ID);
    assert_eq!((snap.boost_fee_bps, snap.ad_fee_bps), (100, 250));

    // GameEntry.game_authority (X5) is exactly the PDA the game signs with.
    let entry: platform_core::state::GameEntry = fetch(&w.env.svm, &game_pda(&mock_consumer::ID));
    assert_eq!(entry.game_authority, mock_game_authority());
}

#[test]
fn direct_call_without_game_pda_signer_is_rejected() {
    // §2.2: only the game program (via its PDA signer) may start a pool.
    let mut w = world();
    let attacker = funded(&mut w.env.svm);
    let ix = mix_open_pool_direct(&attacker.pubkey(), &attacker.pubkey());
    assert_custom_err(
        &send(&mut w.env.svm, &[ix], &[&attacker]),
        MOCK_NOT_GAME_AUTHORITY,
    );
}

#[test]
fn suspended_or_unregistered_game_cannot_start_new_work() {
    let mut w = world();
    suspend(&mut w);
    let payer = funded(&mut w.env.svm);
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_game_open_pool(&payer.pubkey())],
            &[&payer],
        ),
        MOCK_GAME_NOT_ACTIVE,
    );
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_owner_action(&mock_consumer::ID, &w.owner.pubkey())],
            &[&w.owner],
        ),
        MOCK_GAME_NOT_ACTIVE,
    );

    // A game platform-core never registered.
    let unregistered = deploy_game(&mut w.env.svm);
    let owner = w.owner.insecure_clone();
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_owner_action(&unregistered, &owner.pubkey())],
            &[&owner],
        ),
        ACCOUNT_NOT_INITIALIZED,
    );
}

#[test]
fn game_entry_of_another_game_cannot_be_substituted() {
    // §2.1 seeds bind the GameEntry to *this* game_program.
    let mut w = world();
    let other = deploy_game(&mut w.env.svm);
    let other_owner = funded(&mut w.env.svm);
    let ix = ix_register_game(
        &w.admin.pubkey(),
        Role::Admin,
        &other_owner.pubkey(),
        &other,
    );
    assert_ok(&send(&mut w.env.svm, &[ix], &[&w.admin, &other_owner]));

    let mut ix = mix_owner_action(&mock_consumer::ID, &other_owner.pubkey());
    replace_account(&mut ix, 1, game_pda(&other)); // game_entry
    assert_custom_err(
        &send(&mut w.env.svm, &[ix], &[&other_owner]),
        CONSTRAINT_SEEDS,
    );
}

// ================================================================== §2.3

#[test]
fn only_the_registered_owner_passes_the_owner_check() {
    let mut w = world();
    let owner = w.owner.insecure_clone();
    assert_ok(&send(
        &mut w.env.svm,
        &[mix_owner_action(&mock_consumer::ID, &owner.pubkey())],
        &[&owner],
    ));
    let impostor = funded(&mut w.env.svm);
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_owner_action(&mock_consumer::ID, &impostor.pubkey())],
            &[&impostor],
        ),
        MOCK_NOT_GAME_OWNER,
    );
}

// ================================================================== §2.4

#[test]
fn moderator_role_is_checked_exactly() {
    let mut w = world();
    let admin = w.admin.insecure_clone();
    let moderator = holder(&mut w.env, &admin, Role::Moderator);
    assert_ok(&send(
        &mut w.env.svm,
        &[mix_moderate(
            &moderator.pubkey(),
            &role_pda(Role::Moderator, &moderator.pubkey()),
        )],
        &[&moderator],
    ));

    // X4: Admin is NOT a moderator for consumers — it holds no Moderator PDA.
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_moderate(
                &admin.pubkey(),
                &role_pda(Role::Moderator, &admin.pubkey()),
            )],
            &[&admin],
        ),
        ACCOUNT_NOT_INITIALIZED,
    );
    // Admin passing its Admin PDA instead → seeds mismatch.
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_moderate(
                &admin.pubkey(),
                &role_pda(Role::Admin, &admin.pubkey()),
            )],
            &[&admin],
        ),
        CONSTRAINT_SEEDS,
    );
    // Someone else using the real moderator's PDA → seeds mismatch.
    let thief = funded(&mut w.env.svm);
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_moderate(
                &thief.pubkey(),
                &role_pda(Role::Moderator, &moderator.pubkey()),
            )],
            &[&thief],
        ),
        CONSTRAINT_SEEDS,
    );
    // Revoked → fail closed.
    let ix = ix_revoke_role(
        &admin.pubkey(),
        Role::Moderator,
        &moderator.pubkey(),
        &admin.pubkey(),
    );
    assert_ok(&send(&mut w.env.svm, &[ix], &[&admin]));
    assert_custom_err(
        &send(
            &mut w.env.svm,
            &[mix_moderate(
                &moderator.pubkey(),
                &role_pda(Role::Moderator, &moderator.pubkey()),
            )],
            &[&moderator],
        ),
        ACCOUNT_NOT_INITIALIZED,
    );
}

// ======================================================== §2.6 + §3 (SOL)

fn fund_escrow(w: &mut World, lamports: u64) {
    let payer = funded(&mut w.env.svm);
    assert_ok(&send(
        &mut w.env.svm,
        &[mix_init_escrow(&payer.pubkey())],
        &[&payer],
    ));
    let ix = system_instruction::transfer(&payer.pubkey(), &mock_escrow_pda(), lamports);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&payer]));
}

#[test]
fn user_pays_sol_fee_into_treasury_and_fake_treasury_is_rejected() {
    let mut w = world();
    let user = funded(&mut w.env.svm);
    let before = lamports(&w.env.svm, &treasury_pda());
    let ix = mix_pay_fee_from_user(&user.pubkey(), &treasury_pda(), 12_345);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&user]));
    assert_eq!(lamports(&w.env.svm, &treasury_pda()), before + 12_345);

    let fake = Pubkey::new_unique();
    let ix = mix_pay_fee_from_user(&user.pubkey(), &fake, 1);
    assert_custom_err(&send(&mut w.env.svm, &[ix], &[&user]), CONSTRAINT_SEEDS);
}

#[test]
fn settle_from_escrow_splits_fee_with_floor_and_works_while_suspended() {
    let mut w = world();
    fund_escrow(&mut w, 3 * SOL);
    // Rule 3: settling something already started must work even when Suspended.
    suspend(&mut w);

    let t0 = lamports(&w.env.svm, &treasury_pda());
    let o0 = lamports(&w.env.svm, &w.owner.pubkey());
    let e0 = lamports(&w.env.svm, &mock_escrow_pda());
    let payer = funded(&mut w.env.svm);
    // 1 SOL at 1% (DECISIONS rev 7) → 0.01 SOL fee / 0.99 SOL to the owner.
    let ix = mix_settle(&mock_consumer::ID, &w.owner.pubkey(), SOL, 100);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&payer]));
    assert_eq!(lamports(&w.env.svm, &treasury_pda()), t0 + SOL / 100);
    assert_eq!(
        lamports(&w.env.svm, &w.owner.pubkey()),
        o0 + SOL - SOL / 100
    );
    assert_eq!(lamports(&w.env.svm, &mock_escrow_pda()), e0 - SOL);

    // §3 floor: 199 lamports at 1% → fee 1 (not 1.99), owner 198.
    let t1 = lamports(&w.env.svm, &treasury_pda());
    let o1 = lamports(&w.env.svm, &w.owner.pubkey());
    let ix = mix_settle(&mock_consumer::ID, &w.owner.pubkey(), 199, 100);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&payer]));
    assert_eq!(lamports(&w.env.svm, &treasury_pda()), t1 + 1);
    assert_eq!(lamports(&w.env.svm, &w.owner.pubkey()), o1 + 198);
}

#[test]
fn settle_pays_the_owner_recorded_at_settle_time() {
    // rev 3 #6: ad revenue goes to GameEntry.owner as read at settle.
    let mut w = world();
    fund_escrow(&mut w, SOL);
    let new_owner = funded(&mut w.env.svm);
    let ix = ix_transfer_game_owner(&w.owner.pubkey(), &new_owner.pubkey(), &mock_consumer::ID);
    let owner = w.owner.insecure_clone();
    assert_ok(&send(&mut w.env.svm, &[ix], &[&owner, &new_owner]));

    let payer = funded(&mut w.env.svm);
    let ix = mix_settle(&mock_consumer::ID, &owner.pubkey(), 1_000, 100);
    assert_custom_err(&send(&mut w.env.svm, &[ix], &[&payer]), MOCK_NOT_GAME_OWNER);
    let n0 = lamports(&w.env.svm, &new_owner.pubkey());
    let ix = mix_settle(&mock_consumer::ID, &new_owner.pubkey(), 1_000, 100);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&payer]));
    assert_eq!(lamports(&w.env.svm, &new_owner.pubkey()), n0 + 990);
}

#[test]
fn typed_account_treasury_variant_also_works() {
    // 03 §2.6 question resolved: typing the Treasury as `Account<Treasury>` (mut) in a
    // consumer is safe — Anchor 1.2.1 only writes an `Account<T>` back on exit when the
    // current program owns it (anchor-lang src/accounts/account.rs:261), so the foreign
    // Treasury's data is never touched; only lamports move.
    let mut w = world();
    fund_escrow(&mut w, SOL);
    let payer = funded(&mut w.env.svm);
    let t0 = lamports(&w.env.svm, &treasury_pda());
    let before: platform_core::state::Treasury = fetch(&w.env.svm, &treasury_pda());
    assert_ok(&send(
        &mut w.env.svm,
        &[mix_settle_typed_treasury(500)],
        &[&payer],
    ));
    assert_eq!(lamports(&w.env.svm, &treasury_pda()), t0 + 500);
    let after: platform_core::state::Treasury = fetch(&w.env.svm, &treasury_pda());
    assert_eq!((after.version, after.bump), (before.version, before.bump));
}

// ====================================================== §2.6 (token fee)

#[test]
fn boost_needs_allowlist_but_collect_fee_does_not() {
    // 03 §2.6 split (Fable round 1): `boost` checks AllowedMint; `collect_fee` must NOT —
    // a mint disallowed after the boost must still pay its fee into the Treasury ATA.
    let mut w = world();
    let admin = w.admin.insecure_clone();
    for tp in [TOKEN_PROGRAM, TOKEN_2022_PROGRAM] {
        let mint = create_mint(&mut w.env.svm, &admin, &tp, 6, &[]);
        let user = funded(&mut w.env.svm);
        let user_ata = create_ata(&mut w.env.svm, &user, &user.pubkey(), &mint, &tp);
        mint_to(&mut w.env.svm, &admin, &mint, &user_ata, &tp, 10_000, 6);
        let payer = funded(&mut w.env.svm);

        // Not allowlisted → boost refused (fail closed).
        assert_custom_err(
            &send(&mut w.env.svm, &[mix_boost_check(&mint)], &[&payer]),
            ACCOUNT_NOT_INITIALIZED,
        );

        let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &tp);
        assert_ok(&send(&mut w.env.svm, &[ix], &[&admin]));
        assert_ok(&send(&mut w.env.svm, &[mix_boost_check(&mint)], &[&payer]));
        // X3: no init_if_needed on the consumer side — the ATA already exists.
        let ix = mix_collect_fee_token(&user.pubkey(), &mint, &tp, &user_ata, 100);
        assert_ok(&send(&mut w.env.svm, &[ix], &[&user]));
        let t_ata = ata(&treasury_pda(), &mint, &tp);
        assert_eq!(token_balance(&w.env.svm, &t_ata), 100);

        // Disallow: new boosts refused, but the pending fee is still collectable.
        let ix = ix_disallow_mint(&admin.pubkey(), Role::Admin, &mint, &admin.pubkey());
        assert_ok(&send(&mut w.env.svm, &[ix], &[&admin]));
        assert_custom_err(
            &send(&mut w.env.svm, &[mix_boost_check(&mint)], &[&payer]),
            ACCOUNT_NOT_INITIALIZED,
        );
        let ix = mix_collect_fee_token(&user.pubkey(), &mint, &tp, &user_ata, 50);
        assert_ok(&send(&mut w.env.svm, &[ix], &[&user]));
        assert_eq!(token_balance(&w.env.svm, &t_ata), 150);

        // End to end: the platform Admin withdraws what the consumer paid in.
        let dest = create_ata(&mut w.env.svm, &admin, &admin.pubkey(), &mint, &tp);
        let ix = ix_withdraw_token(&admin.pubkey(), &mint, &tp, &dest, 150);
        assert_ok(&send(&mut w.env.svm, &[ix], &[&admin]));
        assert_eq!(token_balance(&w.env.svm, &dest), 150);
    }
}

/// Sets up an allowlisted mint and a funded user for the token negative tests.
fn token_setup(w: &mut World, tp: &Pubkey) -> (Pubkey, Keypair, Pubkey) {
    let admin = w.admin.insecure_clone();
    let mint = create_mint(&mut w.env.svm, &admin, tp, 6, &[]);
    let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, tp);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&admin]));
    let user = funded(&mut w.env.svm);
    let user_ata = create_ata(&mut w.env.svm, &user, &user.pubkey(), &mint, tp);
    mint_to(&mut w.env.svm, &admin, &mint, &user_ata, tp, 10_000, 6);
    (mint, user, user_ata)
}

// Account indices in mock_consumer::accounts::{BoostCheck, CollectFeeToken}.
const BOOST_ALLOWED_MINT: usize = 1;
const COLLECT_TREASURY: usize = 3;
const COLLECT_TREASURY_ATA: usize = 4;

#[test]
fn spam_mint_cannot_borrow_an_allowlisted_mints_allowed_mint() {
    // (a) AllowedMint seeds bind it to *this* mint.
    let mut w = world();
    let (good_mint, _, _) = token_setup(&mut w, &TOKEN_PROGRAM);
    let admin = w.admin.insecure_clone();
    let spam = create_mint(&mut w.env.svm, &admin, &TOKEN_PROGRAM, 6, &[]);
    let mut ix = mix_boost_check(&spam);
    replace_account(&mut ix, BOOST_ALLOWED_MINT, allowed_mint_pda(&good_mint));
    let payer = funded(&mut w.env.svm);
    assert_custom_err(&send(&mut w.env.svm, &[ix], &[&payer]), CONSTRAINT_SEEDS);
}

#[test]
fn fee_cannot_be_redirected_to_a_non_treasury_token_account() {
    // (b) `associated_token::authority = treasury` rejects any other token account.
    let mut w = world();
    let (mint, user, user_ata) = token_setup(&mut w, &TOKEN_PROGRAM);
    let thief = funded(&mut w.env.svm);
    let thief_ata = create_ata(
        &mut w.env.svm,
        &thief,
        &thief.pubkey(),
        &mint,
        &TOKEN_PROGRAM,
    );

    let mut ix = mix_collect_fee_token(&user.pubkey(), &mint, &TOKEN_PROGRAM, &user_ata, 10);
    replace_account(&mut ix, COLLECT_TREASURY_ATA, thief_ata);
    assert_failed_with_any_of(
        &send(&mut w.env.svm, &[ix], &[&user]),
        &[CONSTRAINT_TOKEN_OWNER, CONSTRAINT_ASSOCIATED],
    );
    assert_eq!(token_balance(&w.env.svm, &thief_ata), 0);
}

#[test]
fn fake_treasury_with_its_own_ata_is_rejected() {
    // (c) the attack the REQUIRED seeds on `treasury` exist for: attacker passes its own
    // wallet as `treasury` and that wallet's ATA as `treasury_ata` (consistent pair).
    let mut w = world();
    let (mint, user, user_ata) = token_setup(&mut w, &TOKEN_2022_PROGRAM);
    let thief = funded(&mut w.env.svm);
    let thief_ata = create_ata(
        &mut w.env.svm,
        &thief,
        &thief.pubkey(),
        &mint,
        &TOKEN_2022_PROGRAM,
    );

    let mut ix = mix_collect_fee_token(&user.pubkey(), &mint, &TOKEN_2022_PROGRAM, &user_ata, 10);
    replace_account(&mut ix, COLLECT_TREASURY, thief.pubkey());
    replace_account(&mut ix, COLLECT_TREASURY_ATA, thief_ata);
    assert_custom_err(&send(&mut w.env.svm, &[ix], &[&user]), CONSTRAINT_SEEDS);
    assert_eq!(token_balance(&w.env.svm, &thief_ata), 0);
}

#[test]
fn real_game_authority_pda_without_its_signature_is_rejected() {
    // §2.2: passing the right PDA address is not enough — it must have signed (only the
    // game program can produce that signature).
    let mut w = world();
    let attacker = funded(&mut w.env.svm);
    let mut ix = mix_open_pool_direct(&attacker.pubkey(), &mock_game_authority());
    unsign(&mut ix, &mock_game_authority());
    assert_custom_err(
        &send(&mut w.env.svm, &[ix], &[&attacker]),
        ACCOUNT_NOT_SIGNER,
    );
}

fn assert_failed_with_any_of(res: &litesvm::types::TransactionResult, codes: &[u32]) {
    match res {
        Ok(_) => panic!("expected one of {codes:?}, tx succeeded"),
        Err(e) => {
            let s = format!("{:?}", e.err);
            assert!(
                codes.iter().any(|c| s.contains(&format!("Custom({c})"))),
                "expected one of {codes:?}, got {s}"
            );
        }
    }
}

#[test]
fn settle_pays_a_program_owned_owner_k4() {
    // K-4 / ad-pool A29: if GameEntry.owner is not system-owned (e.g. an Admin
    // force_set_game_owner mistake), settle must still go through — the snippet uses
    // `UncheckedAccount` + `address`, not `SystemAccount`. Money still only reaches the
    // pinned owner address.
    let mut w = world();
    fund_escrow(&mut w, SOL);
    let admin = w.admin.insecure_clone();
    let program_owned = config_pda(); // owned by platform-core, has data
    let ix = ix_force_set_game_owner(&admin.pubkey(), &mock_consumer::ID, &program_owned);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&admin]));

    let o0 = lamports(&w.env.svm, &program_owned);
    let payer = funded(&mut w.env.svm);
    let ix = mix_settle(&mock_consumer::ID, &program_owned, 1_000, 100);
    assert_ok(&send(&mut w.env.svm, &[ix], &[&payer]));
    assert_eq!(lamports(&w.env.svm, &program_owned), o0 + 990);
    // The credited account's data is untouched.
    let cfg: platform_core::state::PlatformConfig = fetch(&w.env.svm, &config_pda());
    assert_eq!(cfg.version, platform_core::constants::ACCOUNT_VERSION);

    // Still pinned: any other destination is rejected.
    let other = funded(&mut w.env.svm).pubkey();
    let ix = mix_settle(&mock_consumer::ID, &other, 1_000, 100);
    assert_custom_err(&send(&mut w.env.svm, &[ix], &[&payer]), MOCK_NOT_GAME_OWNER);
}
