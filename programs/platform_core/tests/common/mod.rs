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

/// Reads a program binary from `target/deploy` at RUNTIME (not `include_bytes!`):
/// `anchor build` compiles every test target to produce the IDL, so a compile-time
/// include of a not-yet-built .so would break the build itself.
pub fn program_so(name: &str) -> Vec<u8> {
    let path = format!("{}/../deploy/{name}.so", env!("CARGO_TARGET_TMPDIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e} — build first: ./scripts/test.sh"))
}

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
    let bytes = &program_so("platform_core");
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

// ================================================================ Phase 2

use {
    anchor_lang::solana_program::{instruction::AccountMeta, system_instruction},
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id,
            spl_associated_token_account::instruction::create_associated_token_account,
        },
        token_2022::spl_token_2022::{
            self,
            extension::{
                default_account_state::instruction::initialize_default_account_state,
                metadata_pointer, transfer_fee::instruction::initialize_transfer_fee_config,
                transfer_hook, ExtensionType,
            },
            instruction as t22_ix,
            state::{Account as TokenAccountState, AccountState, Mint as MintState},
        },
    },
    platform_core::state::GameStatus,
};

pub const TOKEN_PROGRAM: Pubkey = anchor_spl::token::ID;
pub const TOKEN_2022_PROGRAM: Pubkey = anchor_spl::token_2022::ID;
pub const ATA_PROGRAM: Pubkey = anchor_spl::associated_token::ID;
/// Executable, but loaded by the non-upgradeable BPF loader → not a valid game program.
pub const MEMO_PROGRAM: Pubkey =
    anchor_lang::pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

/// Mint extensions a test mint can be created with.
#[derive(Clone, Copy, Debug)]
pub enum TestExt {
    PermanentDelegate,
    TransferFeeConfig,
    TransferHook,
    NonTransferable,
    DefaultFrozen,
    DefaultInitialized,
    MetadataPointer,
    /// Close authority set → mint can be closed and re-created (rejected).
    MintCloseAuthority,
    /// Extension present but no authority → mint can never be closed (allowed).
    MintCloseAuthorityNone,
    /// Q8 (decision 2026-10-09): accepted, like a freeze authority.
    Pausable,
}

impl TestExt {
    fn extension_type(self) -> ExtensionType {
        match self {
            TestExt::PermanentDelegate => ExtensionType::PermanentDelegate,
            TestExt::TransferFeeConfig => ExtensionType::TransferFeeConfig,
            TestExt::TransferHook => ExtensionType::TransferHook,
            TestExt::NonTransferable => ExtensionType::NonTransferable,
            TestExt::DefaultFrozen | TestExt::DefaultInitialized => {
                ExtensionType::DefaultAccountState
            }
            TestExt::MetadataPointer => ExtensionType::MetadataPointer,
            TestExt::MintCloseAuthority | TestExt::MintCloseAuthorityNone => {
                ExtensionType::MintCloseAuthority
            }
            TestExt::Pausable => ExtensionType::Pausable,
        }
    }

    fn init_ix(self, mint: &Pubkey, authority: &Pubkey) -> Instruction {
        let p = &TOKEN_2022_PROGRAM;
        match self {
            TestExt::PermanentDelegate => {
                t22_ix::initialize_permanent_delegate(p, mint, authority).unwrap()
            }
            TestExt::TransferFeeConfig => {
                initialize_transfer_fee_config(p, mint, Some(authority), Some(authority), 0, 0)
                    .unwrap()
            }
            TestExt::TransferHook => {
                transfer_hook::instruction::initialize(p, mint, Some(*authority), None).unwrap()
            }
            TestExt::NonTransferable => t22_ix::initialize_non_transferable_mint(p, mint).unwrap(),
            TestExt::DefaultFrozen => {
                initialize_default_account_state(p, mint, &AccountState::Frozen).unwrap()
            }
            TestExt::DefaultInitialized => {
                initialize_default_account_state(p, mint, &AccountState::Initialized).unwrap()
            }
            TestExt::MetadataPointer => {
                metadata_pointer::instruction::initialize(p, mint, Some(*authority), None).unwrap()
            }
            TestExt::MintCloseAuthority => {
                t22_ix::initialize_mint_close_authority(p, mint, Some(authority)).unwrap()
            }
            TestExt::MintCloseAuthorityNone => {
                t22_ix::initialize_mint_close_authority(p, mint, None).unwrap()
            }
            TestExt::Pausable => {
                spl_token_2022::extension::pausable::instruction::initialize(p, mint, authority)
                    .unwrap()
            }
        }
    }
}

