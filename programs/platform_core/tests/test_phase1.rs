//! Phase 1 — initialize · grant/revoke/renounce role · set_fee (specs/designs/02 §5).
mod common;

use {
    anchor_lang::{prelude::Pubkey, Space},
    common::*,
    platform_core::{
        constants::*,
        state::{FeeKind, PlatformConfig, Role, RoleMember, Treasury},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

// Anchor framework error codes used below (anchor-lang ErrorCode).
const CONSTRAINT_SEEDS: u32 = 2006;
const CONSTRAINT_ADDRESS: u32 = 2012;
const ACCOUNT_NOT_INITIALIZED: u32 = 3012;
const UNAUTHORIZED: u32 = 6000;
const FEE_TOO_HIGH: u32 = 6001;
const LAST_ADMIN: u32 = 6002;
const INVALID_MEMBER: u32 = 6011;

const ALL_ROLES: [Role; 5] = [
    Role::Admin,
    Role::GameRegistrar,
    Role::Moderator,
    Role::FeeManager,
    Role::AllowlistManager,
];

/// I2: `admin_count` equals the number of live `RoleMember(Admin, *)` among `candidates`.
fn assert_admin_count_invariant(env: &Env, candidates: &[Pubkey]) {
    let live = candidates
        .iter()
        .filter(|m| !is_closed(&env.svm, &role_pda(Role::Admin, m)))
        .count();
    let config: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(config.admin_count as usize, live, "I2 admin_count");
    assert!(config.admin_count >= 1, "I2 admin_count >= 1");
}

// ================================================================ smoke

#[test]
fn smoke_litesvm_runs_program_built_with_cargo_build_sbf_4_3() {
    // Plan 04 Phase 1 step 1: LiteSVM must execute a .so built by cargo-build-sbf 4.3.0
    // (SBPF v3). 0.10 / agave 3.1 could not (InvalidAccountData); 0.17 / agave 4.3 can.
    let mut env = setup();
    let admin = init_default(&mut env);
    let role: RoleMember = fetch(&env.svm, &role_pda(Role::Admin, &admin.pubkey()));
    assert_eq!(role.member, admin.pubkey());
}

// ============================================================ initialize

#[test]
fn initialize_writes_all_three_accounts() {
    let mut env = setup();
    set_time(&mut env.svm, 1_700_000_000);
    let admin = Keypair::new();
    let ua = env.upgrade_authority.insecure_clone();
    let ix = ix_initialize(&ua.pubkey(), 100, 250, &admin.pubkey());
    assert_ok(&send(&mut env.svm, &[ix], &[&ua]));

    let config: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(config.version, ACCOUNT_VERSION);
    assert_eq!(config.boost_fee_bps, 100);
    assert_eq!(config.ad_fee_bps, 250);
    assert_eq!(config.admin_count, 1);
    assert_eq!(
        config.treasury_bump,
        Pubkey::find_program_address(&[TREASURY_SEED], &platform_core::id()).1
    );
    assert_eq!(config.initialized_at, 1_700_000_000);
    assert_eq!(config.updated_at, 1_700_000_000);
    assert_eq!(config._reserved, [0u8; 64]);

    let treasury: Treasury = fetch(&env.svm, &treasury_pda());
    assert_eq!(treasury.version, ACCOUNT_VERSION);
    assert_eq!(treasury.bump, config.treasury_bump);
    // Pre-funded at rent-exempt minimum so the first small fee transfer never fails (01 §6).
    let size = 8 + Treasury::INIT_SPACE;
    assert_eq!(
        lamports(&env.svm, &treasury_pda()),
        env.svm.minimum_balance_for_rent_exemption(size)
    );

    let role: RoleMember = fetch(&env.svm, &role_pda(Role::Admin, &admin.pubkey()));
    assert_eq!(role.version, ACCOUNT_VERSION);
    assert_eq!(role.role, Role::Admin);
    assert_eq!(role.member, admin.pubkey());
    assert_eq!(role.granted_by, ua.pubkey());
    assert_eq!(role.granted_at, 1_700_000_000);
}

#[test]
fn initialize_first_admin_need_not_be_upgrade_authority() {
    // X6: the cold upgrade-authority key signs init but does not become Admin.
    let mut env = setup();
    let admin = init_default(&mut env);
    assert!(!is_closed(
        &env.svm,
        &role_pda(Role::Admin, &admin.pubkey())
    ));
    assert!(is_closed(
        &env.svm,
        &role_pda(Role::Admin, &env.upgrade_authority.pubkey())
    ));
}

#[test]
fn initialize_first_admin_may_be_upgrade_authority() {
    let mut env = setup();
    let ua = env.upgrade_authority.insecure_clone();
    let ix = ix_initialize(&ua.pubkey(), 100, 100, &ua.pubkey());
    assert_ok(&send(&mut env.svm, &[ix], &[&ua]));
    assert!(!is_closed(&env.svm, &role_pda(Role::Admin, &ua.pubkey())));
}

#[test]
fn initialize_rejects_non_upgrade_authority() {
    // Gotcha #6: anyone else racing to initialize must fail.
    let mut env = setup();
    let attacker = funded(&mut env.svm);
    let ix = ix_initialize(&attacker.pubkey(), 100, 100, &attacker.pubkey());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&attacker]), UNAUTHORIZED);
    assert!(is_closed(&env.svm, &config_pda()));
}

