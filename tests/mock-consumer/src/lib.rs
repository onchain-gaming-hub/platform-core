//! TEST ONLY — never deploy.
//!
//! A stand-in for rewards-pool / ad-pool / lobby / a game program. Every account check
//! below is copied from skills/platform-core-integration/SKILL.md §2 so the snippets
//! we publish to consumers are proven to compile and behave as documented.
//! It reads platform-core ONLY through `declare_program!` + `idls/platform_core.json`
//! (no crate dependency), exactly like a real consumer.
#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

declare_id!("Bdf6JfWt4n2pGAfyR1zxNe33bmaWcTwckMwLWe52uW3q");

declare_program!(platform_core);
use platform_core::{
    accounts::{AllowedMint, GameEntry, PlatformConfig, RoleMember, Treasury},
    types::{GameStatus, Role},
};

pub const SNAPSHOT_SEED: &[u8] = b"snapshot";
pub const ESCROW_SEED: &[u8] = b"escrow";
/// The hub-wide PDA-signer seed of every game program (R1 · X5).
pub const GAME_AUTHORITY_SEED: &[u8] = b"game_authority";

#[error_code]
pub enum MockError {
    GameNotActive,
    NotGameAuthority,
    NotGameOwner,
    MathOverflow,
}

#[program]
pub mod mock_consumer {
    use super::*;

    /// Acting as the GAME: CPI into this same program's `open_pool`, signed by our
    /// `[GAME_AUTHORITY_SEED]` PDA — what a real game does towards rewards-pool (R1).
    pub fn game_open_pool(ctx: Context<GameOpenPool>) -> Result<()> {
        // `crate::cpi` is gated behind the `cpi` feature (which also drops the
        // entrypoint), so the self-CPI is built from the always-available client modules.
        let ix = anchor_lang::solana_program::instruction::Instruction {
            program_id: crate::ID,
            accounts: crate::accounts::OpenPool {
                payer: ctx.accounts.payer.key(),
                game_authority: ctx.accounts.game_authority.key(),
                game_program: ctx.accounts.game_program.key(),
                game_entry: ctx.accounts.game_entry.key(),
                config: ctx.accounts.config.key(),
                snapshot: ctx.accounts.snapshot.key(),
                system_program: ctx.accounts.system_program.key(),
            }
            .to_account_metas(None),
            data: anchor_lang::InstructionData::data(&crate::instruction::OpenPool {}),
        };
        let seeds: &[&[&[u8]]] = &[&[GAME_AUTHORITY_SEED, &[ctx.bumps.game_authority]]];
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.game_authority.to_account_info(),
                ctx.accounts.game_program.to_account_info(),
                ctx.accounts.game_entry.to_account_info(),
                ctx.accounts.config.to_account_info(),
                ctx.accounts.snapshot.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.this_program.to_account_info(),
            ],
            seeds,
        )?;
        Ok(())
    }

    /// Acting as a CONSUMER (≈ rewards-pool `open_pool`): §2.1 game Active,
    /// §2.2 caller is that game's PDA signer, §2.5 fee snapshot.
    pub fn open_pool(ctx: Context<OpenPool>) -> Result<()> {
        let snapshot = &mut ctx.accounts.snapshot;
        snapshot.boost_fee_bps = ctx.accounts.config.boost_fee_bps;
        snapshot.ad_fee_bps = ctx.accounts.config.ad_fee_bps;
        snapshot.game_program = ctx.accounts.game_entry.game_program;
        Ok(())
    }

    /// §2.3 the game owner must sign (≈ ad-pool `init_ad_config`, rewards-pool
    /// `fund_game_vault`) — and the game must be Active (starting something new).
    pub fn owner_action(_ctx: Context<OwnerAction>) -> Result<()> {
        Ok(())
    }

    /// §2.4 exact role check (≈ ad-pool approve/reject by the moderation Worker).
    pub fn moderate(_ctx: Context<Moderate>) -> Result<()> {
        Ok(())
    }

    /// §2.6 SOL fee straight from a user wallet → Treasury (system transfer CPI).
    pub fn pay_fee_from_user(ctx: Context<PayFeeFromUser>, amount: u64) -> Result<()> {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            amount,
        )
    }

    /// Creates this program's escrow PDA (holds SOL like `AdSlotAuction` / `MatchPool`).
    pub fn init_escrow(ctx: Context<InitEscrow>) -> Result<()> {
        ctx.accounts.escrow.bump = ctx.bumps.escrow;
        Ok(())
    }

    /// ≈ ad-pool `settle`: §2.3 pays `GameEntry.owner` read *now*, no Active check
    /// (works while Suspended — rule 3), §2.6 fee from our escrow via sub/add_lamports,
    /// §3 fee math (floor).
    pub fn settle(ctx: Context<Settle>, amount: u64, fee_bps: u16) -> Result<()> {
        // A real consumer uses its stored snapshot (≤ MAX_FEE_BPS); guard the test input.
        require!(fee_bps <= 10_000, MockError::MathOverflow);
        let fee = (amount as u128)
            .checked_mul(fee_bps as u128)
            .ok_or(MockError::MathOverflow)?
            / 10_000u128;
        let fee = u64::try_from(fee).map_err(|_| MockError::MathOverflow)?;
        let net = amount - fee;
        ctx.accounts.escrow.sub_lamports(amount)?;
        ctx.accounts.treasury.add_lamports(fee)?;
        ctx.accounts.game_owner.add_lamports(net)?;
        Ok(())
    }

    /// Same as `settle`'s fee leg but with the Treasury typed as `Account<Treasury>`
    /// (mut) instead of `UncheckedAccount` — answers 03 §2.6's open question of what
    /// Anchor's exit does with a foreign-owned typed account.
    pub fn settle_typed_treasury(ctx: Context<SettleTypedTreasury>, fee: u64) -> Result<()> {
        ctx.accounts.escrow.sub_lamports(fee)?;
        ctx.accounts.treasury.add_lamports(fee)?;
        Ok(())
    }

    /// §2.6 boost side (≈ rewards-pool `boost`): the mint must be allowlisted NOW.
    /// No Treasury accounts here — fees are collected later by `collect_fee_token`.
    pub fn boost_check(_ctx: Context<BoostCheck>) -> Result<()> {
        Ok(())
    }

    /// §2.6 fee side (≈ rewards-pool `collect_fee`): token fee → the Treasury ATA that
    /// platform-core created at `allow_mint` (X3). NO AllowedMint here (rule 3): a mint
    /// disallowed after the boost must still be collectable.
    pub fn collect_fee_token(ctx: Context<CollectFeeToken>, amount: u64) -> Result<()> {
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.user_ata.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.treasury_ata.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )
    }
}