/// Creates a mint owned by `token_program` with `exts` (Token-2022 only) and
/// `authority` as mint + freeze authority. Returns the mint address.
pub fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    token_program: &Pubkey,
    decimals: u8,
    exts: &[TestExt],
) -> Pubkey {
    let mint = Keypair::new();
    let types: Vec<ExtensionType> = exts.iter().map(|e| e.extension_type()).collect();
    let space = ExtensionType::try_calculate_account_len::<MintState>(&types).unwrap();
    let rent = svm.minimum_balance_for_rent_exemption(space);
    let mut ixs = vec![system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        space as u64,
        token_program,
    )];
    for e in exts {
        ixs.push(e.init_ix(&mint.pubkey(), &payer.pubkey()));
    }
    ixs.push(
        t22_ix::initialize_mint2(
            token_program,
            &mint.pubkey(),
            &payer.pubkey(),
            Some(&payer.pubkey()),
            decimals,
        )
        .unwrap(),
    );
    assert_ok(&send(svm, &ixs, &[payer, &mint]));
    mint.pubkey()
}

pub fn ata(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, token_program)
}

pub fn create_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Pubkey {
    let ix = create_associated_token_account(&payer.pubkey(), owner, mint, token_program);
    assert_ok(&send(svm, &[ix], &[payer]));
    ata(owner, mint, token_program)
}

/// `mint_authority` mints `amount` straight into `destination` (a token account).
pub fn mint_to(
    svm: &mut LiteSVM,
    mint_authority: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    token_program: &Pubkey,
    amount: u64,
    decimals: u8,
) {
    let ix = t22_ix::mint_to_checked(
        token_program,
        mint,
        destination,
        &mint_authority.pubkey(),
        &[],
        amount,
        decimals,
    )
    .unwrap();
    assert_ok(&send(svm, &[ix], &[mint_authority]));
}

pub fn token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let acc = svm.get_account(token_account).expect("token account");
    // Base `Account` layout (165 B) is identical for Token and Token-2022.
    use anchor_lang::solana_program::program_pack::Pack;
    TokenAccountState::unpack_from_slice(&acc.data[..TokenAccountState::LEN])
        .unwrap()
        .amount
}

pub fn token_account_owner(svm: &LiteSVM, token_account: &Pubkey) -> Pubkey {
    let acc = svm.get_account(token_account).expect("token account");
    use anchor_lang::solana_program::program_pack::Pack;
    TokenAccountState::unpack_from_slice(&acc.data[..TokenAccountState::LEN])
        .unwrap()
        .owner
}

pub fn game_pda(game_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[GAME_SEED, game_program.as_ref()], &platform_core::id()).0
}

pub fn allowed_mint_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[BOOST_MINT_SEED, mint.as_ref()], &platform_core::id()).0
}

/// Flip `is_signer` off for `key` in `ix` (simulates a missing co-signature).
pub fn unsign(ix: &mut Instruction, key: &Pubkey) {
    for m in ix.accounts.iter_mut() {
        if m.pubkey == *key {
            m.is_signer = false;
        }
    }
}

