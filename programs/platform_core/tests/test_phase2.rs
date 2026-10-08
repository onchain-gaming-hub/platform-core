//! Phase 2 — game registry · boost mint allowlist · treasury withdraw (specs/designs/02 §5).
mod common;

use {
    anchor_lang::{prelude::Pubkey, solana_program::system_instruction, Discriminator, Space},
    common::*,
    platform_core::{
        constants::*,
        state::{AllowedMint, GameEntry, GameStatus, Role, Treasury},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const ACCOUNT_NOT_SIGNER: u32 = 3010;
const ACCOUNT_NOT_INITIALIZED: u32 = 3012;
const CONSTRAINT_ADDRESS: u32 = 2012;
const UNAUTHORIZED: u32 = 6000;
const GAME_PROGRAM_NOT_EXECUTABLE: u32 = 6003;
const INSUFFICIENT_TREASURY: u32 = 6004;
const UNSUPPORTED_MINT_EXTENSION: u32 = 6005;
const INVALID_MEMBER: u32 = 6011;
const INVALID_GAME_PROGRAM: u32 = 6012;
const CONSTRAINT_TOKEN_MINT: u32 = 2014;
const CONSTRAINT_TOKEN_OWNER: u32 = 2015;
const CONSTRAINT_MINT_TOKEN_PROGRAM: u32 = 2022;
const ACCOUNT_DISCRIMINATOR_MISMATCH: u32 = 3002;
const ACCOUNT_OWNED_BY_WRONG_PROGRAM: u32 = 3007;

// ============================================================== registry

fn register(env: &mut Env, admin: &Keypair, owner: &Keypair, game: &Pubkey) {
    let ix = ix_register_game(&admin.pubkey(), Role::Admin, &owner.pubkey(), game);
    assert_ok(&send(&mut env.svm, &[ix], &[admin, owner]));
}

#[test]
fn registrar_registers_game_with_owner_cosign() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let registrar = holder(&mut env, &admin, Role::GameRegistrar);
    let owner = funded(&mut env.svm);
    set_time(&mut env.svm, 1_750_000_000);

    let ix = ix_register_game(
        &registrar.pubkey(),
        Role::GameRegistrar,
        &owner.pubkey(),
        &game,
    );
    assert_ok(&send(&mut env.svm, &[ix], &[&registrar, &owner]));

    let entry: GameEntry = fetch(&env.svm, &game_pda(&game));
    let (ga, ga_bump) = Pubkey::find_program_address(&[GAME_AUTHORITY_SEED], &game);
    assert_eq!(entry.version, ACCOUNT_VERSION);
    assert_eq!(entry.status, GameStatus::Active);
    assert_eq!(entry.game_program, game);
    assert_eq!(entry.owner, owner.pubkey());
    assert_eq!(
        entry.game_authority, ga,
        "X5: matches the game's PDA signer"
    );
    assert_eq!(entry.game_authority_bump, ga_bump);
    assert_eq!(entry.registered_by, registrar.pubkey());
    assert_eq!(entry.registered_at, 1_750_000_000);
    assert_eq!(entry._reserved, [0u8; 64]);

    // Relayer's getProgramAccounts filters (03 §5) on the real account bytes.
    let raw = env.svm.get_account(&game_pda(&game)).unwrap();
    assert_eq!(raw.data.len(), 220, "dataSize 220");
    assert_eq!(&raw.data[..8], GameEntry::DISCRIMINATOR);
    assert_eq!(raw.data[10], 0x00, "memcmp(10) = Active");
    assert_eq!(&raw.data[11..43], game.as_ref(), "dataSlice(11, 32)");
}

#[test]
fn admin_may_register_as_superuser() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let owner = funded(&mut env.svm);
    register(&mut env, &admin, &owner, &game);
    let entry: GameEntry = fetch(&env.svm, &game_pda(&game));
    assert_eq!(entry.registered_by, admin.pubkey());
}

