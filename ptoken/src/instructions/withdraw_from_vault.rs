use bytemuck::{Pod, Zeroable};
use pinocchio::cpi::{Seed, Signer};
use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_constraints::{Accounts, Context, ensure_writable};
use pinocchio_token::instructions::TransferChecked;

use crate::error::{PtokenError, ensure};
use crate::event::emit;
use crate::helper::{
    check_mint, check_token_account, check_vault_state, find_associated_token_address,
};
use crate::state::Vault;

pub const DISCRIMINATOR: u8 = 2;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct WithdrawFromVaultArgs {
    pub amount: u64,
}

impl TryFrom<&[u8]> for WithdrawFromVaultArgs {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let args = bytemuck::try_pod_read_unaligned::<Self>(data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
        ensure!(args.amount != 0, PtokenError::ZeroWithdrawalAmount);
        Ok(args)
    }
}

#[derive(Accounts)]
struct Ctx<'account> {
    #[account(signer)]
    authority: &'account AccountView,
    #[account(mut @ PtokenError::AccountNotWritable)]
    vault: &'account AccountView,
    destination: &'account AccountView,
    mint: &'account AccountView,
    token_program: &'account AccountView,
    #[account(
        seeds = [Vault::SEED, authority.address().as_ref(), mint.address().as_ref()],
        bump @ PtokenError::InvalidVaultStateAddress,
    )]
    vault_state: &'account AccountView,
}

pub fn process(
    program_id: &Address,
    accounts: &[AccountView],
    args: WithdrawFromVaultArgs,
) -> ProgramResult {
    let ctx = Context::<Ctx<'_>>::try_from_accounts(program_id, accounts)?;
    let Ctx {
        authority,
        vault,
        destination,
        mint,
        token_program,
        vault_state,
    } = ctx.accounts;

    let token_program_id = token_program.address();
    let vault_address =
        find_associated_token_address(vault_state.address(), mint.address(), token_program_id)?;
    ensure!(
        vault.address() == &vault_address,
        PtokenError::InvalidTokenVaultAddress
    );
    check_token_account(vault, token_program, vault_state, mint)?;

    ensure_writable(destination).map_err(|_| PtokenError::AccountNotWritable)?;
    ensure!(
        destination.address() != vault.address(),
        PtokenError::WithdrawalDestinationIsVault
    );
    check_token_account(destination, token_program, authority, mint)?;
    let decimals = check_mint(mint, token_program)?;

    ensure!(
        vault_state.owned_by(program_id),
        PtokenError::InvalidVaultStateOwner
    );
    check_vault_state(vault_state, authority, mint, vault, token_program_id)?;

    let bump = [ctx.bumps.vault_state];
    let seeds = [
        Seed::from(Vault::SEED),
        Seed::from(authority.address().as_ref()),
        Seed::from(mint.address().as_ref()),
        Seed::from(&bump),
    ];
    TransferChecked::new(vault, mint, destination, vault_state, args.amount, decimals)
        .invoke_signed_with_program(&[Signer::from(&seeds)], token_program_id)?;

    emit!(VaultWithdrawn {
        caller: *authority.address().as_array(),
        mint: *mint.address().as_array(),
        destination: *destination.address().as_array(),
        amount: args.amount
    });

    Ok(())
}