pub fn ix_register_game(
    signer: &Pubkey,
    auth_role_kind: Role,
    owner: &Pubkey,
    game_program: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::RegisterGame { auth_role_kind }.data(),
        platform_core::accounts::RegisterGame {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            owner: *owner,
            game_program: *game_program,
            game_entry: game_pda(game_program),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_set_game_status(
    signer: &Pubkey,
    auth_role_kind: Role,
    game_program: &Pubkey,
    status: GameStatus,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::SetGameStatus {
            auth_role_kind,
            status,
        }
        .data(),
        platform_core::accounts::SetGameStatus {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            game_entry: game_pda(game_program),
        }
        .to_account_metas(None),
    )
}

pub fn ix_transfer_game_owner(
    owner: &Pubkey,
    new_owner: &Pubkey,
    game_program: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::TransferGameOwner {}.data(),
        platform_core::accounts::TransferGameOwner {
            owner: *owner,
            new_owner: *new_owner,
            game_entry: game_pda(game_program),
        }
        .to_account_metas(None),
    )
}

pub fn ix_force_set_game_owner(
    admin: &Pubkey,
    game_program: &Pubkey,
    new_owner: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::ForceSetGameOwner {
            new_owner: *new_owner,
        }
        .data(),
        platform_core::accounts::ForceSetGameOwner {
            admin: *admin,
            admin_role: role_pda(Role::Admin, admin),
            game_entry: game_pda(game_program),
        }
        .to_account_metas(None),
    )
}

pub fn ix_allow_mint(
    signer: &Pubkey,
    auth_role_kind: Role,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::AllowMint { auth_role_kind }.data(),
        platform_core::accounts::AllowMint {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            config: config_pda(),
            treasury: treasury_pda(),
            mint: *mint,
            allowed_mint: allowed_mint_pda(mint),
            treasury_ata: ata(&treasury_pda(), mint, token_program),
            token_program: *token_program,
            associated_token_program: ATA_PROGRAM,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_disallow_mint(
    signer: &Pubkey,
    auth_role_kind: Role,
    mint: &Pubkey,
    added_by: &Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::DisallowMint { auth_role_kind }.data(),
        platform_core::accounts::DisallowMint {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            allowed_mint: allowed_mint_pda(mint),
            added_by: *added_by,
        }
        .to_account_metas(None),
    )
}

pub fn ix_withdraw_sol(admin: &Pubkey, to: &Pubkey, amount: u64) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::WithdrawSol { amount }.data(),
        platform_core::accounts::WithdrawSol {
            admin: *admin,
            admin_role: role_pda(Role::Admin, admin),
            config: config_pda(),
            treasury: treasury_pda(),
            to: *to,
        }
        .to_account_metas(None),
    )
}

pub fn ix_withdraw_token(
    admin: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
    to: &Pubkey,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::WithdrawToken { amount }.data(),
        platform_core::accounts::WithdrawToken {
            admin: *admin,
            admin_role: role_pda(Role::Admin, admin),
            config: config_pda(),
            treasury: treasury_pda(),
            mint: *mint,
            treasury_ata: ata(&treasury_pda(), mint, token_program),
            to: *to,
            token_program: *token_program,
        }
        .to_account_metas(None),
    )
}

/// Replace the account at `index` in `ix` (account-substitution tests).
pub fn replace_account(ix: &mut Instruction, index: usize, key: Pubkey) {
    let m: &mut AccountMeta = &mut ix.accounts[index];
    m.pubkey = key;
}

/// Grants `role` to a fresh funded keypair and returns it.
pub fn holder(env: &mut Env, admin: &Keypair, role: Role) -> Keypair {
    let k = funded(&mut env.svm);
    assert_ok(&send(
        &mut env.svm,
        &[ix_grant_role(&admin.pubkey(), role, &k.pubkey())],
        &[admin],
    ));
    k
}

/// Deploys a copy of our .so at a fresh address through the *upgradeable* loader — a
/// stand-in for a real game program.
pub fn deploy_game(svm: &mut LiteSVM) -> Pubkey {
    let id = Pubkey::new_unique();
    let bytes = &program_so("platform_core");
    svm.add_program(id, bytes).unwrap();
    id
}

pub fn spl_token_2022_id() -> Pubkey {
    spl_token_2022::ID
}

// ================================================================ Phase 3

