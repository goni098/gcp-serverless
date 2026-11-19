#![cfg(feature = "spl")]

use pinocchio::account::{NOT_BORROWED, RuntimeAccount};
use pinocchio_constraints::spl::{ensure_mint, ensure_token_account};
use pinocchio_constraints::{AccountView, Accounts, Address, Context, ProgramError};

#[test]
fn validates_spl_program_accounts() {
    #[derive(Accounts)]
    struct TokenProgramAccount<'acc> {
        #[program(spl)]
        token_program: &'acc AccountView,
    }

    #[derive(Accounts)]
    struct CustomErrorAccount<'acc> {
        #[program(spl @ ProgramError::Custom(46))]
        _token_program: &'acc AccountView,
    }

    let program_id = Address::new_from_array([9; 32]);
    for (address, executable, expected) in [
        (pinocchio_token::ID, 1, Ok(())),
        (pinocchio_token_2022::ID, 1, Ok(())),
        (
            pinocchio_token::ID,
            0,
            Err(ProgramError::IncorrectProgramId),
        ),
        (program_id.clone(), 1, Err(ProgramError::IncorrectProgramId)),
    ] {
        let mut header = RuntimeAccount {
            address: address.clone(),
            executable,
            ..RuntimeAccount::default()
        };
        // SAFETY: The header remains live and declares zero data bytes.
        let accounts = [unsafe { AccountView::new_unchecked(&raw mut header) }];
        let result = TokenProgramAccount::try_accounts(&program_id, &accounts)
            .map(|(parsed, (), _)| assert_eq!(parsed.token_program.address(), &address));
        assert_eq!(result, expected);
        assert_eq!(
            CustomErrorAccount::try_accounts(&program_id, &accounts).map(|_| ()),
            expected.map_err(|_| ProgramError::Custom(46)),
        );
    }
}

#[derive(Accounts)]
struct TransferAccounts<'acc> {
    #[account(signer)]
    authority: &'acc AccountView,
    #[mint(program = token_program)]
    mint: &'acc AccountView,
    #[account(mut)]
    #[token_account(
        mint = mint,
        program = token_program,
        authority = authority @ ProgramError::Custom(45),
    )]
    source: &'acc AccountView,
    token_program: &'acc AccountView,
}

#[repr(C)]
struct AccountWithData<const LEN: usize> {
    header: RuntimeAccount,
    data: [u8; LEN],
}

