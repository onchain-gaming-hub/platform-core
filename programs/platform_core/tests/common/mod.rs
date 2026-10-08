//! Shared LiteSVM helpers for instruction tests.
#![allow(dead_code)]
// `TransactionResult` is litesvm's type; its large Err variant is not ours to box.
#![allow(clippy::result_large_err)]

use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{bpf_loader_upgradeable, instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    platform_core::{
        constants::*,
        state::{FeeKind, Role},
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub const SOL: u64 = 1_000_000_000;

pub struct Env {
    pub svm: LiteSVM,
    pub upgrade_authority: Keypair,
}

/// ⚠️ Loads the PRE-BUILT `target/deploy/platform_core.so` — run `anchor build` before
/// `cargo test`, or tests run against a stale binary (use `anchor test`, which builds first).
///
/// Loads `platform_core.so` through the upgradeable loader and sets a real upgrade
/// authority on its ProgramData (LiteSVM creates it with `None`).
pub fn setup() -> Env {
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/platform_core.so"
    ));
    svm.add_program(platform_core::id(), bytes).unwrap();
    let upgrade_authority = funded(&mut svm);
    set_upgrade_authority(
        &mut svm,
        &platform_core::id(),
        Some(upgrade_authority.pubkey()),
    );
    Env {
        svm,
        upgrade_authority,
    }
}

pub fn program_data_address(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Rewrites the `UpgradeableLoaderState::ProgramData` header (bincode):
/// u32 tag = 3 · u64 slot · Option<Pubkey> (1 tag byte + 32).
pub fn set_upgrade_authority(svm: &mut LiteSVM, program_id: &Pubkey, auth: Option<Pubkey>) {
    let address = program_data_address(program_id);
    let mut account = svm.get_account(&address).expect("programdata");
    assert_eq!(&account.data[0..4], &3u32.to_le_bytes(), "ProgramData tag");
    match auth {
        Some(pk) => {
            account.data[12] = 1;
            account.data[13..45].copy_from_slice(pk.as_ref());
        }
        None => {
            account.data[12] = 0;
            account.data[13..45].fill(0);
        }
    }
    svm.set_account(address, account).unwrap();
}

pub fn funded(svm: &mut LiteSVM) -> Keypair {
    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), 100 * SOL).unwrap();
    kp
}

pub fn set_time(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock: Clock = svm.get_sysvar();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
}

/// Sends one tx (payer = first signer) and expires the blockhash so an identical
/// tx later in the same test is not rejected as a duplicate.
pub fn send(svm: &mut LiteSVM, ixs: &[Instruction], signers: &[&Keypair]) -> TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    let res = svm.send_transaction(tx);
    svm.expire_blockhash();
    res
}

pub fn assert_ok(res: &TransactionResult) {
    if let Err(e) = res {
        panic!("tx failed: {:?}\nlogs: {:#?}", e.err, e.meta.logs);
    }
}

/// Asserts the tx failed with `InstructionError::Custom(code)` (Anchor / program errors).
pub fn assert_custom_err(res: &TransactionResult, code: u32) {
    match res {
        Ok(_) => panic!("expected Custom({code}), tx succeeded"),
        Err(e) => {
            let s = format!("{:?}", e.err);
            assert!(
                s.contains(&format!("Custom({code})")),
                "expected Custom({code}), got {s}\nlogs: {:#?}",
                e.meta.logs
            );
        }
    }
}

pub fn assert_failed(res: &TransactionResult) {
    assert!(res.is_err(), "expected failure, tx succeeded");
}

pub fn fetch<T: AccountDeserialize>(svm: &LiteSVM, address: &Pubkey) -> T {
    let account = svm.get_account(address).expect("account exists");
    T::try_deserialize(&mut account.data.as_slice()).unwrap()
}

/// A closed account is either gone or zero-lamport / empty.
pub fn is_closed(svm: &LiteSVM, address: &Pubkey) -> bool {
    match svm.get_account(address) {
        None => true,
        Some(a) => a.lamports == 0 && a.data.is_empty(),
    }
}