#[test]
fn register_requires_owner_signature() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let owner = Pubkey::new_unique();
    let mut ix = ix_register_game(&admin.pubkey(), Role::Admin, &owner, &game);
    unsign(&mut ix, &owner);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), ACCOUNT_NOT_SIGNER);
    assert!(is_closed(&env.svm, &game_pda(&game)));
}

#[test]
fn register_rejects_non_executable_program() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let owner = funded(&mut env.svm);
    let not_a_program = funded(&mut env.svm).pubkey(); // plain system account
    let ix = ix_register_game(
        &admin.pubkey(),
        Role::Admin,
        &owner.pubkey(),
        &not_a_program,
    );
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin, &owner]),
        GAME_PROGRAM_NOT_EXECUTABLE,
    );
}

#[test]
fn register_twice_fails_and_other_roles_cannot_register() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let owner = funded(&mut env.svm);
    register(&mut env, &admin, &owner, &game);
    let ix = ix_register_game(&admin.pubkey(), Role::Admin, &owner.pubkey(), &game);
    assert_failed(&send(&mut env.svm, &[ix], &[&admin, &owner]));

    let fm = holder(&mut env, &admin, Role::FeeManager);
    let game2 = deploy_game(&mut env.svm);
    let ix = ix_register_game(&fm.pubkey(), Role::FeeManager, &owner.pubkey(), &game2);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&fm, &owner]), UNAUTHORIZED);
    let outsider = funded(&mut env.svm);
    let ix = ix_register_game(
        &outsider.pubkey(),
        Role::GameRegistrar,
        &owner.pubkey(),
        &game2,
    );
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&outsider, &owner]),
        ACCOUNT_NOT_INITIALIZED,
    );
}

#[test]
fn suspend_and_reactivate_game() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let registrar = holder(&mut env, &admin, Role::GameRegistrar);
    let owner = funded(&mut env.svm);
    register(&mut env, &admin, &owner, &game);

    let ix = ix_set_game_status(
        &registrar.pubkey(),
        Role::GameRegistrar,
        &game,
        GameStatus::Suspended,
    );
    assert_ok(&send(&mut env.svm, &[ix], &[&registrar]));
    let raw = env.svm.get_account(&game_pda(&game)).unwrap();
    assert_eq!(
        raw.data[10], 0x01,
        "relayer memcmp(10, 0x00) now excludes it"
    );

    let ix = ix_set_game_status(&admin.pubkey(), Role::Admin, &game, GameStatus::Active);
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
    let entry: GameEntry = fetch(&env.svm, &game_pda(&game));
    assert_eq!(entry.status, GameStatus::Active);

    let moderator = holder(&mut env, &admin, Role::Moderator);
    let ix = ix_set_game_status(
        &moderator.pubkey(),
        Role::Moderator,
        &game,
        GameStatus::Suspended,
    );
    assert_custom_err(&send(&mut env.svm, &[ix], &[&moderator]), UNAUTHORIZED);
}

#[test]
fn set_game_status_rejects_non_game_entry_account() {
    // GameEntry is checked by owner + discriminator only (no seeds) — a RoleMember or
    // any other platform-core account must still be rejected.
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let mut ix = ix_set_game_status(&admin.pubkey(), Role::Admin, &game, GameStatus::Suspended);
    replace_account(&mut ix, 2, role_pda(Role::Admin, &admin.pubkey()));
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        ACCOUNT_DISCRIMINATOR_MISMATCH,
    );
    let mut ix = ix_set_game_status(&admin.pubkey(), Role::Admin, &game, GameStatus::Suspended);
    replace_account(&mut ix, 2, config_pda());
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        ACCOUNT_DISCRIMINATOR_MISMATCH,
    );
}