#[account]
#[derive(InitSpace)]
pub struct Snapshot {
    pub game_program: Pubkey,
    pub boost_fee_bps: u16,
    pub ad_fee_bps: u16,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub bump: u8,
}

#[derive(Accounts)]
pub struct GameOpenPool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: our own PDA signer; signs the CPI below.
    #[account(seeds = [GAME_AUTHORITY_SEED], bump)]
    pub game_authority: UncheckedAccount<'info>,
    /// CHECK: this program (it is the registered game).
    #[account(address = crate::ID)]
    pub game_program: UncheckedAccount<'info>,
    /// CHECK: validated by `open_pool`.
    pub game_entry: UncheckedAccount<'info>,
    /// CHECK: validated by `open_pool`.
    pub config: UncheckedAccount<'info>,
    /// CHECK: created by `open_pool`.
    #[account(mut)]
    pub snapshot: UncheckedAccount<'info>,
    /// CHECK: forwarded to the CPI (`open_pool` checks it is the System program).
    pub system_program: UncheckedAccount<'info>,
    /// CHECK: this program, as the CPI target.
    #[account(address = crate::ID)]
    pub this_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct OpenPool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // --- 03 §2.2 ------------------------------------------------------------
    #[account(address = game_entry.game_authority @ MockError::NotGameAuthority)]
    pub game_authority: Signer<'info>,

    /// CHECK: only its key is used, to derive the GameEntry seeds.
    pub game_program: UncheckedAccount<'info>,

    // --- 03 §2.1 ------------------------------------------------------------
    #[account(
        seeds = [b"game", game_program.key().as_ref()],
        bump = game_entry.bump,
        seeds::program = platform_core::ID,
        constraint = matches!(game_entry.status, GameStatus::Active) @ MockError::GameNotActive,
    )]
    pub game_entry: Account<'info, GameEntry>,

    // --- 03 §2.5 ------------------------------------------------------------
    #[account(seeds = [b"config"], bump = config.bump, seeds::program = platform_core::ID)]
    pub config: Account<'info, PlatformConfig>,

    #[account(
        init,
        payer = payer,
        space = 8 + Snapshot::INIT_SPACE,
        seeds = [SNAPSHOT_SEED, game_program.key().as_ref()],
        bump,
    )]
    pub snapshot: Account<'info, Snapshot>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OwnerAction<'info> {
    /// CHECK: only its key is used, to derive the GameEntry seeds.
    pub game_program: UncheckedAccount<'info>,

    #[account(
        seeds = [b"game", game_program.key().as_ref()],
        bump = game_entry.bump,
        seeds::program = platform_core::ID,
        constraint = matches!(game_entry.status, GameStatus::Active) @ MockError::GameNotActive,
    )]
    pub game_entry: Account<'info, GameEntry>,

    // --- 03 §2.3 (owner signs) ---------------------------------------------
    #[account(address = game_entry.owner @ MockError::NotGameOwner)]
    pub game_owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct Moderate<'info> {
    // --- 03 §2.4 ------------------------------------------------------------
    #[account(
        seeds = [b"role", &[Role::Moderator as u8], moderator.key().as_ref()],
        bump = moderator_role.bump,
        seeds::program = platform_core::ID,
    )]
    pub moderator_role: Account<'info, RoleMember>,
    pub moderator: Signer<'info>,
}