pub fn lamports(svm: &LiteSVM, address: &Pubkey) -> u64 {
    svm.get_account(address).map(|a| a.lamports).unwrap_or(0)
}

// ------------------------------------------------------------------ PDAs

pub fn config_pda() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &platform_core::id()).0
}

pub fn treasury_pda() -> Pubkey {
    Pubkey::find_program_address(&[TREASURY_SEED], &platform_core::id()).0
}

pub fn role_pda(role: Role, member: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[ROLE_SEED, &[role as u8], member.as_ref()],
        &platform_core::id(),
    )
    .0
}

// ---------------------------------------------------------- instructions

pub fn ix_initialize(
    authority: &Pubkey,
    boost_fee_bps: u16,
    ad_fee_bps: u16,
    first_admin: &Pubkey,
) -> Instruction {
    ix_initialize_with(
        authority,
        &platform_core::id(),
        &program_data_address(&platform_core::id()),
        boost_fee_bps,
        ad_fee_bps,
        first_admin,
    )
}

/// Variant that lets tests substitute the `program` / `program_data` accounts.
pub fn ix_initialize_with(
    authority: &Pubkey,
    program: &Pubkey,
    program_data: &Pubkey,
    boost_fee_bps: u16,
    ad_fee_bps: u16,
    first_admin: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::Initialize {
            boost_fee_bps,
            ad_fee_bps,
            first_admin: *first_admin,
        }
        .data(),
        platform_core::accounts::Initialize {
            authority: *authority,
            program: *program,
            program_data: *program_data,
            config: config_pda(),
            treasury: treasury_pda(),
            admin_role: role_pda(Role::Admin, first_admin),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_grant_role(admin: &Pubkey, role: Role, member: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::GrantRole {
            role,
            member: *member,
        }
        .data(),
        platform_core::accounts::GrantRole {
            admin: *admin,
            admin_role: role_pda(Role::Admin, admin),
            config: config_pda(),
            role_member: role_pda(role, member),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_revoke_role(
    admin: &Pubkey,
    role: Role,
    member: &Pubkey,
    granted_by: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::RevokeRole {
            role,
            member: *member,
        }
        .data(),
        platform_core::accounts::RevokeRole {
            admin: *admin,
            admin_role: role_pda(Role::Admin, admin),
            config: config_pda(),
            role_member: role_pda(role, member),
            granted_by: *granted_by,
        }
        .to_account_metas(None),
    )
}

pub fn ix_renounce_role(member: &Pubkey, role: Role, granted_by: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::RenounceRole { role }.data(),
        platform_core::accounts::RenounceRole {
            member: *member,
            config: config_pda(),
            role_member: role_pda(role, member),
            granted_by: *granted_by,
        }
        .to_account_metas(None),
    )
}

pub fn ix_set_fee(
    signer: &Pubkey,
    auth_role_kind: Role,
    kind: FeeKind,
    fee_bps: u16,
) -> Instruction {
    ix_set_fee_with_role_account(
        signer,
        &role_pda(auth_role_kind, signer),
        auth_role_kind,
        kind,
        fee_bps,
    )
}

/// Variant that lets tests pass someone else's RoleMember PDA.
pub fn ix_set_fee_with_role_account(
    signer: &Pubkey,
    auth_role: &Pubkey,
    auth_role_kind: Role,
    kind: FeeKind,
    fee_bps: u16,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::SetFee {
            auth_role_kind,
            kind,
            fee_bps,
        }
        .data(),
        platform_core::accounts::SetFee {
            signer: *signer,
            auth_role: *auth_role,
            config: config_pda(),
        }
        .to_account_metas(None),
    )
}

/// Initializes with the default fees (boost 1% · ad 1% — DECISIONS rev 7) and a fresh
/// first Admin that is NOT the upgrade authority (X6). Returns the Admin keypair.
pub fn init_default(env: &mut Env) -> Keypair {
    let admin = funded(&mut env.svm);
    let ix = ix_initialize(&env.upgrade_authority.pubkey(), 100, 100, &admin.pubkey());
    let ua = env.upgrade_authority.insecure_clone();
    assert_ok(&send(&mut env.svm, &[ix], &[&ua]));
    admin
}