use platform_core::state::AdCreative;

pub fn default_ad_pda(width: u16, height: u16) -> Pubkey {
    Pubkey::find_program_address(
        &[DEFAULT_AD_SEED, &width.to_le_bytes(), &height.to_le_bytes()],
        &platform_core::id(),
    )
    .0
}

pub fn creative(tag: &str) -> AdCreative {
    AdCreative {
        image_url: format!("https://r2.hub/ads/{tag}.png"),
        image_sha256: [7u8; 32],
        text: format!("default ad {tag}"),
        click_url: "https://hub.example/play".into(),
    }
}

pub fn ix_create_default_ad(
    signer: &Pubkey,
    auth_role_kind: Role,
    width: u16,
    height: u16,
    creative: AdCreative,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::CreateDefaultAd {
            auth_role_kind,
            width,
            height,
            creative,
        }
        .data(),
        platform_core::accounts::CreateDefaultAd {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            default_ad: default_ad_pda(width, height),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn ix_update_default_ad(
    signer: &Pubkey,
    auth_role_kind: Role,
    default_ad: &Pubkey,
    creative: AdCreative,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::UpdateDefaultAd {
            auth_role_kind,
            creative,
        }
        .data(),
        platform_core::accounts::UpdateDefaultAd {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            default_ad: *default_ad,
        }
        .to_account_metas(None),
    )
}

pub fn ix_set_default_ad_enabled(
    signer: &Pubkey,
    auth_role_kind: Role,
    default_ad: &Pubkey,
    enabled: bool,
) -> Instruction {
    Instruction::new_with_bytes(
        platform_core::id(),
        &platform_core::instruction::SetDefaultAdEnabled {
            auth_role_kind,
            enabled,
        }
        .data(),
        platform_core::accounts::SetDefaultAdEnabled {
            signer: *signer,
            auth_role: role_pda(auth_role_kind, signer),
            default_ad: *default_ad,
        }
        .to_account_metas(None),
    )
}

/// Loads the TEST ONLY mock consumer (built by
/// `cargo build-sbf --manifest-path tests/mock-consumer/Cargo.toml --sbf-out-dir target/deploy`)
/// through the upgradeable loader, so it can also be registered as a game.
pub fn load_mock_consumer(svm: &mut LiteSVM) {
    let bytes = &program_so("mock_consumer");
    svm.add_program(mock_consumer::ID, bytes).unwrap();
}

pub fn mock_game_authority() -> Pubkey {
    Pubkey::find_program_address(&[mock_consumer::GAME_AUTHORITY_SEED], &mock_consumer::ID).0
}

pub fn mock_snapshot_pda(game_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[mock_consumer::SNAPSHOT_SEED, game_program.as_ref()],
        &mock_consumer::ID,
    )
    .0
}

pub fn mock_escrow_pda() -> Pubkey {
    Pubkey::find_program_address(&[mock_consumer::ESCROW_SEED], &mock_consumer::ID).0
}

fn mock_ix(data: Vec<u8>, accounts: Vec<AccountMeta>) -> Instruction {
    Instruction {
        program_id: mock_consumer::ID,
        accounts,
        data,
    }
}

pub fn mix_game_open_pool(payer: &Pubkey) -> Instruction {
    let game = mock_consumer::ID;
    mock_ix(
        mock_consumer::instruction::GameOpenPool {}.data(),
        mock_consumer::accounts::GameOpenPool {
            payer: *payer,
            game_authority: mock_game_authority(),
            game_program: game,
            game_entry: game_pda(&game),
            config: config_pda(),
            snapshot: mock_snapshot_pda(&game),
            system_program: system_program::ID,
            this_program: mock_consumer::ID,
        }
        .to_account_metas(None),
    )
}