#[test]
fn transfer_game_owner_needs_both_signatures() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let owner = funded(&mut env.svm);
    let new_owner = funded(&mut env.svm);
    register(&mut env, &admin, &owner, &game);

    // Not the current owner.
    let impostor = funded(&mut env.svm);
    let ix = ix_transfer_game_owner(&impostor.pubkey(), &new_owner.pubkey(), &game);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&impostor, &new_owner]),
        UNAUTHORIZED,
    );
    // New owner did not co-sign.
    let mut ix = ix_transfer_game_owner(&owner.pubkey(), &new_owner.pubkey(), &game);
    unsign(&mut ix, &new_owner.pubkey());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&owner]), ACCOUNT_NOT_SIGNER);

    let ix = ix_transfer_game_owner(&owner.pubkey(), &new_owner.pubkey(), &game);
    assert_ok(&send(&mut env.svm, &[ix], &[&owner, &new_owner]));
    let entry: GameEntry = fetch(&env.svm, &game_pda(&game));
    assert_eq!(entry.owner, new_owner.pubkey());
    // Old owner lost control.
    let ix = ix_transfer_game_owner(&owner.pubkey(), &impostor.pubkey(), &game);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&owner, &impostor]),
        UNAUTHORIZED,
    );
}

#[test]
fn force_set_game_owner_is_admin_only() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let game = deploy_game(&mut env.svm);
    let owner = funded(&mut env.svm);
    register(&mut env, &admin, &owner, &game);
    let rescue = Pubkey::new_unique();

    let registrar = holder(&mut env, &admin, Role::GameRegistrar);
    let ix = ix_force_set_game_owner(&registrar.pubkey(), &game, &rescue);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&registrar]),
        ACCOUNT_NOT_INITIALIZED,
    );
    let ix = ix_force_set_game_owner(&admin.pubkey(), &game, &Pubkey::default());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), INVALID_MEMBER);

    let ix = ix_force_set_game_owner(&admin.pubkey(), &game, &rescue);
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
    let entry: GameEntry = fetch(&env.svm, &game_pda(&game));
    assert_eq!(entry.owner, rescue);
}

// ============================================================= allowlist

fn allow(env: &mut Env, signer: &Keypair, kind: Role, mint: &Pubkey, tp: &Pubkey) {
    let ix = ix_allow_mint(&signer.pubkey(), kind, mint, tp);
    assert_ok(&send(&mut env.svm, &[ix], &[signer]));
}

#[test]
fn allow_spl_token_mint_creates_allowed_mint_and_treasury_ata() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let alm = holder(&mut env, &admin, Role::AllowlistManager);
    let mint = create_mint(&mut env.svm, &alm, &TOKEN_PROGRAM, 6, &[]);
    allow(
        &mut env,
        &alm,
        Role::AllowlistManager,
        &mint,
        &TOKEN_PROGRAM,
    );

    let am: AllowedMint = fetch(&env.svm, &allowed_mint_pda(&mint));
    assert_eq!(am.version, ACCOUNT_VERSION);
    assert_eq!(am.mint, mint);
    assert_eq!(am.token_program, TOKEN_PROGRAM);
    assert_eq!(am.decimals, 6);
    assert_eq!(am.added_by, alm.pubkey());
    let t_ata = ata(&treasury_pda(), &mint, &TOKEN_PROGRAM);
    assert_eq!(token_account_owner(&env.svm, &t_ata), treasury_pda());
    assert_eq!(env.svm.get_account(&t_ata).unwrap().owner, TOKEN_PROGRAM);
}

#[test]
fn allow_token_2022_mints_plain_and_with_benign_extensions() {
    // Q4: Token-2022 is fully supported; only the Q5 extensions are refused.
    let mut env = setup();
    let admin = init_default(&mut env);
    for exts in [
        vec![],
        vec![TestExt::MetadataPointer],
        vec![TestExt::MintCloseAuthorityNone],
        vec![TestExt::DefaultInitialized],
    ] {
        let mint = create_mint(&mut env.svm, &admin, &TOKEN_2022_PROGRAM, 9, &exts);
        allow(&mut env, &admin, Role::Admin, &mint, &TOKEN_2022_PROGRAM);
        let am: AllowedMint = fetch(&env.svm, &allowed_mint_pda(&mint));
        assert_eq!(am.token_program, TOKEN_2022_PROGRAM, "{exts:?}");
        let t_ata = ata(&treasury_pda(), &mint, &TOKEN_2022_PROGRAM);
        assert_eq!(
            token_account_owner(&env.svm, &t_ata),
            treasury_pda(),
            "{exts:?}"
        );
    }
}