#[test]
fn initialize_rejects_program_without_upgrade_authority() {
    // A program deployed --final (authority None) can never be initialized.
    let mut env = setup();
    set_upgrade_authority(&mut env.svm, &platform_core::id(), None);
    let ua = env.upgrade_authority.insecure_clone();
    let ix = ix_initialize(&ua.pubkey(), 100, 100, &ua.pubkey());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&ua]), UNAUTHORIZED);
}

#[test]
fn initialize_rejects_programdata_of_another_program() {
    // Attacker deploys their own copy (they are its upgrade authority) and passes that
    // ProgramData: the `program.programdata_address()` constraint must catch it.
    let mut env = setup();
    let attacker = funded(&mut env.svm);
    let other_program = Pubkey::new_unique();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/platform_core.so"
    ));
    env.svm.add_program(other_program, bytes).unwrap();
    set_upgrade_authority(&mut env.svm, &other_program, Some(attacker.pubkey()));

    let ix = ix_initialize_with(
        &attacker.pubkey(),
        &platform_core::id(),
        &program_data_address(&other_program),
        100,
        100,
        &attacker.pubkey(),
    );
    assert_custom_err(&send(&mut env.svm, &[ix], &[&attacker]), UNAUTHORIZED);
    assert!(is_closed(&env.svm, &config_pda()));
}

#[test]
fn initialize_twice_fails() {
    let mut env = setup();
    init_default(&mut env);
    let ua = env.upgrade_authority.insecure_clone();
    let other = Keypair::new();
    let ix = ix_initialize(&ua.pubkey(), 100, 100, &other.pubkey());
    assert_failed(&send(&mut env.svm, &[ix], &[&ua]));
    let config: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(config.admin_count, 1);
}

#[test]
fn initialize_enforces_fee_ceiling_on_both_fees() {
    let mut env = setup();
    let ua = env.upgrade_authority.insecure_clone();
    let a = Pubkey::new_unique();
    for (boost, ad) in [(MAX_FEE_BPS + 1, 100), (100, MAX_FEE_BPS + 1)] {
        let ix = ix_initialize(&ua.pubkey(), boost, ad, &a);
        assert_custom_err(&send(&mut env.svm, &[ix], &[&ua]), FEE_TOO_HIGH);
    }
    let ix = ix_initialize(&ua.pubkey(), MAX_FEE_BPS, MAX_FEE_BPS, &a);
    assert_ok(&send(&mut env.svm, &[ix], &[&ua]));
}

// ================================================================== RBAC

#[test]
fn grant_every_role_and_admin_count_only_tracks_admin() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let mut admins = vec![admin.pubkey()];
    for role in ALL_ROLES {
        let member = Pubkey::new_unique();
        let ix = ix_grant_role(&admin.pubkey(), role, &member);
        assert_ok(&send(&mut env.svm, &[ix], &[&admin]));
        let rm: RoleMember = fetch(&env.svm, &role_pda(role, &member));
        assert_eq!(rm.version, ACCOUNT_VERSION);
        assert_eq!(rm.role, role);
        assert_eq!(rm.member, member);
        assert_eq!(rm.granted_by, admin.pubkey());
        assert_eq!(rm._reserved, [0u8; 32]);
        if role == Role::Admin {
            admins.push(member);
        }
        assert_admin_count_invariant(&env, &admins);
    }
    let config: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(config.admin_count, 2);
}