#[derive(Accounts)]
pub struct PayFeeFromUser<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump, seeds::program = platform_core::ID)]
    pub config: Account<'info, PlatformConfig>,

    // --- 03 §2.6 ------------------------------------------------------------
    /// CHECK: ตรวจด้วย seeds + seeds::program
    #[account(mut, seeds = [b"treasury"], bump = config.treasury_bump, seeds::program = platform_core::ID)]
    pub treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitEscrow<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(init, payer = payer, space = 8 + Escrow::INIT_SPACE, seeds = [ESCROW_SEED], bump)]
    pub escrow: Account<'info, Escrow>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Settle<'info> {
    #[account(mut, seeds = [ESCROW_SEED], bump = escrow.bump)]
    pub escrow: Account<'info, Escrow>,

    /// CHECK: only its key is used, to derive the GameEntry seeds.
    pub game_program: UncheckedAccount<'info>,

    // --- 03 §2.3: settle reads `owner` but does NOT require Active (rule 3) ---
    #[account(
        seeds = [b"game", game_program.key().as_ref()],
        bump = game_entry.bump,
        seeds::program = platform_core::ID,
    )]
    pub game_entry: Account<'info, GameEntry>,

    /// CHECK: K-4 / ad-pool A29 — pinned by `address`; UncheckedAccount (not
    /// SystemAccount) so settle never gets stuck if the owner isn't system-owned.
    #[account(mut, address = game_entry.owner @ MockError::NotGameOwner)]
    pub game_owner: UncheckedAccount<'info>,

    #[account(seeds = [b"config"], bump = config.bump, seeds::program = platform_core::ID)]
    pub config: Account<'info, PlatformConfig>,

    /// CHECK: ตรวจด้วย seeds + seeds::program
    #[account(mut, seeds = [b"treasury"], bump = config.treasury_bump, seeds::program = platform_core::ID)]
    pub treasury: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SettleTypedTreasury<'info> {
    #[account(mut, seeds = [ESCROW_SEED], bump = escrow.bump)]
    pub escrow: Account<'info, Escrow>,

    #[account(seeds = [b"config"], bump = config.bump, seeds::program = platform_core::ID)]
    pub config: Account<'info, PlatformConfig>,

    #[account(mut, seeds = [b"treasury"], bump = config.treasury_bump, seeds::program = platform_core::ID)]
    pub treasury: Account<'info, Treasury>,
}

#[derive(Accounts)]
pub struct BoostCheck<'info> {
    pub mint: InterfaceAccount<'info, Mint>,

    // --- 03 §2.6 (boost) ------------------------------------------------------
    #[account(
        seeds = [b"boost_mint", mint.key().as_ref()],
        bump = allowed_mint.bump,
        seeds::program = platform_core::ID,
    )]
    pub allowed_mint: Account<'info, AllowedMint>,
}

#[derive(Accounts)]
pub struct CollectFeeToken<'info> {
    pub user: Signer<'info>,

    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,

    // --- 03 §2.6 (collect_fee) — REQUIRED, every line -------------------------
    #[account(seeds = [b"config"], bump = config.bump, seeds::program = platform_core::ID)]
    pub config: Account<'info, PlatformConfig>,

    /// CHECK: ตรวจด้วย seeds + seeds::program (ATA authority only)
    #[account(seeds = [b"treasury"], bump = config.treasury_bump, seeds::program = platform_core::ID)]
    pub treasury: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = treasury,
        associated_token::token_program = token_program,
    )]
    pub treasury_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, token::mint = mint, token::authority = user, token::token_program = token_program)]
    pub user_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