#[test]
fn forbidden_extensions_are_rejected_for_tokens_and_nfts() {
    // Q5 · applies to every mint including NFTs (decimals 0).
    let mut env = setup();
    let admin = init_default(&mut env);
    for ext in [
        TestExt::PermanentDelegate,
        TestExt::TransferFeeConfig,
        TestExt::TransferHook,
        TestExt::NonTransferable,
        TestExt::DefaultFrozen,
        // Closable mint → could be re-created with any of the above after allowlisting.
        TestExt::MintCloseAuthority,
    ] {
        for decimals in [9u8, 0u8] {
            let mint = create_mint(&mut env.svm, &admin, &TOKEN_2022_PROGRAM, decimals, &[ext]);
            let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_2022_PROGRAM);
            assert_custom_err(
                &send(&mut env.svm, &[ix], &[&admin]),
                UNSUPPORTED_MINT_EXTENSION,
            );
            assert!(
                is_closed(&env.svm, &allowed_mint_pda(&mint)),
                "{ext:?} decimals {decimals}"
            );
        }
    }
    // A forbidden extension mixed with benign ones is still rejected.
    let mint = create_mint(
        &mut env.svm,
        &admin,
        &TOKEN_2022_PROGRAM,
        0,
        &[TestExt::MetadataPointer, TestExt::PermanentDelegate],
    );
    let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_2022_PROGRAM);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        UNSUPPORTED_MINT_EXTENSION,
    );
}

#[test]
fn allow_mint_succeeds_when_treasury_ata_was_pre_created() {
    // Anyone can create the Treasury's ATA first; `init` would DoS allow_mint (X3).
    let mut env = setup();
    let admin = init_default(&mut env);
    let griefer = funded(&mut env.svm);
    for tp in [TOKEN_PROGRAM, TOKEN_2022_PROGRAM] {
        let mint = create_mint(&mut env.svm, &admin, &tp, 6, &[]);
        create_ata(&mut env.svm, &griefer, &treasury_pda(), &mint, &tp);
        allow(&mut env, &admin, Role::Admin, &mint, &tp);
        assert!(!is_closed(&env.svm, &allowed_mint_pda(&mint)));
    }
}

#[test]
fn allow_mint_account_checks() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let mint = create_mint(&mut env.svm, &admin, &TOKEN_2022_PROGRAM, 6, &[]);

    // Wrong token program for this mint.
    let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_PROGRAM);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        CONSTRAINT_MINT_TOKEN_PROGRAM,
    );
    // Treasury ATA address not the real ATA (index 6 = treasury_ata).
    let mut ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_2022_PROGRAM);
    replace_account(&mut ix, 6, ata(&admin.pubkey(), &mint, &TOKEN_2022_PROGRAM));
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), CONSTRAINT_ADDRESS);
    // Other roles.
    let fm = holder(&mut env, &admin, Role::FeeManager);
    let ix = ix_allow_mint(&fm.pubkey(), Role::FeeManager, &mint, &TOKEN_2022_PROGRAM);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&fm]), UNAUTHORIZED);

    allow(&mut env, &admin, Role::Admin, &mint, &TOKEN_2022_PROGRAM);
    // Allowing twice fails.
    let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_2022_PROGRAM);
    assert_failed(&send(&mut env.svm, &[ix], &[&admin]));
}

