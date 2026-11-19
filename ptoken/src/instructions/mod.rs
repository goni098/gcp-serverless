//! Instructions supported by the program.

use deposit_into_vault::DepositIntoVaultArgs;
use initialize_vault::InitializeVaultArgs;
use pinocchio::error::ProgramError;
use withdraw_from_vault::WithdrawFromVaultArgs;

use crate::error::bail;

pub mod deposit_into_vault;
pub mod initialize_vault;
pub mod withdraw_from_vault;

/// An instruction decoded from the transaction data.
#[derive(Debug, PartialEq, Eq)]
pub enum Instruction {
    InitializeVault(InitializeVaultArgs),
    DepositIntoVault(DepositIntoVaultArgs),
    WithdrawFromVault(WithdrawFromVaultArgs),
}

impl TryFrom<&[u8]> for Instruction {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let Some((&discriminator, args)) = data.split_first() else {
            bail!(ProgramError::InvalidInstructionData);
        };

        match discriminator {
            initialize_vault::DISCRIMINATOR => Ok(Self::InitializeVault(args.try_into()?)),
            deposit_into_vault::DISCRIMINATOR => Ok(Self::DepositIntoVault(args.try_into()?)),
            withdraw_from_vault::DISCRIMINATOR => Ok(Self::WithdrawFromVault(args.try_into()?)),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
