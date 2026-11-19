#![no_std]

pub mod error;
pub mod event;
pub mod instructions;
pub mod state;

use instructions::Instruction;
use pinocchio::error::ProgramError;
use pinocchio::{
    AccountView, Address, ProgramResult, no_allocator, nostd_panic_handler, program_entrypoint,
};
use pinocchio_pubkey::declare_id;

use crate::error::ensure;

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

declare_id!("Cj7Vqiuz89t81zfb2aFiykeg3oNd2hcGpY9vRVnKrvsk");

fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    ensure!(
        program_id.to_bytes() == crate::ID,
        ProgramError::IncorrectProgramId
    );

    match Instruction::try_from(instruction_data)? {
        Instruction::InitializeVault(args) => {
            instructions::initialize_vault::process(program_id, accounts, args)
        }
        Instruction::DepositIntoVault(args) => {
            instructions::deposit_into_vault::process(program_id, accounts, args)
        }
        Instruction::WithdrawFromVault(args) => {
            instructions::withdraw_from_vault::process(program_id, accounts, args)
        }
    }
}