#[test]
fn disallow_closes_allowed_mint_but_keeps_treasury_ata() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let alm = holder(&mut env, &admin, Role::AllowlistManager);
    let mint = create_mint(&mut env.svm, &alm, &TOKEN_PROGRAM, 6, &[]);
    allow(
        &mut env,
        &alm,
        Role::AllowlistManager,
        &mint,
        &TOKEN_PROGRAM,
    );
    let t_ata = ata(&treasury_pda(), &mint, &TOKEN_PROGRAM);
    mint_to(&mut env.svm, &alm, &mint, &t_ata, &TOKEN_PROGRAM, 777, 6);

    // Wrong rent receiver.
    let ix = ix_disallow_mint(&admin.pubkey(), Role::Admin, &mint, &admin.pubkey());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), CONSTRAINT_ADDRESS);

    let pda = allowed_mint_pda(&mint);
    let rent = lamports(&env.svm, &pda);
    let before = lamports(&env.svm, &alm.pubkey());
    // Admin disallows; rent goes back to the AllowlistManager who added it.
    let ix = ix_disallow_mint(&admin.pubkey(), Role::Admin, &mint, &alm.pubkey());
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
    assert!(is_closed(&env.svm, &pda));
    assert_eq!(lamports(&env.svm, &alm.pubkey()), before + rent);
    // R4: ATA (and its fees) untouched.
    assert_eq!(token_balance(&env.svm, &t_ata), 777);

    // Re-allow works (ATA already exists → idempotent).
    allow(
        &mut env,
        &alm,
        Role::AllowlistManager,
        &mint,
        &TOKEN_PROGRAM,
    );
}

#[test]
fn moderator_cannot_disallow() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let mint = create_mint(&mut env.svm, &admin, &TOKEN_PROGRAM, 6, &[]);
    allow(&mut env, &admin, Role::Admin, &mint, &TOKEN_PROGRAM);
    let m = holder(&mut env, &admin, Role::Moderator);
    let ix = ix_disallow_mint(&m.pubkey(), Role::Moderator, &mint, &admin.pubkey());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&m]), UNAUTHORIZED);
}

// ============================================================== treasury

fn treasury_min(env: &Env) -> u64 {
    env.svm
        .minimum_balance_for_rent_exemption(8 + Treasury::INIT_SPACE)
}

#[test]
fn consumer_can_send_sol_to_treasury_and_admin_withdraws_down_to_rent_floor() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let payer = funded(&mut env.svm);
    // A consumer program / user credits the program-owned Treasury with a system transfer.
    let fee_in = 3 * SOL / 100;
    let ix = system_instruction::transfer(&payer.pubkey(), &treasury_pda(), fee_in);
    assert_ok(&send(&mut env.svm, &[ix], &[&payer]));
    assert_eq!(
        lamports(&env.svm, &treasury_pda()),
        treasury_min(&env) + fee_in
    );

    let revenue = Pubkey::new_unique();
    // One lamport too many → I3 guard.
    let ix = ix_withdraw_sol(&admin.pubkey(), &revenue, fee_in + 1);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), INSUFFICIENT_TREASURY);

    let ix = ix_withdraw_sol(&admin.pubkey(), &revenue, fee_in);
    assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
    assert_eq!(lamports(&env.svm, &revenue), fee_in);
    assert_eq!(
        lamports(&env.svm, &treasury_pda()),
        treasury_min(&env),
        "I3"
    );
    // Treasury data intact after lamport moves.
    let t: Treasury = fetch(&env.svm, &treasury_pda());
    assert_eq!(t.version, ACCOUNT_VERSION);

    // Nothing left above the floor.
    let ix = ix_withdraw_sol(&admin.pubkey(), &revenue, 1);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), INSUFFICIENT_TREASURY);
}

#[test]
fn only_admin_withdraws_sol() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let payer = funded(&mut env.svm);
    let ix = system_instruction::transfer(&payer.pubkey(), &treasury_pda(), SOL);
    assert_ok(&send(&mut env.svm, &[ix], &[&payer]));
    for role in [
        Role::FeeManager,
        Role::AllowlistManager,
        Role::GameRegistrar,
    ] {
        let h = holder(&mut env, &admin, role);
        let ix = ix_withdraw_sol(&h.pubkey(), &h.pubkey(), 1);
        assert_custom_err(&send(&mut env.svm, &[ix], &[&h]), ACCOUNT_NOT_INITIALIZED);
    }
    // A fake "treasury" (any account) cannot be substituted: seeds pin it.
    let mut ix = ix_withdraw_sol(&admin.pubkey(), &admin.pubkey(), 1);
    replace_account(&mut ix, 3, payer.pubkey());
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        ACCOUNT_OWNED_BY_WRONG_PROGRAM,
    );
}

