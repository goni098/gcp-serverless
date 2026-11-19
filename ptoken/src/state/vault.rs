use bytemuck::{Pod, Zeroable};
use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};

use crate::error::{PtokenError, ensure};

/// Program-owned state linking a user and mint to their token vault.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct Vault {
    pub discriminator: [u8; 8],
    pub authority: [u8; 32],
    pub mint: [u8; 32],
    pub token_vault: [u8; 32],
    pub token_program: [u8; 32],
}

impl Vault {
    pub const SEED: &[u8] = b"vault";
    pub const DISCRIMINATOR: [u8; 8] = *b"VAULT\0\0\x01";
    pub const LEN: usize = size_of::<Self>();

    pub(crate) fn new(
        authority: &AccountView,
        mint: &AccountView,
        token_vault: &AccountView,
        token_program: &AccountView,
    ) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            authority: authority.address().to_bytes(),
            mint: mint.address().to_bytes(),
            token_vault: token_vault.address().to_bytes(),
            token_program: token_program.address().to_bytes(),
        }
    }

    pub(crate) fn validate_data(&self, vault: &AccountView) -> ProgramResult {
        ensure!(
            &*vault.try_borrow()? == bytemuck::bytes_of(self),
            PtokenError::InvalidVaultState
        );
        Ok(())
    }

    pub fn derive(authority: &Address, mint: &Address) -> Result<(Address, u8), ProgramError> {
        let seeds = [Self::SEED, authority.as_ref(), mint.as_ref()];

        Address::try_find_program_address(&seeds, &crate::ID.into())
            .ok_or(ProgramError::InvalidSeeds)
    }
}
