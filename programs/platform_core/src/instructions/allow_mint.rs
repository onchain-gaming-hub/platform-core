use {
    anchor_lang::prelude::*,
    anchor_spl::{
        associated_token::{self, get_associated_token_address_with_program_id, AssociatedToken},
        token_interface::{Mint, TokenInterface},
    },
};

use crate::{
    constants::*,
    error::PlatformError,
    events::MintAllowed,
    mint_policy::check_mint_extensions,
    state::{AllowedMint, PlatformConfig, Role, RoleMember, Treasury},
};

/// AllowlistManager | Admin allows a mint for boost (02 §2.5 · Q4 · Q5) and makes sure
/// the Treasury's ATA for it exists (X3), so consumers never `init_if_needed` it —
/// except for a pure 1/1 NFT, which never pays a platform fee (R14 · maintainer 2026-10-09).
/// The ATA is created with `create_idempotent`: anyone can pre-create a Treasury ATA,
/// and a plain `init` would then fail (DoS). Its rent is never refunded (R4).
#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct AllowMint<'info> {
    /// Acts under `auth_role_kind`; pays rent for `AllowedMint` (+ the ATA if new).
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::AllowlistManager | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, PlatformConfig>,

    #[account(seeds = [TREASURY_SEED], bump = config.treasury_bump)]
    pub treasury: Account<'info, Treasury>,

    /// SPL Token or Token-2022 mint, owned by `token_program`.
    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = signer,
        space = 8 + AllowedMint::INIT_SPACE,
        seeds = [BOOST_MINT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub allowed_mint: Account<'info, AllowedMint>,

    /// CHECK: created (idempotently) by the ATA program below; the address pins it to
    /// ATA(treasury, mint, token_program).
    #[account(
        mut,
        address = get_associated_token_address_with_program_id(
            &treasury.key(), &mint.key(), &token_program.key()
        ),
    )]
    pub treasury_ata: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_allow_mint(ctx: Context<AllowMint>, _auth_role_kind: Role) -> Result<()> {
    check_mint_extensions(&ctx.accounts.mint.to_account_info())?;

    if !is_pure_one_of_one(&ctx.accounts.mint) {
        create_treasury_ata(&ctx)?;
    }

    let mint = ctx.accounts.mint.key();
    let token_program = ctx.accounts.token_program.key();
    let allowed = &mut ctx.accounts.allowed_mint;
    allowed.version = ACCOUNT_VERSION;
    allowed.bump = ctx.bumps.allowed_mint;
    allowed.mint = mint;
    allowed.token_program = token_program;
    allowed.decimals = ctx.accounts.mint.decimals;
    allowed.added_by = ctx.accounts.signer.key();
    allowed.added_at = Clock::get()?.unix_timestamp;

    emit!(MintAllowed {
        mint,
        token_program,
    });
    Ok(())
}

/// A mint that can never be divided or grow: exactly one unit, no decimals and no mint
/// authority left. Platform fee on NFTs is 0% (R14), so its Treasury ATA would never be
/// used. Without the mint-authority condition supply could grow later, the mint would
/// owe fees, and `collect_fee` would fail for lack of the ATA — so such mints still get
/// one. The ATA can still be created by anyone later (idempotent) if ever needed.
fn is_pure_one_of_one(mint: &InterfaceAccount<Mint>) -> bool {
    mint.decimals == 0 && mint.supply == 1 && mint.mint_authority.is_none()
}

fn create_treasury_ata(ctx: &Context<AllowMint>) -> Result<()> {
    associated_token::create_idempotent(CpiContext::new(
        ctx.accounts.associated_token_program.key(),
        associated_token::Create {
            payer: ctx.accounts.signer.to_account_info(),
            associated_token: ctx.accounts.treasury_ata.to_account_info(),
            authority: ctx.accounts.treasury.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        },
    ))?;
    Ok(())
}
