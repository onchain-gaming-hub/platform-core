use anchor_lang::prelude::*;

/// ⚠️ Variant order = error code (6000 + index). Consumers / FE may match on the
/// number → append only, never reorder (specs/designs/02 §4).
#[error_code]
pub enum PlatformError {
    #[msg("Signer does not hold the required role")]
    Unauthorized, // 6000
    #[msg("Fee exceeds MAX_FEE_BPS")]
    FeeTooHigh, // 6001
    #[msg("Cannot remove the last Admin")]
    LastAdmin, // 6002
    #[msg("Game program account is not executable")]
    GameProgramNotExecutable, // 6003
    #[msg("Withdrawal would leave Treasury below rent-exempt minimum")]
    InsufficientTreasury, // 6004
    #[msg("Mint has a Token-2022 extension that is not allowed")]
    UnsupportedMintExtension, // 6005
    #[msg("Ad width and height must be greater than zero")]
    InvalidAdSize, // 6006
    #[msg("Ad creative field exceeds its max length")]
    CreativeFieldTooLong, // 6007
    #[msg("Ad click_url must start with https://")]
    InvalidClickUrl, // 6008
    #[msg("Ad image_sha256 must not be all zeros")]
    InvalidImageHash, // 6009
    #[msg("Arithmetic overflow")]
    MathOverflow, // 6010
    #[msg("Role member must not be the default (all-zero) pubkey")]
    InvalidMember, // 6011
}