#[test]
fn admin_withdraws_token_fees_for_spl_and_token_2022_even_after_disallow() {
    let mut env = setup();
    let admin = init_default(&mut env);
    for tp in [TOKEN_PROGRAM, TOKEN_2022_PROGRAM] {
        let mint = create_mint(&mut env.svm, &admin, &tp, 6, &[]);
        allow(&mut env, &admin, Role::Admin, &mint, &tp);
        let t_ata = ata(&treasury_pda(), &mint, &tp);
        // Simulates a consumer's fee transfer into the Treasury ATA.
        mint_to(&mut env.svm, &admin, &mint, &t_ata, &tp, 1_000, 6);
        let dest = create_ata(&mut env.svm, &admin, &admin.pubkey(), &mint, &tp);

        let ix = ix_withdraw_token(&admin.pubkey(), &mint, &tp, &dest, 400);
        assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
        assert_eq!(token_balance(&env.svm, &t_ata), 600);
        assert_eq!(token_balance(&env.svm, &dest), 400);

        // Disallow, then the remaining fees are still withdrawable.
        let ix = ix_disallow_mint(&admin.pubkey(), Role::Admin, &mint, &admin.pubkey());
        assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
        let ix = ix_withdraw_token(&admin.pubkey(), &mint, &tp, &dest, 600);
        assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
        assert_eq!(token_balance(&env.svm, &t_ata), 0);
        assert_eq!(token_balance(&env.svm, &dest), 1_000);

        // More than the balance fails in the token program.
        let ix = ix_withdraw_token(&admin.pubkey(), &mint, &tp, &dest, 1);
        assert_failed(&send(&mut env.svm, &[ix], &[&admin]));
    }
}

#[test]
fn withdraw_token_account_checks() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let mint = create_mint(&mut env.svm, &admin, &TOKEN_PROGRAM, 6, &[]);
    allow(&mut env, &admin, Role::Admin, &mint, &TOKEN_PROGRAM);
    let t_ata = ata(&treasury_pda(), &mint, &TOKEN_PROGRAM);
    mint_to(&mut env.svm, &admin, &mint, &t_ata, &TOKEN_PROGRAM, 500, 6);
    let dest = create_ata(&mut env.svm, &admin, &admin.pubkey(), &mint, &TOKEN_PROGRAM);

    // Non-admin.
    let alm = holder(&mut env, &admin, Role::AllowlistManager);
    let alm_dest = create_ata(&mut env.svm, &alm, &alm.pubkey(), &mint, &TOKEN_PROGRAM);
    let ix = ix_withdraw_token(&alm.pubkey(), &mint, &TOKEN_PROGRAM, &alm_dest, 1);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&alm]), ACCOUNT_NOT_INITIALIZED);

    // Source must be the Treasury's ATA (index 5): an attacker-owned account of the
    // same mint is rejected even though the Treasury PDA would not sign for it anyway.
    mint_to(
        &mut env.svm,
        &admin,
        &mint,
        &alm_dest,
        &TOKEN_PROGRAM,
        50,
        6,
    );
    let mut ix = ix_withdraw_token(&admin.pubkey(), &mint, &TOKEN_PROGRAM, &dest, 1);
    replace_account(&mut ix, 5, alm_dest);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        CONSTRAINT_TOKEN_OWNER,
    );

    // Destination of a different mint.
    let mint2 = create_mint(&mut env.svm, &admin, &TOKEN_PROGRAM, 6, &[]);
    let wrong_dest = create_ata(
        &mut env.svm,
        &admin,
        &admin.pubkey(),
        &mint2,
        &TOKEN_PROGRAM,
    );
    let ix = ix_withdraw_token(&admin.pubkey(), &mint, &TOKEN_PROGRAM, &wrong_dest, 1);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), CONSTRAINT_TOKEN_MINT);

    assert_eq!(token_balance(&env.svm, &t_ata), 500, "nothing moved");
}

