use pinocchio::account::RuntimeAccount;
use pinocchio_constraints::{AccountView, Accounts, Address, Context, ProgramError};

#[derive(Accounts)]
struct DepositAccounts<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,
    #[account(mut)]
    source: &'acc AccountView,
}

#[test]
fn constructs_a_context_only_for_valid_accounts() {
    let mut raw = [
        RuntimeAccount {
            is_signer: 1,
            address: [1; 32].into(),
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            is_writable: 1,
            address: [2; 32].into(),
            ..RuntimeAccount::default()
        },
        RuntimeAccount::default(),
    ];
    // SAFETY: Each header remains live for the test and declares zero data bytes.
    let [payer, source, readonly] = raw
        .each_mut()
        .map(|account| unsafe { AccountView::new_unchecked(account) });
    let program_id = [3; 32].into();

    let accounts = [payer.clone(), source.clone()];
    let ctx = Context::<DepositAccounts<'_>>::try_from_accounts(&program_id, &accounts).unwrap();
    let () = ctx.bumps;
    assert_eq!(ctx.accounts.payer.address(), payer.address());
    assert_eq!(ctx.accounts.source.address(), source.address());
    assert!(ctx.remaining_accounts.is_empty());

    assert_eq!(
        DepositAccounts::try_accounts(&program_id, &[source.clone(), payer.clone()]).err(),
        Some(ProgramError::MissingRequiredSignature),
    );
    assert_eq!(
        DepositAccounts::try_accounts(&program_id, &[payer.clone(), readonly.clone()]).err(),
        Some(ProgramError::Immutable),
    );
    assert_eq!(
        DepositAccounts::try_accounts(&program_id, &accounts[..1]).err(),
        Some(ProgramError::NotEnoughAccountKeys),
    );
    let accounts = [payer, source, readonly, accounts[0].clone()];
    let ctx = Context::<DepositAccounts<'_>>::try_from_accounts(&program_id, &accounts).unwrap();
    assert_eq!(ctx.remaining_accounts, &accounts[2..]);
}

#[test]
fn accepts_caller_defined_errors() {
    struct ReadOnly;

    impl From<ReadOnly> for ProgramError {
        fn from(_: ReadOnly) -> Self {
            Self::Custom(42)
        }
    }

    #[derive(Accounts)]
    struct WriteAccounts<'acc> {
        #[account(mut @ ReadOnly)]
        source: &'acc AccountView,
    }

    let mut raw = [
        RuntimeAccount::default(),
        RuntimeAccount {
            is_writable: 1,
            ..RuntimeAccount::default()
        },
    ];
    // SAFETY: Each header remains live for the test and declares zero data bytes.
    let [readonly, writable] = raw
        .each_mut()
        .map(|account| unsafe { AccountView::new_unchecked(account) });
    let program_id = [3; 32].into();

    assert_eq!(
        WriteAccounts::try_accounts(&program_id, &[readonly]).err(),
        Some(ProgramError::Custom(42)),
    );
    let accounts = [writable];
    let (parsed, (), remaining) = WriteAccounts::try_accounts(&program_id, &accounts).unwrap();
    assert!(parsed.source.is_writable());
    assert!(remaining.is_empty());
}

#[test]
fn validates_account_ownership() {
    #[derive(Accounts)]
    struct OwnedAccount<'acc> {
        #[account(owner = program.address())]
        data: &'acc AccountView,
        program: &'acc AccountView,
    }

    let program_id = Address::new_from_array([3; 32]);
    let mut raw = [
        RuntimeAccount {
            owner: program_id.clone(),
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: program_id.clone(),
            ..RuntimeAccount::default()
        },
        RuntimeAccount::default(),
    ];
    // SAFETY: Each header remains live for the test and declares zero data bytes.
    let [data, program, other] = raw
        .each_mut()
        .map(|account| unsafe { AccountView::new_unchecked(account) });
    let accounts = [data.clone(), program];
    let (parsed, (), _) = OwnedAccount::try_accounts(&program_id, &accounts).unwrap();
    assert!(parsed.data.owned_by(parsed.program.address()));
    assert_eq!(
        OwnedAccount::try_accounts(&program_id, &[data, other]).err(),
        Some(ProgramError::InvalidAccountOwner),
    );
}

#[test]
fn validates_initialization_by_lamport_balance() {
    #[derive(Accounts)]
    struct CreateAccounts<'acc> {
        #[account(initialized)]
        existing: &'acc AccountView,
        #[account(uninitialized)]
        new: &'acc AccountView,
    }

    let mut raw = [
        RuntimeAccount {
            lamports: 1,
            ..RuntimeAccount::default()
        },
        RuntimeAccount::default(),
    ];
    // SAFETY: Both headers remain live and declare zero data bytes.
    let [funded, empty] = raw
        .each_mut()
        .map(|account| unsafe { AccountView::new_unchecked(account) });
    let program_id = Address::new_from_array([3; 32]);
    let accounts = [funded.clone(), empty.clone()];
    let ctx = Context::<CreateAccounts<'_>>::try_from_accounts(&program_id, &accounts).unwrap();
    assert_eq!(ctx.accounts.existing.lamports(), 1);
    assert_eq!(ctx.accounts.new.lamports(), 0);
    assert_eq!(
        CreateAccounts::try_accounts(&program_id, &[empty.clone(), empty]).err(),
        Some(ProgramError::UninitializedAccount),
    );
    assert_eq!(
        CreateAccounts::try_accounts(&program_id, &[funded.clone(), funded]).err(),
        Some(ProgramError::AccountAlreadyInitialized),
    );
}