/// Calls the consumer instruction directly, with `game_authority` chosen by the caller.
pub fn mix_open_pool_direct(payer: &Pubkey, game_authority: &Pubkey) -> Instruction {
    let game = mock_consumer::ID;
    mock_ix(
        mock_consumer::instruction::OpenPool {}.data(),
        mock_consumer::accounts::OpenPool {
            payer: *payer,
            game_authority: *game_authority,
            game_program: game,
            game_entry: game_pda(&game),
            config: config_pda(),
            snapshot: mock_snapshot_pda(&game),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn mix_owner_action(game_program: &Pubkey, game_owner: &Pubkey) -> Instruction {
    mock_ix(
        mock_consumer::instruction::OwnerAction {}.data(),
        mock_consumer::accounts::OwnerAction {
            game_program: *game_program,
            game_entry: game_pda(game_program),
            game_owner: *game_owner,
        }
        .to_account_metas(None),
    )
}

pub fn mix_moderate(moderator: &Pubkey, moderator_role: &Pubkey) -> Instruction {
    mock_ix(
        mock_consumer::instruction::Moderate {}.data(),
        mock_consumer::accounts::Moderate {
            moderator_role: *moderator_role,
            moderator: *moderator,
        }
        .to_account_metas(None),
    )
}

pub fn mix_pay_fee_from_user(user: &Pubkey, treasury: &Pubkey, amount: u64) -> Instruction {
    mock_ix(
        mock_consumer::instruction::PayFeeFromUser { amount }.data(),
        mock_consumer::accounts::PayFeeFromUser {
            user: *user,
            config: config_pda(),
            treasury: *treasury,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn mix_init_escrow(payer: &Pubkey) -> Instruction {
    mock_ix(
        mock_consumer::instruction::InitEscrow {}.data(),
        mock_consumer::accounts::InitEscrow {
            payer: *payer,
            escrow: mock_escrow_pda(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn mix_settle(
    game_program: &Pubkey,
    game_owner: &Pubkey,
    amount: u64,
    fee_bps: u16,
) -> Instruction {
    mock_ix(
        mock_consumer::instruction::Settle { amount, fee_bps }.data(),
        mock_consumer::accounts::Settle {
            escrow: mock_escrow_pda(),
            game_program: *game_program,
            game_entry: game_pda(game_program),
            game_owner: *game_owner,
            config: config_pda(),
            treasury: treasury_pda(),
        }
        .to_account_metas(None),
    )
}

pub fn mix_settle_typed_treasury(fee: u64) -> Instruction {
    mock_ix(
        mock_consumer::instruction::SettleTypedTreasury { fee }.data(),
        mock_consumer::accounts::SettleTypedTreasury {
            escrow: mock_escrow_pda(),
            config: config_pda(),
            treasury: treasury_pda(),
        }
        .to_account_metas(None),
    )
}

pub fn mix_boost_check(mint: &Pubkey) -> Instruction {
    mock_ix(
        mock_consumer::instruction::BoostCheck {}.data(),
        mock_consumer::accounts::BoostCheck {
            mint: *mint,
            allowed_mint: allowed_mint_pda(mint),
        }
        .to_account_metas(None),
    )
}

pub fn mix_collect_fee_token(
    user: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
    user_ata: &Pubkey,
    amount: u64,
) -> Instruction {
    mock_ix(
        mock_consumer::instruction::CollectFeeToken { amount }.data(),
        mock_consumer::accounts::CollectFeeToken {
            user: *user,
            mint: *mint,
            config: config_pda(),
            treasury: treasury_pda(),
            treasury_ata: ata(&treasury_pda(), mint, token_program),
            user_ata: *user_ata,
            token_program: *token_program,
        }
        .to_account_metas(None),
    )
}

/// Removes the mint authority (makes supply permanent) — used to build pure 1/1 NFTs.
pub fn drop_mint_authority(
    svm: &mut LiteSVM,
    authority: &Keypair,
    mint: &Pubkey,
    token_program: &Pubkey,
) {
    let ix = t22_ix::set_authority(
        token_program,
        mint,
        None,
        t22_ix::AuthorityType::MintTokens,
        &authority.pubkey(),
        &[],
    )
    .unwrap();
    assert_ok(&send(svm, &[ix], &[authority]));
}