#[test]
fn checks_associated_token_accounts_for_both_programs() {
    #[derive(Accounts)]
    struct AssociatedAccounts<'acc> {
        #[associated_token(program = token_program, mint = mint, authority = wallet)]
        ata: &'acc AccountView,
        mint: &'acc AccountView,
        wallet: &'acc AccountView,
        token_program: &'acc AccountView,
    }

    const MINT: Address = Address::new_from_array([1; 32]);
    const WALLET: Address = Address::new_from_array([2; 32]);
    const TOKEN_LEN: usize = pinocchio_token::state::Account::LEN;
    let program_id = Address::new_from_array([9; 32]);
    for token_program in [pinocchio_token::ID, pinocchio_token_2022::ID] {
        let (address, _) = Address::try_find_program_address(
            &[WALLET.as_ref(), token_program.as_ref(), MINT.as_ref()],
            &pinocchio_associated_token_account::ID,
        )
        .unwrap();
        let mut data = [0; TOKEN_LEN];
        data[..32].copy_from_slice(MINT.as_ref());
        data[32..64].copy_from_slice(WALLET.as_ref());
        data[108] = pinocchio_token::state::AccountState::Initialized.into();
        let header = RuntimeAccount {
            address: address.clone(),
            owner: token_program.clone(),
            borrow_state: NOT_BORROWED,
            data_len: TOKEN_LEN as u64,
            ..RuntimeAccount::default()
        };
        let mut canonical = AccountWithData {
            header: header.clone(),
            data,
        };
        let mut noncanonical = AccountWithData {
            header: RuntimeAccount {
                address: [3; 32].into(),
                ..header
            },
            data,
        };
        let mut references = [
            RuntimeAccount {
                address: MINT,
                ..RuntimeAccount::default()
            },
            RuntimeAccount {
                address: WALLET,
                ..RuntimeAccount::default()
            },
            RuntimeAccount {
                address: token_program,
                executable: 1,
                ..RuntimeAccount::default()
            },
        ];
        // SAFETY: All headers remain live and are followed by their declared data.
        let [mint, wallet, program] = references
            .each_mut()
            .map(|header| unsafe { AccountView::new_unchecked(header) });
        // SAFETY: The account headers remain live and are followed by their declared data.
        let (mut canonical, noncanonical) = unsafe {
            (
                AccountView::new_unchecked((&raw mut canonical).cast()),
                AccountView::new_unchecked((&raw mut noncanonical).cast()),
            )
        };
        let accounts = [
            canonical.clone(),
            mint.clone(),
            wallet.clone(),
            program.clone(),
        ];
        let (parsed, (), _) = AssociatedAccounts::try_accounts(&program_id, &accounts).unwrap();
        assert_eq!(parsed.ata.address(), &address);
        assert_eq!(parsed.mint.address(), &MINT);
        assert_eq!(parsed.wallet.address(), &WALLET);
        assert!(parsed.ata.owned_by(parsed.token_program.address()));
        assert_eq!(
            AssociatedAccounts::try_accounts(&program_id, &[noncanonical, mint, wallet, program])
                .err(),
            Some(ProgramError::InvalidSeeds),
        );
        canonical.try_borrow_mut().unwrap()[0] = 7;
        assert_eq!(
            AssociatedAccounts::try_accounts(&program_id, &accounts).err(),
            Some(ProgramError::InvalidAccountData)
        );
        canonical.try_borrow_mut().unwrap()[0] = 1;
        canonical.try_borrow_mut().unwrap()[32] = 7;
        assert_eq!(
            AssociatedAccounts::try_accounts(&program_id, &accounts).err(),
            Some(ProgramError::InvalidAccountData)
        );
    }
}

#[test]
fn allows_uninitialized_spl_accounts_when_requested() {
    #[derive(Accounts)]
    struct InitializeAccounts<'acc> {
        #[mint(program = token_program, allow_uninitialized, decimals = 0)]
        mint: &'acc AccountView,
        #[token_account(program = token_program, allow_uninitialized, mint = mint)]
        token: &'acc AccountView,
        token_program: &'acc AccountView,
    }

    const MINT_LEN: usize = pinocchio_token::state::Mint::LEN;
    const TOKEN_LEN: usize = pinocchio_token::state::Account::LEN;
    const MINT_ID: Address = Address::new_from_array([1; 32]);
    let program_id = Address::new_from_array([9; 32]);
    for id in [pinocchio_token::ID, pinocchio_token_2022::ID] {
        let header = RuntimeAccount {
            owner: id.clone(),
            borrow_state: NOT_BORROWED,
            ..RuntimeAccount::default()
        };
        let mut raw_mint = AccountWithData {
            header: RuntimeAccount {
                address: MINT_ID,
                data_len: MINT_LEN as u64,
                ..header.clone()
            },
            data: [0; MINT_LEN],
        };
        let mut raw_token = AccountWithData {
            header: RuntimeAccount {
                address: [2; 32].into(),
                data_len: TOKEN_LEN as u64,
                ..header
            },
            data: [0; TOKEN_LEN],
        };
        raw_token.data[..32].copy_from_slice(MINT_ID.as_ref());
        let mut raw_program = RuntimeAccount {
            address: id,
            executable: 1,
            ..RuntimeAccount::default()
        };
        // SAFETY: All headers remain live and are followed by their declared data.
        let (mut mint, mut token, program) = unsafe {
            (
                AccountView::new_unchecked((&raw mut raw_mint).cast()),
                AccountView::new_unchecked((&raw mut raw_token).cast()),
                AccountView::new_unchecked(&raw mut raw_program),
            )
        };
        assert_eq!(
            ensure_mint(&mint, &program, None),
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            ensure_token_account(&token, &program, None, None),
            Err(ProgramError::UninitializedAccount)
        );

        let accounts = [mint.clone(), token.clone(), program.clone()];
        let (parsed, (), _) = InitializeAccounts::try_accounts(&program_id, &accounts).unwrap();
        assert!(parsed.mint.owned_by(parsed.token_program.address()));
        assert!(parsed.token.owned_by(parsed.token_program.address()));

        mint.try_borrow_mut().unwrap()[44] = 1;
        assert_eq!(
            InitializeAccounts::try_accounts(&program_id, &accounts).err(),
            Some(ProgramError::InvalidAccountData)
        );
        mint.try_borrow_mut().unwrap()[44] = 0;
        token.try_borrow_mut().unwrap()[108] = 3;
        assert_eq!(
            InitializeAccounts::try_accounts(&program_id, &accounts).err(),
            Some(ProgramError::InvalidAccountData)
        );
        assert_eq!(
            InitializeAccounts::try_accounts(&program_id, &[token.clone(), mint, program]).err(),
            Some(ProgramError::InvalidAccountData)
        );
    }
}