#[test]
fn non_admin_cannot_grant() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let fee_manager = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(
            &admin.pubkey(),
            Role::FeeManager,
            &fee_manager.pubkey(),
        )],
        &[&admin],
    ));
    // Holds FeeManager but not Admin → its Admin PDA does not exist.
    let ix = ix_grant_role(&fee_manager.pubkey(), Role::Admin, &fee_manager.pubkey());
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&fee_manager]),
        ACCOUNT_NOT_INITIALIZED,
    );
}

#[test]
fn granting_an_existing_pair_fails() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let m = Pubkey::new_unique();
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&admin.pubkey(), Role::Moderator, &m)],
        &[&admin],
    ));
    assert_failed(&send(
        &mut env.svm,
        &[ix_grant_role(&admin.pubkey(), Role::Moderator, &m)],
        &[&admin],
    ));
}

#[test]
fn revoke_closes_account_and_refunds_rent_to_granted_by() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let admin2 = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(
            &admin.pubkey(),
            Role::Admin,
            &admin2.pubkey(),
        )],
        &[&admin],
    ));
    let m = Pubkey::new_unique();
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&admin.pubkey(), Role::Moderator, &m)],
        &[&admin],
    ));
    let pda = role_pda(Role::Moderator, &m);
    let rent = lamports(&env.svm, &pda);
    let before = lamports(&env.svm, &admin.pubkey());

    // A *different* Admin revokes; rent must go to the original granter (D11).
    let ix = ix_revoke_role(&admin2.pubkey(), Role::Moderator, &m, &admin.pubkey());
    assert_ok(&send(&mut env.svm, &[ix], &[&admin2]));
    assert!(is_closed(&env.svm, &pda));
    assert_eq!(lamports(&env.svm, &admin.pubkey()), before + rent);
}

#[test]
fn revoke_rejects_wrong_rent_receiver() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let m = Pubkey::new_unique();
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&admin.pubkey(), Role::Moderator, &m)],
        &[&admin],
    ));
    let thief = Pubkey::new_unique();
    let ix = ix_revoke_role(&admin.pubkey(), Role::Moderator, &m, &thief);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), CONSTRAINT_ADDRESS);
    assert!(!is_closed(&env.svm, &role_pda(Role::Moderator, &m)));
}

#[test]
fn non_admin_cannot_revoke() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let outsider = funded(&mut env.svm);
    let ix = ix_revoke_role(
        &outsider.pubkey(),
        Role::Admin,
        &admin.pubkey(),
        &env.upgrade_authority.pubkey(),
    );
    assert_custom_err(
        &send(&mut env.svm, &[ix], &[&outsider]),
        ACCOUNT_NOT_INITIALIZED,
    );
}

#[test]
fn last_admin_cannot_be_revoked_or_renounced() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let ua = env.upgrade_authority.pubkey();

    let ix = ix_revoke_role(&admin.pubkey(), Role::Admin, &admin.pubkey(), &ua);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), LAST_ADMIN);

    let ix = ix_renounce_role(&admin.pubkey(), Role::Admin, &ua);
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), LAST_ADMIN);

    assert_admin_count_invariant(&env, &[admin.pubkey()]);
}

#[test]
fn admin_lifecycle_keeps_invariant_i2() {
    let mut env = setup();
    let a1 = init_default(&mut env);
    let a2 = funded(&mut env.svm);
    let a3 = funded(&mut env.svm);
    let all = [a1.pubkey(), a2.pubkey(), a3.pubkey()];
    let ua = env.upgrade_authority.pubkey();

    for a in [&a2, &a3] {
        assert_ok(&send(
            &mut env.svm,
            &[ix_grant_role(&a1.pubkey(), Role::Admin, &a.pubkey())],
            &[&a1],
        ));
        assert_admin_count_invariant(&env, &all);
    }
    // 3 → 2 via renounce (a1 was granted by the upgrade authority at init).
    assert_ok(&send(
        &mut env.svm,
        &[ix_renounce_role(&a1.pubkey(), Role::Admin, &ua)],
        &[&a1],
    ));
    assert_admin_count_invariant(&env, &all);
    // 2 → 1 via revoke by another Admin.
    assert_ok(&send(
        &mut env.svm,
        &[ix_revoke_role(
            &a2.pubkey(),
            Role::Admin,
            &a3.pubkey(),
            &a1.pubkey(),
        )],
        &[&a2],
    ));
    assert_admin_count_invariant(&env, &all);
    // 1 left → both paths refused.
    assert_custom_err(
        &send(
            &mut env.svm,
            &[ix_renounce_role(&a2.pubkey(), Role::Admin, &a1.pubkey())],
            &[&a2],
        ),
        LAST_ADMIN,
    );
    assert_custom_err(
        &send(
            &mut env.svm,
            &[ix_revoke_role(
                &a2.pubkey(),
                Role::Admin,
                &a2.pubkey(),
                &a1.pubkey(),
            )],
            &[&a2],
        ),
        LAST_ADMIN,
    );
    assert_admin_count_invariant(&env, &all);
    // Revoked admin can be re-granted (PDA re-init after close).
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&a2.pubkey(), Role::Admin, &a1.pubkey())],
        &[&a2],
    ));
    assert_admin_count_invariant(&env, &all);
}

