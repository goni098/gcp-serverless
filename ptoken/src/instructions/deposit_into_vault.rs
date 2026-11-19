use bytemuck::{Pod, Zeroable};
use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_constraints::{Accounts, Context, ensure_pda};

use crate::error::{PtokenError, ensure};
use crate::event::emit;
use crate::helper::{
    check_mint, check_token_account, check_vault_state, find_associated_token_address,
};
use crate::state::Vault;

pub const DISCRIMINATOR: u8 = 1;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct DepositIntoVaultArgs {
    pub amount: u64,
}

impl TryFrom<&[u8]> for DepositIntoVaultArgs {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let args = bytemuck::try_pod_read_unaligned::<Self>(data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        ensure!(args.amount != 0, PtokenError::ZeroDepositAmount);

        Ok(args)
    }
}

#[derive(Accounts)]
struct Ctx<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,
    mint: &'acc AccountView,
    source_token: &'acc AccountView,
    vault: &'acc AccountView,
    vault_token: &'acc AccountView,
    token_program: &'acc AccountView,
}

pub fn process(
    program_id: &Address,
    accounts: &[AccountView],
    args: DepositIntoVaultArgs,
) -> ProgramResult {
    let Ctx {
        payer,
        source_token,
        vault,
        vault_token,
        mint,
        token_program,
    } = Context::<Ctx<'_>>::try_from_accounts(program_id, accounts)?.accounts;

    let token_program_id = token_program.address();
    let vault_token_address =
        find_associated_token_address(vault.address(), mint.address(), token_program_id)?;
    check_mint(mint, token_program)?;
    pinocchio_constraints::ensure_writable(source_token)
        .map_err(|_| PtokenError::AccountNotWritable)?;
    check_token_account(source_token, token_program, payer, mint)?;

    pinocchio_constraints::ensure_writable(vault_token)
        .map_err(|_| PtokenError::AccountNotWritable)?;
    ensure!(
        vault_token.address() == &vault_token_address,
        PtokenError::InvalidTokenVaultAddress
    );
    check_token_account(vault_token, token_program, vault, mint)?;

    ensure!(vault.owned_by(program_id), PtokenError::InvalidVaultStateOwner);
    ensure_pda(
        vault,
        program_id,
        &[Vault::SEED, payer.address().as_ref(), mint.address().as_ref()],
        None,
    )
    .map_err(|_| PtokenError::InvalidVaultStateAddress)?;
    check_vault_state(vault, payer, mint, vault_token, token_program_id)?;

    pinocchio_token::instructions::Transfer::new(source_token, vault_token, payer, args.amount)
        .invoke_with_program(token_program_id)?;

    emit!(VaultDeposited {
        caller: payer.address().to_bytes(),
        mint: mint.address().to_bytes(),
        amount: args.amount
    });

    Ok(())
}
