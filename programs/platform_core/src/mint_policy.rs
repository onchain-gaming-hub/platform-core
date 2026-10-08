//! Which mints may be allowlisted for boost (Q4 · Q5 · 02 §2.5).
//!
//! SPL Token mints have no extensions → always fine. Token-2022 mints are fine unless
//! they carry an extension that can take, tax, block or hook transfers of the
//! platform's funds. Applies to every mint, NFTs included (maintainer 2026-10-08).
//!
//! The check runs once, at `allow_mint`, so it must also rule out anything that lets
//! the mint change shape later (a closable mint can be re-created with new extensions).

use {
    crate::error::PlatformError,
    anchor_lang::prelude::*,
    anchor_spl::token_2022::spl_token_2022::{
        self,
        extension::{
            default_account_state::DefaultAccountState, mint_close_authority::MintCloseAuthority,
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
        state::{AccountState, Mint},
    },
};

/// Rejected outright (Q5): a delegate that can move funds out of the Treasury ATA,
/// transfer fees that break the fee/payout math, foreign code inside our transfers,
/// and tokens that cannot be paid out at all.
const FORBIDDEN: [ExtensionType; 4] = [
    ExtensionType::PermanentDelegate,
    ExtensionType::TransferFeeConfig,
    ExtensionType::TransferHook,
    ExtensionType::NonTransferable,
];

pub fn check_mint_extensions(mint: &AccountInfo) -> Result<()> {
    if *mint.owner != spl_token_2022::ID {
        return Ok(());
    }
    let data = mint.try_borrow_data()?;
    let state = StateWithExtensions::<Mint>::unpack(&data)?;
    for extension in state.get_extension_types()? {
        if FORBIDDEN.contains(&extension) {
            msg!("rejected mint extension: {:?}", extension);
            return err!(PlatformError::UnsupportedMintExtension);
        }
        if extension == ExtensionType::MintCloseAuthority {
            // A closable mint can be closed at zero supply and re-created at the same
            // address WITH a forbidden extension — after it was allowlisted, while the
            // Treasury ATA and consumer vaults still point at it. Only allow it when no
            // one can ever close the mint (QA review, Phase 2).
            let close = state.get_extension::<MintCloseAuthority>()?;
            if Option::<Pubkey>::from(close.close_authority).is_some() {
                msg!("rejected mint extension: MintCloseAuthority with an authority set");
                return err!(PlatformError::UnsupportedMintExtension);
            }
        }
        if extension == ExtensionType::DefaultAccountState {
            // New token accounts (incl. our Treasury ATA) would start frozen.
            let default_state = state.get_extension::<DefaultAccountState>()?;
            if default_state.state == AccountState::Frozen as u8 {
                msg!("rejected mint extension: DefaultAccountState = Frozen");
                return err!(PlatformError::UnsupportedMintExtension);
            }
        }
    }
    Ok(())
}