#[test]
fn renounce_non_admin_role_and_not_held_role() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let m = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(
            &admin.pubkey(),
            Role::AllowlistManager,
            &m.pubkey(),
        )],
        &[&admin],
    ));
    let before = lamports(&env.svm, &admin.pubkey());
    let pda = role_pda(Role::AllowlistManager, &m.pubkey());
    let rent = lamports(&env.svm, &pda);
    assert_ok(&send(
        &mut env.svm,
        &[ix_renounce_role(
            &m.pubkey(),
            Role::AllowlistManager,
            &admin.pubkey(),
        )],
        &[&m],
    ));
    assert!(is_closed(&env.svm, &pda));
    assert_eq!(lamports(&env.svm, &admin.pubkey()), before + rent);

    // Renouncing a role you don't hold fails.
    assert_custom_err(
        &send(
            &mut env.svm,
            &[ix_renounce_role(
                &m.pubkey(),
                Role::Moderator,
                &admin.pubkey(),
            )],
            &[&m],
        ),
        ACCOUNT_NOT_INITIALIZED,
    );
    let config: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(config.admin_count, 1);
}

// =================================================================== fee

#[test]
fn fee_manager_sets_each_fee_independently() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let fm = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(
            &admin.pubkey(),
            Role::FeeManager,
            &fm.pubkey(),
        )],
        &[&admin],
    ));
    set_time(&mut env.svm, 1_800_000_000);

    let res = send(
        &mut env.svm,
        &[ix_set_fee(
            &fm.pubkey(),
            Role::FeeManager,
            FeeKind::Boost,
            300,
        )],
        &[&fm],
    );
    assert_ok(&res);
    assert!(
        res.as_ref()
            .unwrap()
            .logs
            .iter()
            .any(|l| l.starts_with("Program data: ")),
        "FeeChanged event emitted"
    );
    let c: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!((c.boost_fee_bps, c.ad_fee_bps), (300, 100));
    assert_eq!(c.updated_at, 1_800_000_000);

    assert_ok(&send(
        &mut env.svm,
        &[ix_set_fee(&fm.pubkey(), Role::FeeManager, FeeKind::Ad, 50)],
        &[&fm],
    ));
    let c: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!((c.boost_fee_bps, c.ad_fee_bps), (300, 50));
}

#[test]
fn admin_is_superuser_for_set_fee() {
    // Q2 / X4: inside platform-core, Admin may act wherever a specific role is required.
    let mut env = setup();
    let admin = init_default(&mut env);
    assert_ok(&send(
        &mut env.svm,
        &[ix_set_fee(&admin.pubkey(), Role::Admin, FeeKind::Ad, 0)],
        &[&admin],
    ));
    let c: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(c.ad_fee_bps, 0);
}

#[test]
fn other_roles_cannot_set_fee() {
    let mut env = setup();
    let admin = init_default(&mut env);
    for role in [Role::GameRegistrar, Role::Moderator, Role::AllowlistManager] {
        let holder = funded(&mut env.svm);
        assert_ok(&send(
            &mut env.svm,
            &[ix_grant_role(&admin.pubkey(), role, &holder.pubkey())],
            &[&admin],
        ));
        assert_custom_err(
            &send(
                &mut env.svm,
                &[ix_set_fee(&holder.pubkey(), role, FeeKind::Boost, 1)],
                &[&holder],
            ),
            UNAUTHORIZED,
        );
    }
}