#[test]
fn register_rejects_non_upgradeable_builtin_and_self() {
    // Only deployed (upgradeable-loader) programs other than platform-core may become
    // games — anything else would land on the relayer's co-sign allowlist.
    let mut env = setup();
    let admin = init_default(&mut env);
    let owner = funded(&mut env.svm);
    for program in [
        MEMO_PROGRAM,                                    // BPF loader v2
        anchor_lang::solana_program::system_program::ID, // native builtin
        TOKEN_PROGRAM,      // upgradeable-loader owned (core BPF / p-token) → denylist
        TOKEN_2022_PROGRAM, // upgradeable-loader owned → denylist
        ATA_PROGRAM,
        platform_core::id(), // ourselves
        anchor_lang::pubkey!("AddressLookupTab1e1111111111111111111111111"), // core BPF
    ] {
        let ix = ix_register_game(&admin.pubkey(), Role::Admin, &owner.pubkey(), &program);
        assert_custom_err(
            &send(&mut env.svm, &[ix], &[&admin, &owner]),
            INVALID_GAME_PROGRAM,
        );
    }
}

#[test]
fn closable_mint_attack_is_blocked_at_allow_time() {
    // QA finding: a mint WITH a close authority could be allowlisted, closed at zero
    // supply, and re-created at the same address with PermanentDelegate. Show the
    // mint really is closable, and that allow_mint refuses it up front.
    let mut env = setup();
    let admin = init_default(&mut env);
    let mint = create_mint(
        &mut env.svm,
        &admin,
        &TOKEN_2022_PROGRAM,
        0,
        &[TestExt::MintCloseAuthority],
    );
    let ix = ix_allow_mint(&admin.pubkey(), Role::Admin, &mint, &TOKEN_2022_PROGRAM);
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&admin]),
        UNSUPPORTED_MINT_EXTENSION,
    );
    // The same kind of mint can indeed be closed (so the rule is not theoretical).
    let close = anchor_spl::token_2022::spl_token_2022::instruction::close_account(
        &TOKEN_2022_PROGRAM,
        &mint,
        &admin.pubkey(),
        &admin.pubkey(),
        &[],
    )
    .unwrap();
    assert_ok(&send(&mut env.svm, &[close], &[&admin]));
    assert!(is_closed(&env.svm, &mint));
}

#[test]
fn mint_without_close_authority_cannot_be_closed() {
    // Complements the close-authority rule: `close_authority = None` is permanent.
    let mut env = setup();
    let admin = init_default(&mut env);
    let mint = create_mint(
        &mut env.svm,
        &admin,
        &TOKEN_2022_PROGRAM,
        0,
        &[TestExt::MintCloseAuthorityNone],
    );
    allow(&mut env, &admin, Role::Admin, &mint, &TOKEN_2022_PROGRAM);
    let close = anchor_spl::token_2022::spl_token_2022::instruction::close_account(
        &TOKEN_2022_PROGRAM,
        &mint,
        &admin.pubkey(),
        &admin.pubkey(),
        &[],
    )
    .unwrap();
    assert_failed(&send(&mut env.svm, &[close], &[&admin]));
    assert!(!is_closed(&env.svm, &mint));
}

#[test]
fn disallow_mint_rejects_other_account_types() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let mint = create_mint(&mut env.svm, &admin, &TOKEN_PROGRAM, 6, &[]);
    allow(&mut env, &admin, Role::Admin, &mint, &TOKEN_PROGRAM);
    for wrong in [
        role_pda(Role::Admin, &admin.pubkey()),
        config_pda(),
        treasury_pda(),
    ] {
        let mut ix = ix_disallow_mint(&admin.pubkey(), Role::Admin, &mint, &admin.pubkey());
        replace_account(&mut ix, 2, wrong); // allowed_mint
        assert_failed(&send(&mut env.svm, &[ix], &[&admin]));
        assert!(!is_closed(&env.svm, &wrong));
    }
    assert!(!is_closed(&env.svm, &allowed_mint_pda(&mint)));
}
