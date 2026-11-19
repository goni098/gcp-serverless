use pinocchio::error::ProgramError;

/// Stable custom error codes returned by the program.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtokenError {
    /// The supplied system program address is incorrect.
    InvalidSystemProgram = 6000,
    /// The supplied token program is neither SPL Token nor Token-2022.
    InvalidTokenProgram = 6001,
    /// An account that the instruction must update is read-only.
    AccountNotWritable = 6002,
    /// The mint belongs to a different token program.
    InvalidMintOwner = 6003,
    /// The token vault address differs from the expected address.
    InvalidTokenVaultAddress = 6004,
    /// The vault state account is at the wrong PDA.
    InvalidVaultStateAddress = 6005,
    /// The vault state account has already been initialized.
    VaultAlreadyInitialized = 6006,

    VaultTokenAlreadyInitialized = 60061,

    /// The vault state account has the wrong program owner.
    InvalidVaultStateOwner = 6007,
    /// The vault state's layout, discriminator, or stored keys are invalid.
    InvalidVaultState = 6008,
    /// The token account is incompatible with the mint, owner, or instruction.
    InvalidTokenAccount = 6009,
    /// Token-2022 did not return a valid token account size.
    InvalidTokenAccountSize = 6010,

    InvalidVaultTokenAddress = 6015,

    /// The deposit amount is zero.
    ZeroDepositAmount = 6011,
    /// The deposit source is the same account as the token vault.
    DepositSourceIsVault = 6012,
    /// The withdrawal amount is zero.
    ZeroWithdrawalAmount = 6013,
    /// The withdrawal destination is the same account as the token vault.
    WithdrawalDestinationIsVault = 6014,
    /// New vaults support only Token-2022 mints with the base mint layout.
    UnsupportedToken2022Mint = 6016,
}

impl From<PtokenError> for ProgramError {
    fn from(error: PtokenError) -> Self {
        Self::Custom(error as u32)
    }
}

/// Return early with an error converted to the current function's error type.
macro_rules! bail {
    ($error:expr $(,)?) => {
        return ::core::result::Result::Err(::core::convert::Into::into($error))
    };
}

/// Return the supplied error if the condition is false.
macro_rules! ensure {
    ($condition:expr, $error:expr $(,)?) => {
        if !$condition {
            $crate::error::bail!($error);
        }
    };
}

pub(crate) use bail;
pub(crate) use ensure;