#[test]
fn set_fee_cannot_be_forged() {
    let mut env = setup();
    let admin = init_default(&mut env);
    let fm = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(
            &admin.pubkey(),
            Role::FeeManager,
            &fm.pubkey(),
        )],
        &[&admin],
    ));
    let outsider = funded(&mut env.svm);

    // Claims FeeManager without holding it → its own PDA doesn't exist.
    assert_custom_err(
        &send(
            &mut env.svm,
            &[ix_set_fee(
                &outsider.pubkey(),
                Role::FeeManager,
                FeeKind::Boost,
                1,
            )],
            &[&outsider],
        ),
        ACCOUNT_NOT_INITIALIZED,
    );
    // Passes the real FeeManager's PDA → seeds are derived from the signer → mismatch.
    let ix = ix_set_fee_with_role_account(
        &outsider.pubkey(),
        &role_pda(Role::FeeManager, &fm.pubkey()),
        Role::FeeManager,
        FeeKind::Boost,
        1,
    );
    assert_custom_err(&send(&mut env.svm, &[ix], &[&outsider]), CONSTRAINT_SEEDS);
    // Passes its own valid Moderator PDA while claiming FeeManager → seeds mismatch.
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(
            &admin.pubkey(),
            Role::Moderator,
            &outsider.pubkey(),
        )],
        &[&admin],
    ));
    let ix = ix_set_fee_with_role_account(
        &outsider.pubkey(),
        &role_pda(Role::Moderator, &outsider.pubkey()),
        Role::FeeManager,
        FeeKind::Boost,
        1,
    );
    assert_custom_err(&send(&mut env.svm, &[ix], &[&outsider]), CONSTRAINT_SEEDS);

    let c: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!(c.boost_fee_bps, 100, "fee unchanged");
}

#[test]
fn set_fee_enforces_ceiling() {
    let mut env = setup();
    let admin = init_default(&mut env);
    for kind in [FeeKind::Boost, FeeKind::Ad] {
        assert_ok(&send(
            &mut env.svm,
            &[ix_set_fee(&admin.pubkey(), Role::Admin, kind, MAX_FEE_BPS)],
            &[&admin],
        ));
        assert_custom_err(
            &send(
                &mut env.svm,
                &[ix_set_fee(
                    &admin.pubkey(),
                    Role::Admin,
                    kind,
                    MAX_FEE_BPS + 1,
                )],
                &[&admin],
            ),
            FEE_TOO_HIGH,
        );
    }
    let c: PlatformConfig = fetch(&env.svm, &config_pda());
    assert_eq!((c.boost_fee_bps, c.ad_fee_bps), (MAX_FEE_BPS, MAX_FEE_BPS));
}

#[test]
fn admin_may_revoke_itself_when_another_admin_remains() {
    // The same RoleMember is passed as both `admin_role` (read-only) and `role_member`
    // (closed). Allowed: the LastAdmin guard is the only rule that matters (I2).
    let mut env = setup();
    let a1 = init_default(&mut env);
    let a2 = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&a1.pubkey(), Role::Admin, &a2.pubkey())],
        &[&a1],
    ));
    assert_ok(&send(
        &mut env.svm,
        &[ix_revoke_role(
            &a2.pubkey(),
            Role::Admin,
            &a2.pubkey(),
            &a1.pubkey(),
        )],
        &[&a2],
    ));
    assert!(is_closed(&env.svm, &role_pda(Role::Admin, &a2.pubkey())));
    assert_admin_count_invariant(&env, &[a1.pubkey(), a2.pubkey()]);
}

#[test]
fn default_pubkey_cannot_become_member() {
    let mut env = setup();
    let ua = env.upgrade_authority.insecure_clone();
    let ix = ix_initialize(&ua.pubkey(), 100, 100, &Pubkey::default());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&ua]), INVALID_MEMBER);

    let admin = init_default(&mut env);
    let ix = ix_grant_role(&admin.pubkey(), Role::Moderator, &Pubkey::default());
    assert_custom_err(&send(&mut env.svm, &[ix], &[&admin]), INVALID_MEMBER);
}

