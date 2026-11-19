//! Bind and validate instruction accounts in Pinocchio programs.
//!
//! Pair `seeds` with `bump` to require the canonical PDA, or with `bump = value`
//! to validate a specific bump. Both forms expose the validated bump on the context
//! under the account field's name.
//!
//! Use `#[program(system)]` for the System Program or `#[program(id = EXPECTED_ID)]`
//! for a specific executable program. An incorrect address or a clear executable
//! flag returns [`ProgramError::IncorrectProgramId`].
//!
//! `#[account(initialized)]` requires nonzero lamports and `#[account(uninitialized)]`
//! requires zero lamports. Both flags check only the account balance.
//!
//! Enable the optional `spl` feature for `#[program(spl)]`, mint constraints,
//! and token account constraints, including `#[associated_token(...)]`.
//!
//! Use `#[instruction(args: ArgsType)]` to reference decoded instruction arguments in
//! constraints. Pass the arguments by reference to [`Context::try_from_accounts_with_args`].
//! Pass a mutable account slice to [`Context`] when the accounts struct has
//! `&mut AccountView` fields.
//!
//! ```
//! use pinocchio_constraints::{AccountView, Accounts, Address, Context, ProgramError};
//!
//! #[derive(Accounts)]
//! struct DepositAccounts<'acc> {
//!     #[account(signer)]
//!     payer: &'acc AccountView,
//!
//!     #[account(mut)]
//!     source: &'acc AccountView,
//!
//!     #[account(seeds = [b"vault", payer.address().as_ref()], bump)]
//!     vault: &'acc AccountView,
//!
//!     #[program(system)]
//!     system_program: &'acc AccountView,
//! }
//!
//! fn process(program_id: &Address, accounts: &[AccountView]) -> Result<(), ProgramError> {
//!     let ctx = Context::<DepositAccounts<'_>>::try_from_accounts(program_id, accounts)?;
//!     let vault = ctx.accounts.vault;
//!     let vault_bump = ctx.bumps.vault;
//!     let extra_accounts = ctx.remaining_accounts;
//!     Ok(())
//! }
//! ```

#![no_std]

#[cfg(feature = "spl")]
pub mod spl;

use pinocchio::ProgramResult;
pub use pinocchio::{AccountView, Address, error::ProgramError};
pub use pinocchio_constraints_derive::Accounts;

/// Binds and validates the accounts required by an instruction.
pub trait Accounts<'acc>: Sized {
    /// Contains a validated bump for each PDA field. The derive uses `()` when there are none.
    type Bumps;

    /// Bind and validate the instruction's accounts and return their PDA bumps and the remaining slice.
    ///
    /// The derive consumes one account per field in declaration order.
    fn try_accounts(
        program_id: &Address,
        accounts: &'acc [AccountView],
    ) -> Result<(Self, Self::Bumps, &'acc [AccountView]), ProgramError>;
}

/// Binds and validates accounts using decoded instruction arguments.
pub trait AccountsWithArgs<'acc>: Sized {
    /// The input and remaining account slices. The derive uses a mutable slice when a field
    /// requires a mutable view, and a shared slice otherwise.
    type AccountSlice;
    /// The decoded argument type declared by `#[instruction(...)]`.
    type Args: ?Sized;
    /// Contains a validated bump for each PDA field. The derive uses `()` when there are none.
    type Bumps;

    /// Bind and validate the instruction's accounts using the supplied arguments.
    fn try_accounts_with_args(
        program_id: &Address,
        accounts: Self::AccountSlice,
        args: &Self::Args,
    ) -> Result<(Self, Self::Bumps, Self::AccountSlice), ProgramError>;
}

impl<'acc, T: Accounts<'acc>> AccountsWithArgs<'acc> for T {
    type AccountSlice = &'acc [AccountView];
    type Args = ();
    type Bumps = T::Bumps;

    fn try_accounts_with_args(
        program_id: &Address,
        accounts: &'acc [AccountView],
        _args: &(),
    ) -> Result<(Self, Self::Bumps, &'acc [AccountView]), ProgramError> {
        T::try_accounts(program_id, accounts)
    }
}

/// Marks account types whose validation can run without an instruction argument binding.
pub trait AccountsWithoutArgs<'acc>: AccountsWithArgs<'acc, Args = ()> {}

impl<'acc, T: Accounts<'acc>> AccountsWithoutArgs<'acc> for T {}

/// The validated accounts, their PDA bumps, and the remaining accounts for an instruction.
pub struct Context<'acc, T: AccountsWithArgs<'acc>> {
    pub accounts: T,
    pub bumps: T::Bumps,
    /// Accounts supplied after the declared fields. Validate these before use.
    pub remaining_accounts: T::AccountSlice,
}

impl<'acc, T: AccountsWithoutArgs<'acc>> Context<'acc, T> {
    /// Construct a context after validating the supplied accounts.
    pub fn try_from_accounts(
        program_id: &Address,
        accounts: T::AccountSlice,
    ) -> Result<Self, ProgramError> {
        Self::try_from_accounts_with_args(program_id, accounts, &())
    }
}

