use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::PlatformError,
    events::DefaultAdSet,
    state::{AdCreative, DefaultAd, Role, RoleMember},
};

/// On-chain validation of a default-ad creative (02 §2.7). `#[max_len]` only sizes the
/// account; lengths must be checked here so an over-long field gets a clear error
/// instead of a serialization failure. The full 3-layer `click_url` check (A14) is the
/// Worker's job — on chain we only insist on `https://`.
pub(crate) fn validate_creative(creative: &AdCreative) -> Result<()> {
    require!(
        creative.image_url.len() <= AD_IMAGE_URL_MAX_LEN as usize
            && creative.text.len() <= AD_TEXT_MAX_LEN as usize
            && creative.click_url.len() <= AD_CLICK_URL_MAX_LEN as usize,
        PlatformError::CreativeFieldTooLong
    );
    require!(
        creative.click_url.starts_with("https://"),
        PlatformError::InvalidClickUrl
    );
    require!(
        creative.image_sha256 != [0u8; 32],
        PlatformError::InvalidImageHash
    );
    Ok(())
}

/// Moderator | Admin creates the default ad for one slot size (rev 3 #4 · A11 · Q3).
#[derive(Accounts)]
#[instruction(auth_role_kind: Role, width: u16, height: u16)]
pub struct CreateDefaultAd<'info> {
    /// Acts under `auth_role_kind`; pays rent.
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::Moderator | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    #[account(
        init,
        payer = signer,
        space = 8 + DefaultAd::INIT_SPACE,
        seeds = [DEFAULT_AD_SEED, &width.to_le_bytes(), &height.to_le_bytes()],
        bump,
    )]
    pub default_ad: Account<'info, DefaultAd>,

    pub system_program: Program<'info, System>,
}

pub fn handle_create_default_ad(
    ctx: Context<CreateDefaultAd>,
    _auth_role_kind: Role,
    width: u16,
    height: u16,
    creative: AdCreative,
) -> Result<()> {
    require!(width > 0 && height > 0, PlatformError::InvalidAdSize);
    validate_creative(&creative)?;

    let ad = &mut ctx.accounts.default_ad;
    ad.version = ACCOUNT_VERSION;
    ad.bump = ctx.bumps.default_ad;
    ad.width = width;
    ad.height = height;
    ad.enabled = true;
    ad.updated_by = ctx.accounts.signer.key();
    ad.updated_at = Clock::get()?.unix_timestamp;
    ad.creative = creative;

    emit!(DefaultAdSet {
        width,
        height,
        enabled: true,
    });
    Ok(())
}

/// Moderator | Admin replaces the creative of an existing default ad.
#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct UpdateDefaultAd<'info> {
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::Moderator | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    /// Owner + discriminator suffice: platform-core only creates DefaultAd at its PDA.
    #[account(mut)]
    pub default_ad: Account<'info, DefaultAd>,
}

pub fn handle_update_default_ad(
    ctx: Context<UpdateDefaultAd>,
    _auth_role_kind: Role,
    creative: AdCreative,
) -> Result<()> {
    validate_creative(&creative)?;

    let ad = &mut ctx.accounts.default_ad;
    ad.creative = creative;
    ad.updated_by = ctx.accounts.signer.key();
    ad.updated_at = Clock::get()?.unix_timestamp;

    emit!(DefaultAdSet {
        width: ad.width,
        height: ad.height,
        enabled: ad.enabled,
    });
    Ok(())
}

/// Moderator | Admin turns a default ad off/on. Off = FE shows nothing for that size;
/// the account is kept (Nothing is Deleted).
#[derive(Accounts)]
#[instruction(auth_role_kind: Role)]
pub struct SetDefaultAdEnabled<'info> {
    pub signer: Signer<'info>,

    #[account(
        seeds = [ROLE_SEED, &[auth_role_kind as u8], signer.key().as_ref()],
        bump = auth_role.bump,
        constraint = matches!(auth_role_kind, Role::Moderator | Role::Admin)
            @ PlatformError::Unauthorized,
    )]
    pub auth_role: Account<'info, RoleMember>,

    #[account(mut)]
    pub default_ad: Account<'info, DefaultAd>,
}

pub fn handle_set_default_ad_enabled(
    ctx: Context<SetDefaultAdEnabled>,
    _auth_role_kind: Role,
    enabled: bool,
) -> Result<()> {
    let ad = &mut ctx.accounts.default_ad;
    ad.enabled = enabled;
    ad.updated_by = ctx.accounts.signer.key();
    ad.updated_at = Clock::get()?.unix_timestamp;

    emit!(DefaultAdSet {
        width: ad.width,
        height: ad.height,
        enabled,
    });
    Ok(())
}