#[derive(Accounts)]
pub struct VaultAccounts<'acc> {
    #[account(seeds = [b"vault", payer.address().as_ref()], bump)]
    vault: &'acc AccountView,

    #[account(
        seeds = &[b"reserve"],
        bump = payer.address().as_array()[0] @ ProgramError::Custom(43),
    )]
    reserve: &'acc AccountView,

    #[account(signer)]
    payer: &'acc AccountView,
}

#[test]
fn context_exposes_canonical_and_explicit_bumps() {
    let program_id = [3; 32].into();
    let (canonical_reserve, canonical_bump) =
        Address::try_find_program_address(&[b"reserve"], &program_id).unwrap();
    let (reserve, reserve_bump) = (0..canonical_bump)
        .rev()
        .find_map(|bump| {
            Address::create_program_address(&[b"reserve", &[bump]], &program_id)
                .ok()
                .map(|address| (address, bump))
        })
        .unwrap();
    let payer: Address = [reserve_bump; 32].into();
    let (vault, vault_bump) =
        Address::try_find_program_address(&[b"vault", payer.as_ref()], &program_id).unwrap();

    let mut raw = [
        RuntimeAccount {
            address: vault,
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: reserve,
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: payer,
            is_signer: 1,
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: canonical_reserve,
            ..RuntimeAccount::default()
        },
    ];
    // SAFETY: Each header remains live for the test and declares zero data bytes.
    let [vault, reserve, payer, canonical_reserve] = raw
        .each_mut()
        .map(|account| unsafe { AccountView::new_unchecked(account) });

    let accounts = [vault.clone(), reserve.clone(), payer.clone()];
    let ctx = Context::<VaultAccounts<'_>>::try_from_accounts(&program_id, &accounts).unwrap();
    assert_eq!(ctx.accounts.vault.address(), vault.address());
    assert_eq!(ctx.accounts.reserve.address(), reserve.address());
    assert_eq!(ctx.accounts.payer.address(), payer.address());
    assert_eq!(ctx.bumps.vault, vault_bump);
    assert_eq!(ctx.bumps.reserve, reserve_bump);
    assert_eq!(
        VaultAccounts::try_accounts(&[4; 32].into(), &accounts).err(),
        Some(ProgramError::InvalidSeeds),
    );
    assert_eq!(
        VaultAccounts::try_accounts(&program_id, &[vault, canonical_reserve, payer]).err(),
        Some(ProgramError::Custom(43)),
    );
}

#[test]
fn validates_program_addresses_and_executable_flags() {
    const TARGET_ID: Address = Address::new_from_array([7; 32]);

    #[derive(Accounts)]
    struct Programs<'acc> {
        #[program(system)]
        system_program: &'acc AccountView,
        #[program(id = TARGET_ID)]
        target_program: &'acc AccountView,
    }

    #[derive(Accounts)]
    struct CustomProgram<'acc> {
        #[program(id = [7; 32] @ ProgramError::Custom(44))]
        target_program: &'acc AccountView,
    }

    let mut raw = [
        RuntimeAccount {
            executable: 1,
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: TARGET_ID,
            executable: 1,
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: TARGET_ID,
            ..RuntimeAccount::default()
        },
        RuntimeAccount {
            address: [8; 32].into(),
            executable: 1,
            ..RuntimeAccount::default()
        },
    ];
    // SAFETY: Each header remains live for the test and declares zero data bytes.
    let [system, target, non_executable, wrong_id] = raw
        .each_mut()
        .map(|account| unsafe { AccountView::new_unchecked(account) });
    let program_id = [3; 32].into();
    let accounts = [system.clone(), target.clone()];
    let ctx = Context::<Programs<'_>>::try_from_accounts(&program_id, &accounts).unwrap();
    assert_eq!(ctx.accounts.system_program.address(), &Address::default());
    assert_eq!(ctx.accounts.target_program.address(), &TARGET_ID);
    assert_eq!(
        Programs::try_accounts(&program_id, &[system.clone(), non_executable.clone()]).err(),
        Some(ProgramError::IncorrectProgramId),
    );
    assert_eq!(
        Programs::try_accounts(&program_id, &[system, wrong_id.clone()]).err(),
        Some(ProgramError::IncorrectProgramId),
    );
    assert_eq!(
        Programs::try_accounts(&program_id, &[wrong_id, target.clone()]).err(),
        Some(ProgramError::IncorrectProgramId),
    );

    let accounts = [target];
    let (parsed, (), _) = CustomProgram::try_accounts(&program_id, &accounts).unwrap();
    assert_eq!(parsed.target_program.address(), &TARGET_ID);
    assert_eq!(
        CustomProgram::try_accounts(&program_id, &[non_executable]).err(),
        Some(ProgramError::Custom(44)),
    );
}