impl<'acc, T: AccountsWithArgs<'acc>> Context<'acc, T> {
    /// Construct a context after validating the supplied accounts with decoded arguments.
    ///
    /// A derive with `#[instruction(...)]` requires this constructor.
    ///
    /// ```compile_fail
    /// use pinocchio_constraints::{AccountView, Accounts, Address, Context};
    ///
    /// #[derive(Accounts)]
    /// #[instruction(vault_id: u64)]
    /// struct OpenVault<'acc> {
    ///     #[account(seeds = [b"vault", &vault_id.to_le_bytes()], bump)]
    ///     vault: &'acc AccountView,
    /// }
    ///
    /// let _ = Context::<OpenVault<'_>>::try_from_accounts(&Address::default(), &[]);
    /// ```
    pub fn try_from_accounts_with_args(
        program_id: &Address,
        accounts: T::AccountSlice,
        args: &T::Args,
    ) -> Result<Self, ProgramError> {
        let (accounts, bumps, remaining_accounts) =
            T::try_accounts_with_args(program_id, accounts, args)?;
        Ok(Self {
            accounts,
            bumps,
            remaining_accounts,
        })
    }
}

/// Require that the account signed the transaction.
/// Returns [`ProgramError::MissingRequiredSignature`] when the signer flag is clear.
pub fn ensure_signer(account: &AccountView) -> ProgramResult {
    if account.is_signer() {
        Ok(())
    } else {
        Err(ProgramError::MissingRequiredSignature)
    }
}

/// Require the Solana writable flag on an account.
/// Returns [`ProgramError::Immutable`] when the writable flag is clear.
pub fn ensure_writable(account: &AccountView) -> ProgramResult {
    if account.is_writable() {
        Ok(())
    } else {
        Err(ProgramError::Immutable)
    }
}

/// Require a nonzero lamport balance.
/// Returns [`ProgramError::UninitializedAccount`] when the balance is zero.
pub fn ensure_initialized(account: &AccountView) -> ProgramResult {
    if account.lamports() != 0 {
        Ok(())
    } else {
        Err(ProgramError::UninitializedAccount)
    }
}

/// Require a zero lamport balance.
/// Returns [`ProgramError::AccountAlreadyInitialized`] when the balance is nonzero.
pub fn ensure_uninitialized(account: &AccountView) -> ProgramResult {
    if account.lamports() == 0 {
        Ok(())
    } else {
        Err(ProgramError::AccountAlreadyInitialized)
    }
}

/// Require the expected account owner. Returns `InvalidAccountOwner` on a mismatch.
pub fn ensure_owner(account: &AccountView, expected_owner: impl AsRef<[u8]>) -> ProgramResult {
    if account.owner().as_array() == expected_owner.as_ref() {
        Ok(())
    } else {
        Err(ProgramError::InvalidAccountOwner)
    }
}

/// Require an executable account at the expected program ID.
///
/// Returns `IncorrectProgramId` for an address mismatch or a non-executable account.
pub fn ensure_program(account: &AccountView, expected_id: impl AsRef<[u8]>) -> ProgramResult {
    if account.address().as_array() == expected_id.as_ref() && account.executable() {
        Ok(())
    } else {
        Err(ProgramError::IncorrectProgramId)
    }
}

/// Require the executable System Program account.
/// Returns [`ProgramError::IncorrectProgramId`] for an address mismatch or a non-executable account.
pub fn ensure_system_program(account: &AccountView) -> ProgramResult {
    ensure_program(account, [0; 32])
}

/// Check a PDA under the given program ID and return its validated bump.
///
/// Pass `None` to require the canonical bump or `Some(bump)` to validate a specific bump.
/// The caller supplies only the base seeds.
/// Returns [`ProgramError::InvalidSeeds`] for invalid seeds, a failed derivation, or an address mismatch.
pub fn ensure_pda(
    account: &AccountView,
    program_id: &Address,
    seeds: &[&[u8]],
    bump: Option<u8>,
) -> Result<u8, ProgramError> {
    use pinocchio::address::{MAX_SEED_LEN, MAX_SEEDS};

    if seeds.len() >= MAX_SEEDS || seeds.iter().any(|seed| seed.len() > MAX_SEED_LEN) {
        return Err(ProgramError::InvalidSeeds);
    }

    let (address, bump) = match bump {
        None => Address::try_find_program_address(seeds, program_id)
            .ok_or(ProgramError::InvalidSeeds)?,
        Some(bump) => {
            let mut with_bump: [&[u8]; MAX_SEEDS] = [&[]; MAX_SEEDS];
            with_bump[..seeds.len()].copy_from_slice(seeds);
            with_bump[seeds.len()] = core::slice::from_ref(&bump);
            let address = Address::create_program_address(&with_bump[..=seeds.len()], program_id)
                .map_err(|_| ProgramError::InvalidSeeds)?;
            (address, bump)
        }
    };

    if account.address() == &address {
        Ok(bump)
    } else {
        Err(ProgramError::InvalidSeeds)
    }
}
