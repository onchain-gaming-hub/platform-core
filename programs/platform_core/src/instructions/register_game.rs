use anchor_lang::{prelude::*, solana_program::bpf_loader_upgradeable};

use crate::{
    constants::*,
    error::PlatformError,
    events::GameRegistered,
    state::{GameEntry, GameStatus, Role, RoleMember},
};

/// GameRegistrar | Admin registers a game program (02 §2.4 · X5). The game `owner`
/// must co-sign — it receives ad revenue, so a mistyped key would lose money forever.
/// `game_authority` (the game's PDA signer, pattern R1) is derived once here so that
/// consumers never have to call `find_program_address`.
/// Programs that can never be registered as a game, whatever their loader.
/// Core-BPF system programs are owned by the upgradeable loader like any game, so they
/// must be listed explicitly. ALT/Config charge rent to the (relayer) fee payer.
const NON_GAME_PROGRAMS: [Pubkey; 7] = [
    crate::ID,
    anchor_spl::token::ID,
    anchor_spl::token_2022::ID,
    anchor_spl::associated_token::ID,
    pubkey!("Stake11111111111111111111111111111111111111"),
    pubkey!("AddressLookupTab1e1111111111111111111111111"),
    pubkey!("Config1111111111111111111111111111111111111"),
];

#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct RegisterGame<'info> {
    /// Acts under `auth_role_kind`; pays rent and is recorded as `registered_by`.
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::GameRegistrar | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    /// Future `GameEntry.owner`.
    pub owner: Signer<'info>,

    /// CHECK: only its key, `executable` flag and owner are used. Must be a deployed
    /// (upgradeable-loader) program, and not one of the value-moving system programs
    /// or platform-core itself: every Active game lands on the relayer's co-sign
    /// allowlist, so these must never be registrable (fee drain / fund movement).
    /// ⚠️ The owner check alone is NOT enough: SPL Token, Token-2022 and Stake are
    /// core-BPF programs owned by the upgradeable loader too — hence the denylist.
    #[account(
        constraint = game_program.executable @ PlatformError::GameProgramNotExecutable,
        constraint = *game_program.owner == bpf_loader_upgradeable::ID
            @ PlatformError::InvalidGameProgram,
        constraint = !NON_GAME_PROGRAMS.contains(&game_program.key())
            @ PlatformError::InvalidGameProgram,
    )]
    pub game_program: UncheckedAccount<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + GameEntry::INIT_SPACE,
        seeds = [GAME_SEED, game_program.key().as_ref()],
        bump,
    )]
    pub game_entry: Account<'info, GameEntry>,

    pub system_program: Program<'info, System>,
}

pub fn handle_register_game(ctx: Context<RegisterGame>, _auth_role_kind: Role) -> Result<()> {
    let game_program = ctx.accounts.game_program.key();
    let owner = ctx.accounts.owner.key();
    let (game_authority, game_authority_bump) =
        Pubkey::find_program_address(&[GAME_AUTHORITY_SEED], &game_program);
    let now = Clock::get()?.unix_timestamp;

    let entry = &mut ctx.accounts.game_entry;
    entry.version = ACCOUNT_VERSION;
    entry.bump = ctx.bumps.game_entry;
    entry.status = GameStatus::Active;
    entry.game_program = game_program;
    entry.owner = owner;
    entry.game_authority = game_authority;
    entry.game_authority_bump = game_authority_bump;
    entry.registered_by = ctx.accounts.signer.key();
    entry.registered_at = now;
    entry.updated_at = now;

    emit!(GameRegistered {
        game_program,
        owner,
        game_authority,
    });
    Ok(())
}
