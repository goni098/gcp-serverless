use bytemuck::{Pod, Zeroable};
use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_constraints::{Accounts, Context};

use crate::error::{PtokenError, ensure};
use crate::event::emit;
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
struct DepositIntoVault<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,

    #[mint(program = token_program)]
    mint: &'acc AccountView,

    #[account(mut)]
    #[token_account(
        program=token_program,
        mint=mint,
        authority=payer
    )]
    source_token: &'acc AccountView,

    #[account(
        initialized,
        seeds = [Vault::SEED, payer.address().as_ref(), mint.address().as_ref()],
        bump
    )]
    _vault: &'acc AccountView,

    #[account(mut)]
    #[associated_token(
        program = token_program,
        mint = mint,
        authority = vault,
    )]
    vault_token: &'acc AccountView,

    #[program(spl)]
    token_program: &'acc AccountView,
}

pub fn process(
    program_id: &Address,
    accounts: &[AccountView],
    args: DepositIntoVaultArgs,
) -> ProgramResult {
    let ctx = Context::<DepositIntoVault<'_>>::try_from_accounts(program_id, accounts)?;

    pinocchio_token::instructions::Transfer::new(
        ctx.accounts.source_token,
        ctx.accounts.vault_token,
        ctx.accounts.payer,
        args.amount,
    )
    .invoke_with_program(ctx.accounts.token_program.address())?;

    emit!(VaultDeposited {
        caller: ctx.accounts.payer.address().to_bytes(),
        mint: ctx.accounts.mint.address().to_bytes(),
        amount: args.amount
    });

    Ok(())
}
