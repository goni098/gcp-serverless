use bytemuck::{Pod, Zeroable};
use pinocchio::cpi::{Seed, Signer};
use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_constraints::{Accounts, Context};

use crate::error::{PtokenError, ensure};
use crate::event::emit;
use crate::helper::find_associated_token_address;
use crate::state::Vault;

pub const DISCRIMINATOR: u8 = 0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct InitializeVaultArgs;

impl TryFrom<&[u8]> for InitializeVaultArgs {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        ensure!(data.is_empty(), ProgramError::InvalidInstructionData);
        Ok(Self)
    }
}

#[derive(Accounts)]
struct Ctx<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,

    #[mint(program = token_program)]
    mint: &'acc AccountView,

    #[account(
        mut,
        initialized,
        seeds = [Vault::SEED, payer.address().as_ref(), mint.address().as_ref()],
        bump
    )]
    vault: &'acc AccountView,

    #[account(mut)]
    #[token_account(
        mint = mint,
        authority = vault,
        program = token_program
    )]
    vault_token: &'acc AccountView,

    #[program(id = pinocchio_associated_token_account::ID)]
    associated_token_program: &'acc AccountView,

    #[program(spl)]
    token_program: &'acc AccountView,

    #[program(system)]
    system_program: &'acc AccountView,
}

pub fn process(
    program_id: &Address,
    accounts: &mut [AccountView],
    _args: InitializeVaultArgs,
) -> ProgramResult {
    let ctx = Context::<Ctx<'_>>::try_from_accounts(program_id, accounts)?;
    let Ctx {
        payer,
        mint,
        vault,
        vault_token,
        token_program,
        system_program,
        ..
    } = ctx.accounts;

    ensure!(vault.data_len() == 0, PtokenError::VaultAlreadyInitialized);
    let vault_token_address =
        find_associated_token_address(vault.address(), mint.address(), token_program.address())?;
    ensure!(
        vault_token.address() == &vault_token_address,
        PtokenError::InvalidVaultTokenAddress
    );

    let bump = [ctx.bumps.vault];
    let seeds = [
        Seed::from(Vault::SEED),
        Seed::from(payer.address().as_ref()),
        Seed::from(mint.address().as_ref()),
        Seed::from(&bump),
    ];

    // The derived context borrows shared views. A cloned view shares the runtime account.
    let mut vault = vault.clone();
    pinocchio_system::create_account_with_minimum_balance_signed(
        &mut vault,
        Vault::LEN,
        program_id,
        payer,
        None,
        &[Signer::from(&seeds)],
    )?;

    pinocchio_associated_token_account::instructions::CreateIdempotent {
        funding_account: payer,
        account: vault_token,
        wallet: &vault,
        mint,
        system_program,
        token_program,
    }
    .invoke()?;

    let vault_data = Vault {
        discriminator: Vault::DISCRIMINATOR,
        authority: *payer.address().as_array(),
        mint: *mint.address().as_array(),
        token_vault: *vault_token.address().as_array(),
        token_program: *token_program.address().as_array(),
    };

    vault
        .try_borrow_mut()?
        .copy_from_slice(bytemuck::bytes_of(&vault_data));

    emit!(VaultInitialized {
        caller: vault_data.authority,
        mint: vault_data.mint,
    });

    Ok(())
}