#[test]
fn checks_spl_token_and_token_2022() {
    #[derive(Accounts)]
    struct MintAuthorities<'acc> {
        #[mint(
            program = token_program,
            authority = authority,
            freeze_authority = freeze_authority,
            decimals = 6,
        )]
        mint: &'acc AccountView,
        authority: &'acc AccountView,
        freeze_authority: &'acc AccountView,
        token_program: &'acc AccountView,
    }

    const MINT_ID: Address = Address::new_from_array([1; 32]);
    const AUTHORITY_ID: Address = Address::new_from_array([2; 32]);
    const FREEZE_AUTHORITY_ID: Address = Address::new_from_array([4; 32]);
    const MINT_LEN: usize = pinocchio_token::state::Mint::LEN;
    const TOKEN_LEN: usize = pinocchio_token::state::Account::LEN;

    let program_id = [9; 32].into();
    for token_program_id in [pinocchio_token::ID, pinocchio_token_2022::ID] {
        let mut mint_data = [0; MINT_LEN];
        // The base mint layout stores decimals and the initialized flag at bytes 44 and 45.
        mint_data[44] = 6;
        mint_data[45] = 1;
        // Each authority is encoded as a four-byte option tag followed by an address.
        mint_data[0] = 1;
        mint_data[4..36].copy_from_slice(AUTHORITY_ID.as_ref());
        mint_data[46] = 1;
        mint_data[50..82].copy_from_slice(FREEZE_AUTHORITY_ID.as_ref());
        let mut source_data = [0; TOKEN_LEN];
        // Both token programs use the same mint, owner, and state fields in the base layout.
        source_data[..32].copy_from_slice(MINT_ID.as_ref());
        source_data[32..64].copy_from_slice(AUTHORITY_ID.as_ref());
        source_data[108] = pinocchio_token::state::AccountState::Initialized.into();

        let mut mint = AccountWithData {
            header: RuntimeAccount {
                address: MINT_ID,
                owner: token_program_id.clone(),
                borrow_state: NOT_BORROWED,
                data_len: MINT_LEN as u64,
                ..RuntimeAccount::default()
            },
            data: mint_data,
        };
        let mut source = AccountWithData {
            header: RuntimeAccount {
                owner: token_program_id.clone(),
                is_writable: 1,
                borrow_state: NOT_BORROWED,
                data_len: TOKEN_LEN as u64,
                ..RuntimeAccount::default()
            },
            data: source_data,
        };
        let mut token_program = RuntimeAccount {
            address: token_program_id.clone(),
            executable: 1,
            ..RuntimeAccount::default()
        };
        let mut non_executable_program = RuntimeAccount {
            address: token_program_id,
            ..RuntimeAccount::default()
        };
        let mut authority = RuntimeAccount {
            address: AUTHORITY_ID,
            is_signer: 1,
            ..RuntimeAccount::default()
        };
        let mut wrong_authority = RuntimeAccount {
            address: [3; 32].into(),
            is_signer: 1,
            ..RuntimeAccount::default()
        };
        let mut freeze_authority = RuntimeAccount {
            address: FREEZE_AUTHORITY_ID,
            ..RuntimeAccount::default()
        };

        // SAFETY: All headers remain live and are followed by their declared data.
        let (mint, source, token_program, non_executable_program, authority, wrong_authority) = unsafe {
            (
                AccountView::new_unchecked((&raw mut mint).cast()),
                AccountView::new_unchecked((&raw mut source).cast()),
                AccountView::new_unchecked(&raw mut token_program),
                AccountView::new_unchecked(&raw mut non_executable_program),
                AccountView::new_unchecked(&raw mut authority),
                AccountView::new_unchecked(&raw mut wrong_authority),
            )
        };
        // SAFETY: The freeze authority header remains live and declares zero data bytes.
        let freeze_authority = unsafe { AccountView::new_unchecked(&raw mut freeze_authority) };
        let mint_accounts = [
            mint.clone(),
            authority.clone(),
            freeze_authority.clone(),
            token_program.clone(),
        ];
        let (parsed, (), _) = MintAuthorities::try_accounts(&program_id, &mint_accounts).unwrap();
        assert_eq!(parsed.mint.address(), &MINT_ID);
        assert_eq!(parsed.authority.address(), &AUTHORITY_ID);
        assert_eq!(parsed.freeze_authority.address(), &FREEZE_AUTHORITY_ID);
        assert!(parsed.mint.owned_by(parsed.token_program.address()));
        for authorities in [
            [wrong_authority.clone(), freeze_authority],
            [authority.clone(), wrong_authority.clone()],
        ] {
            let [authority, freeze_authority] = authorities;
            assert_eq!(
                MintAuthorities::try_accounts(
                    &program_id,
                    &[
                        mint.clone(),
                        authority,
                        freeze_authority,
                        token_program.clone()
                    ]
                )
                .err(),
                Some(ProgramError::InvalidAccountData),
            );
        }

        let accounts = [
            authority,
            mint.clone(),
            source.clone(),
            token_program.clone(),
        ];
        let ctx =
            Context::<TransferAccounts<'_>>::try_from_accounts(&program_id, &accounts).unwrap();

        assert_eq!(ctx.accounts.authority.address(), &AUTHORITY_ID);
        assert_eq!(ctx.accounts.mint.address(), &MINT_ID);
        assert!(ctx.accounts.source.is_writable());
        assert_eq!(
            ensure_mint(ctx.accounts.mint, ctx.accounts.token_program, None),
            Ok(6)
        );
        assert_eq!(
            ensure_mint(&mint, &token_program, Some(9)),
            Err(ProgramError::InvalidAccountData),
        );
        assert_eq!(
            ensure_mint(&mint, &non_executable_program, None),
            Err(ProgramError::IncorrectProgramId),
        );
        assert_eq!(
            ensure_token_account(&source, &non_executable_program, None, None),
            Err(ProgramError::IncorrectProgramId),
        );
        assert_eq!(
            ensure_token_account(&source, &token_program, None, None),
            Ok(())
        );
        assert_eq!(
            ensure_token_account(&source, &token_program, None, Some(&wrong_authority)),
            Err(ProgramError::InvalidAccountData),
        );
        assert_eq!(
            TransferAccounts::try_accounts(
                &program_id,
                &[wrong_authority, mint, source, token_program]
            )
            .err(),
            Some(ProgramError::Custom(45)),
        );
    }
}
