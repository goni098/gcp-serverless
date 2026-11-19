//! Mint and token account validators, available with the `spl` feature.
//!
//! `#[mint(...)]` and `#[token_account(...)]` require `program`. Both validate
//! the program, account ownership, data layout, and token state. See
//! [`ensure_mint_constraints`] and [`ensure_token_account`] for the default errors.
//!
//! ```
//! use pinocchio_constraints::{AccountView, Accounts};
//!
//! #[derive(Accounts)]
//! struct TransferAccounts<'acc> {
//!     #[account(signer)]
//!     payer: &'acc AccountView,
//!
//!     #[mint(program = token_program)]
//!     mint: &'acc AccountView,
//!
//!     #[account(mut)]
//!     #[token_account(program = token_program, mint = mint, authority = payer)]
//!     source: &'acc AccountView,
//!
//!     token_program: &'acc AccountView,
//! }
//! ```

use pinocchio::ProgramResult;
use pinocchio_token::ID as TOKEN_ID;
use pinocchio_token::state::{Account, AccountState, Mint};
use pinocchio_token_2022::ID as TOKEN_2022_ID;
use pinocchio_token_2022::state::{
    Account as Token2022Account, AccountState as Token2022AccountState, Mint as Token2022Mint,
};

use crate::{AccountView, Address, ProgramError};

enum TokenProgram {
    SplToken,
    Token2022,
}

impl TokenProgram {
    fn from_account(program: &AccountView) -> Result<Self, ProgramError> {
        if !program.executable() {
            return Err(ProgramError::IncorrectProgramId);
        }
        match program.address() {
            &TOKEN_ID => Ok(Self::SplToken),
            &TOKEN_2022_ID => Ok(Self::Token2022),
            _ => Err(ProgramError::IncorrectProgramId),
        }
    }
}

/// Require an executable SPL Token or Token-2022 program account.
/// Returns [`ProgramError::IncorrectProgramId`] for an unsupported address or a non-executable account.
pub fn ensure_spl_program(program: &AccountView) -> ProgramResult {
    TokenProgram::from_account(program).map(|_| ())
}

fn validate_program(
    account: &AccountView,
    program: &AccountView,
) -> Result<TokenProgram, ProgramError> {
    if !account.owned_by(program.address()) {
        return Err(ProgramError::IncorrectProgramId);
    }
    TokenProgram::from_account(program)
}

/// Validate an initialized mint and return its decimals.
/// See [`ensure_mint_constraints`] for the validation errors.
pub fn ensure_mint(
    mint: &AccountView,
    token_program: &AccountView,
    decimals: Option<u8>,
) -> Result<u8, ProgramError> {
    ensure_mint_constraints(mint, token_program, decimals, None, None)
}

/// Validate an initialized mint and optionally check its decimals and authorities.
///
/// - [`ProgramError::IncorrectProgramId`] if the program is unsupported,
///   non-executable, or differs from the mint's runtime owner.
/// - [`ProgramError::AccountBorrowFailed`] if the mint data cannot be borrowed.
/// - [`ProgramError::InvalidAccountData`] if the layout is invalid, the decimals
///   differ, or a requested authority is absent or differs from the expected address.
/// - [`ProgramError::UninitializedAccount`] if the mint initialization byte is any value other than `1`.
pub fn ensure_mint_constraints(
    mint: &AccountView,
    token_program: &AccountView,
    decimals: Option<u8>,
    authority: Option<&AccountView>,
    freeze_authority: Option<&AccountView>,
) -> Result<u8, ProgramError> {
    let check = |initialized: bool,
                 actual_decimals: u8,
                 actual_authority: Option<&Address>,
                 actual_freeze_authority: Option<&Address>| {
        if !initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        if decimals.is_some_and(|expected| actual_decimals != expected)
            || authority.is_some_and(|expected| actual_authority != Some(expected.address()))
            || freeze_authority
                .is_some_and(|expected| actual_freeze_authority != Some(expected.address()))
        {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(actual_decimals)
    };

    match validate_program(mint, token_program)? {
        TokenProgram::SplToken => {
            let mint = Mint::from_account_view(mint)?;
            check(
                mint.is_initialized(),
                mint.decimals(),
                mint.mint_authority(),
                mint.freeze_authority(),
            )
        }
        TokenProgram::Token2022 => {
            let mint = Token2022Mint::from_account_view(mint)?;
            check(
                mint.is_initialized(),
                mint.decimals(),
                mint.mint_authority(),
                mint.freeze_authority(),
            )
        }
    }
}

/// Validate an initialized token account and optionally check its authority and mint.
/// Both `Initialized` and `Frozen` states are accepted.
///
/// - [`ProgramError::IncorrectProgramId`] if the program is unsupported,
///   non-executable, or differs from the token account's runtime owner.
/// - [`ProgramError::AccountBorrowFailed`] if the token account data cannot be borrowed.
/// - [`ProgramError::InvalidAccountData`] if the layout or state byte is invalid,
///   or the mint or owner authority differs from the expected address.
/// - [`ProgramError::UninitializedAccount`] if the stored state is `Uninitialized`.
pub fn ensure_token_account(
    account: &AccountView,
    token_program: &AccountView,
    authority: Option<&AccountView>,
    mint: Option<&AccountView>,
) -> ProgramResult {
    let check = |initialized: bool, actual_authority: &Address, actual_mint: &Address| {
        if !initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        if authority.is_some_and(|account| account.address() != actual_authority)
            || mint.is_some_and(|account| account.address() != actual_mint)
        {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    };

    match validate_program(account, token_program)? {
        TokenProgram::SplToken => {
            let account = Account::from_account_view(account)?;
            check(
                account.state()? != AccountState::Uninitialized,
                account.owner(),
                account.mint(),
            )
        }
        TokenProgram::Token2022 => {
            let account = Token2022Account::from_account_view(account)?;
            check(
                account.state()? != Token2022AccountState::Uninitialized,
                account.owner(),
                account.mint(),
            )
        }
    }
}
