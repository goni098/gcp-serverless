use bytemuck::{Pod, Zeroable};
use pinocchio::cpi::{Seed, Signer};
use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_constraints::{Accounts, Context};

use crate::error::{PtokenError, ensure};
use crate::event::emit;
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
struct WithdrawFromVault<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,

    #[mint(program=token_program)]
    mint: &'acc AccountView,

    #[account(
        owner = crate::ID @ PtokenError::InvalidVaultStateOwner,
        seeds = [Vault::SEED, payer.address().as_ref(), mint.address().as_ref()],
        bump
    )]
    vault: &'acc AccountView,

    #[account(mut)]
    #[associated_token(
        program = token_program,
        mint = mint,
        authority = vault,
    )]
    vault_token: &'acc AccountView,

    #[account(mut)]
    destination: &'acc AccountView,

    #[program(spl)]
    token_program: &'acc AccountView,
}

pub fn process(
    program_id: &Address,
    accounts: &[AccountView],
    args: WithdrawFromVaultArgs,
) -> ProgramResult {
    let ctx = Context::<WithdrawFromVault<'_>>::try_from_accounts(program_id, accounts)?;

    let WithdrawFromVault {
        payer,
        mint,
        vault,
        vault_token,
        destination,
        token_program,
    } = ctx.accounts;

    let bump = [ctx.bumps.vault];
    let seeds = [
        Seed::from(Vault::SEED),
        Seed::from(payer.address().as_ref()),
        Seed::from(mint.address().as_ref()),
        Seed::from(&bump),
    ];

    pinocchio_token::instructions::Transfer::new(vault_token, destination, vault, args.amount)
        .invoke_signed_with_program(&[Signer::from(&seeds)], token_program.address())?;

    emit!(VaultWithdrawn {
        caller: payer.address().to_bytes(),
        mint: mint.address().to_bytes(),
        destination: destination.address().to_bytes(),
        amount: args.amount
    });

    Ok(())
}