#[test]
fn role_arg_cannot_be_swapped_to_skip_admin_count_decrement() {
    // I2: claiming `role = Moderator` while passing an Admin's RoleMember must fail on
    // seeds, otherwise an Admin could be removed without decrementing `admin_count`.
    let mut env = setup();
    let a1 = init_default(&mut env);
    let a2 = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&a1.pubkey(), Role::Admin, &a2.pubkey())],
        &[&a1],
    ));
    let mut revoke = ix_revoke_role(&a1.pubkey(), Role::Moderator, &a2.pubkey(), &a1.pubkey());
    revoke.accounts[3].pubkey = role_pda(Role::Admin, &a2.pubkey()); // role_member
    assert_custom_err(&send(&mut env.svm, &[revoke], &[&a1]), CONSTRAINT_SEEDS);

    let mut renounce = ix_renounce_role(&a2.pubkey(), Role::Moderator, &a1.pubkey());
    renounce.accounts[2].pubkey = role_pda(Role::Admin, &a2.pubkey()); // role_member
    assert_custom_err(&send(&mut env.svm, &[renounce], &[&a2]), CONSTRAINT_SEEDS);

    assert!(!is_closed(&env.svm, &role_pda(Role::Admin, &a2.pubkey())));
    assert_admin_count_invariant(&env, &[a1.pubkey(), a2.pubkey()]);
}

#[test]
fn renouncing_admin_refunds_rent_to_granter() {
    let mut env = setup();
    let a1 = init_default(&mut env);
    let a2 = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&a1.pubkey(), Role::Admin, &a2.pubkey())],
        &[&a1],
    ));
    let pda = role_pda(Role::Admin, &a2.pubkey());
    let rent = lamports(&env.svm, &pda);
    let before = lamports(&env.svm, &a1.pubkey());
    assert_ok(&send(
        &mut env.svm,
        &[ix_renounce_role(&a2.pubkey(), Role::Admin, &a1.pubkey())],
        &[&a2],
    ));
    assert!(is_closed(&env.svm, &pda));
    assert_eq!(lamports(&env.svm, &a1.pubkey()), before + rent);
    assert_admin_count_invariant(&env, &[a1.pubkey(), a2.pubkey()]);
}

#[test]
fn admin_revokes_every_role_it_granted_itself() {
    // Default single-Admin path: signer == granted_by (Signer + UncheckedAccount(mut) on
    // the same key). Covers every role, and rent comes back to the same wallet.
    let mut env = setup();
    let admin = init_default(&mut env);
    for role in ALL_ROLES {
        let m = Pubkey::new_unique();
        assert_ok(&send(
            &mut env.svm,
            &[ix_grant_role(&admin.pubkey(), role, &m)],
            &[&admin],
        ));
        let pda = role_pda(role, &m);
        let rent = lamports(&env.svm, &pda);
        let before = lamports(&env.svm, &admin.pubkey());
        let res = send(
            &mut env.svm,
            &[ix_revoke_role(&admin.pubkey(), role, &m, &admin.pubkey())],
            &[&admin],
        );
        assert_ok(&res);
        let fee = res.unwrap().fee; // signer also pays the tx fee
        assert!(is_closed(&env.svm, &pda), "{role:?} closed");
        assert_eq!(
            lamports(&env.svm, &admin.pubkey()),
            before + rent - fee,
            "{role:?} rent back to signer/granter"
        );
        assert_admin_count_invariant(&env, &[admin.pubkey(), m]);
    }
}

#[test]
fn member_renounces_every_role_it_granted_itself() {
    // member == granted_by: an Admin grants itself each role, then renounces it.
    let mut env = setup();
    let admin = init_default(&mut env);
    for role in [
        Role::GameRegistrar,
        Role::Moderator,
        Role::FeeManager,
        Role::AllowlistManager,
    ] {
        assert_ok(&send(
            &mut env.svm,
            &[ix_grant_role(&admin.pubkey(), role, &admin.pubkey())],
            &[&admin],
        ));
        let pda = role_pda(role, &admin.pubkey());
        let rent = lamports(&env.svm, &pda);
        let before = lamports(&env.svm, &admin.pubkey());
        let res = send(
            &mut env.svm,
            &[ix_renounce_role(&admin.pubkey(), role, &admin.pubkey())],
            &[&admin],
        );
        assert_ok(&res);
        let fee = res.unwrap().fee; // signer also pays the tx fee
        assert!(is_closed(&env.svm, &pda), "{role:?} closed");
        assert_eq!(
            lamports(&env.svm, &admin.pubkey()),
            before + rent - fee,
            "{role:?} rent back"
        );
    }
    // The Admin seat itself is untouched.
    assert_admin_count_invariant(&env, &[admin.pubkey()]);
}
