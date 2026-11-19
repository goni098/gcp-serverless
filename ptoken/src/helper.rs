use pinocchio::error::ProgramError;
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_token::ID as TOKEN_ID;
use pinocchio_token::state::{Account, Mint};
use pinocchio_token_2022::ID as TOKEN_2022_ID;
use pinocchio_token_2022::state::{Account as Token2022Account, Mint as Token2022Mint};

use crate::error::{PtokenError, bail, ensure};

pub(crate) fn find_associated_token_address(
    owner: &Address,
    mint: &Address,
    token_program: &Address,
) -> Result<Address, ProgramError> {
    let (address, _) = Address::try_find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &pinocchio_associated_token_account::ID,
    )
    .ok_or(ProgramError::InvalidSeeds)?;

    Ok(address)
}

/// Check account constraints in the order they appear, with optional error overrides.
///
/// ```rust,ignore
/// // Require a signature from the payer.
/// ensure_account!(payer, signer)?;
///
/// // Require a writable vault at the expected address and customize the address error.
/// ensure_account!(
///     vault,
///     mut,
///     address = &expected_vault => PtokenError::InvalidVaultStateAddress,
/// )?;
///
/// // Check the program that owns the vault state account.
/// ensure_account!(vault, owner = program_id)?;
///
/// // Check the PDA for this program using the canonical bump.
/// let seeds: &[&[u8]] = &[b"vault", payer.address().as_ref(), mint.address().as_ref()];
/// ensure_account!(vault, seeds = seeds, bump)?;
///
/// // Check the PDA using a known bump and customize the error.
/// ensure_account!(
///     vault,
///     seeds = seeds,
///     bump = vault_bump => PtokenError::InvalidVaultStateAddress,
/// )?;
/// ```
macro_rules! ensure_account {
    ($account:expr, $($constraints:tt)+) => {{
        let account: &::pinocchio::AccountView = $account;
        'constraints: {
            $crate::helper::ensure_account!(@checks 'constraints, account, $($constraints)+,);
            ::pinocchio::ProgramResult::Ok(())
        }
    }};
    (@checks $exit:lifetime, $account:ident, $(,)?) => {};
    (@checks $exit:lifetime, $account:ident,
        seeds = $seeds:expr, bump $(= $bump:expr)? $(=> $error:expr)?, $($rest:tt)*
    ) => {
        $crate::helper::ensure_account!(@require $exit,
            $crate::helper::matches_program_address(
                $account, $seeds, $crate::helper::ensure_account!(@bump $($bump)?),
            ),
            ::pinocchio::error::ProgramError::InvalidSeeds $(, $error)?
        );
        $crate::helper::ensure_account!(@checks $exit, $account, $($rest)*);
    };
    (@checks $exit:lifetime, $account:ident,
        $constraint:ident $(= $value:expr)? $(=> $error:expr)?, $($rest:tt)*
    ) => {
        $crate::helper::ensure_account!(
            @check $exit, $account, $constraint $(= $value)? $(=> $error)?
        );
        $crate::helper::ensure_account!(@checks $exit, $account, $($rest)*);
    };
    (@bump) => { ::core::option::Option::None };
    (@bump $bump:expr) => { ::core::option::Option::Some($bump) };
    (@check $exit:lifetime, $account:ident, signer $(=> $error:expr)?) => {
        $crate::helper::ensure_account!(@require $exit, $account.is_signer(),
            ::pinocchio::error::ProgramError::MissingRequiredSignature $(, $error)?)
    };
    (@check $exit:lifetime, $account:ident, mut $(=> $error:expr)?) => {
        $crate::helper::ensure_account!(@require $exit, $account.is_writable(),
            $crate::error::PtokenError::AccountNotWritable $(, $error)?)
    };
    (@check $exit:lifetime, $account:ident, owner = $owner:expr $(=> $error:expr)?) => {
        $crate::helper::ensure_account!(@require $exit, $account.owned_by($owner),
            ::pinocchio::error::ProgramError::IllegalOwner $(, $error)?)
    };
    (@check $exit:lifetime, $account:ident, address = $address:expr $(=> $error:expr)?) => {
        $crate::helper::ensure_account!(@require $exit, $account.address() == $address,
            ::pinocchio::error::ProgramError::InvalidArgument $(, $error)?)
    };
    (@require $exit:lifetime, $condition:expr, $default:expr, $error:expr) => {
        $crate::helper::ensure_account!(@require $exit, $condition, $error)
    };
    (@require $exit:lifetime, $condition:expr, $error:expr) => {
        if !$condition {
            break $exit ::pinocchio::ProgramResult::Err(::core::convert::Into::into($error));
        }
    };
}

pub(crate) use ensure_account;

pub(crate) fn matches_program_address(
    account: &AccountView,
    seeds: &[&[u8]],
    bump: Option<u8>,
) -> bool {
    use pinocchio::address::{MAX_SEED_LEN, MAX_SEEDS};

    if seeds.len() >= MAX_SEEDS || seeds.iter().any(|seed| seed.len() > MAX_SEED_LEN) {
        return false;
    }

    let program_id = Address::new_from_array(crate::ID);
    let Some(bump) = bump else {
        return Address::try_find_program_address(seeds, &program_id)
            .is_some_and(|(address, _)| &address == account.address());
    };

    let mut seeds_with_bump: [&[u8]; MAX_SEEDS] = [&[]; MAX_SEEDS];
    seeds_with_bump[..seeds.len()].copy_from_slice(seeds);
    seeds_with_bump[seeds.len()] = core::slice::from_ref(&bump);
    Address::create_program_address(&seeds_with_bump[..=seeds.len()], &program_id)
        .is_ok_and(|address| &address == account.address())
}

pub(crate) fn check_mint(
    mint: &AccountView,
    token_program: &AccountView,
    expected_decimals: Option<u8>,
) -> Result<u8, ProgramError> {
    let program_id = token_program.address();
    ensure_account!(mint, owner = program_id => PtokenError::InvalidMintOwner)?;

    let (initialized, decimals) = match program_id {
        &TOKEN_ID => {
            let mint = Mint::from_account_view(mint)?;
            (mint.is_initialized(), mint.decimals())
        }
        &TOKEN_2022_ID => {
            let mint = Token2022Mint::from_account_view(mint)?;
            (mint.is_initialized(), mint.decimals())
        }
        _ => bail!(PtokenError::InvalidTokenProgram),
    };

    ensure!(initialized, ProgramError::UninitializedAccount);
    if let Some(expected_decimals) = expected_decimals {
        ensure!(
            decimals == expected_decimals,
            ProgramError::InvalidAccountData
        );
    }

    Ok(decimals)
}

/// Check an initialized SPL mint and return its decimals.
///
/// ```rust,ignore
/// // Read the decimals from a mint owned by the supplied token program.
/// let decimals = ensure_mint!(mint, program = token_program)?;
///
/// // Also require the mint to have exactly six decimals.
/// ensure_mint!(mint, program = token_program, decimals = 6)?;
/// ```
macro_rules! ensure_mint {
    ($mint:expr, program = $program:expr, decimals = $decimals:expr $(,)?) => {
        $crate::helper::check_mint($mint, $program, ::core::option::Option::Some($decimals))
    };
    ($mint:expr, program = $program:expr $(,)?) => {
        $crate::helper::check_mint($mint, $program, ::core::option::Option::None)
    };
}

pub(crate) use ensure_mint;

pub(crate) fn check_token_account(
    account: &AccountView,
    token_program: &AccountView,
    authority: &AccountView,
    mint: &AccountView,
) -> ProgramResult {
    ensure!(
        account.owned_by(token_program.address()),
        PtokenError::InvalidTokenAccount
    );

    let (authority_address, mint_address) = match token_program.address() {
        &TOKEN_2022_ID => {
            let token_account = Token2022Account::from_account_view(account)?;
            (
                token_account.owner().to_bytes(),
                token_account.mint().to_bytes(),
            )
        }
        &TOKEN_ID => {
            let token_account = Account::from_account_view(account)?;
            (
                token_account.owner().to_bytes(),
                token_account.mint().to_bytes(),
            )
        }
        _ => {
            bail!(PtokenError::InvalidTokenProgram)
        }
    };

    ensure!(
        authority.address().as_array() == &authority_address,
        PtokenError::InvalidTokenAccount
    );
    ensure!(
        mint.address().as_array() == &mint_address,
        PtokenError::InvalidTokenAccount
    );

    Ok(())
}

/// Check the token program, authority, and mint, and require writability with `mut`.
///
/// ```rust,ignore
/// // A transfer source must be writable and belong to the payer for this mint.
/// ensure_token_account!(
///     source,
///     mut,
///     program = token_program,
///     authority = payer,
///     mint = mint,
/// )?;
///
/// // Omit `mut` when reading the account. Either writability flag is accepted.
/// ensure_token_account!(source, program = token_program, authority = payer, mint = mint)?;
/// ```
macro_rules! ensure_token_account {
    ($account:expr, mut, program = $program:expr, authority = $authority:expr, mint = $mint:expr $(,)?) => {{
        let account: &::pinocchio::AccountView = $account;
        match $crate::helper::ensure_account!(account, mut) {
            Ok(()) => $crate::helper::ensure_token_account!(
                account,
                program = $program,
                authority = $authority,
                mint = $mint,
            ),
            Err(error) => Err(error),
        }
    }};
    ($account:expr, program = $program:expr, authority = $authority:expr, mint = $mint:expr $(,)?) => {
        $crate::helper::check_token_account($account, $program, $authority, $mint)
    };
}

pub(crate) use ensure_token_account;

#[cfg(test)]
mod tests {
    use pinocchio::account::{NOT_BORROWED, RuntimeAccount};

    use super::*;

    #[test]
    fn account_constraints_return_the_first_error() {
        let mut raw = RuntimeAccount::default();
        // SAFETY: The header is live and declares zero bytes of account data.
        let account = unsafe { AccountView::new_unchecked(&raw mut raw) };
        let other_address = Address::new_from_array([1; 32]);

        assert_eq!(
            ensure_account!(&account, signer, mut),
            Err(ProgramError::MissingRequiredSignature),
        );
        assert_eq!(
            ensure_account!(
                &account,
                address = &other_address => PtokenError::InvalidVaultStateAddress,
                mut,
            ),
            Err(PtokenError::InvalidVaultStateAddress.into()),
        );
    }

    #[test]
    fn pda_constraint_distinguishes_canonical_and_explicit_bumps() {
        let seeds: &[&[u8]] = &[b"vault"];
        let program_id = Address::new_from_array(crate::ID);
        let (canonical_address, canonical_bump) =
            Address::try_find_program_address(seeds, &program_id).unwrap();
        let (other_address, other_bump) = (0..canonical_bump)
            .rev()
            .find_map(|bump| {
                Address::create_program_address(&[b"vault", &[bump]], &program_id)
                    .ok()
                    .map(|address| (address, bump))
            })
            .unwrap();
        let mut canonical = RuntimeAccount {
            address: canonical_address,
            ..RuntimeAccount::default()
        };
        let mut other = RuntimeAccount {
            address: other_address,
            ..RuntimeAccount::default()
        };
        // SAFETY: Both headers are live and declare zero bytes of account data.
        let (canonical, other) = unsafe {
            (
                AccountView::new_unchecked(&raw mut canonical),
                AccountView::new_unchecked(&raw mut other),
            )
        };

        assert_eq!(ensure_account!(&canonical, seeds = seeds, bump), Ok(()));
        assert_eq!(
            ensure_account!(&canonical, seeds = seeds, bump = canonical_bump),
            Ok(()),
        );
        assert_eq!(
            ensure_account!(&other, seeds = seeds, bump = other_bump),
            Ok(()),
        );
        assert_eq!(
            ensure_account!(&other, seeds = seeds, bump),
            Err(ProgramError::InvalidSeeds),
        );
        assert_eq!(
            ensure_account!(
                &canonical,
                seeds = seeds,
                bump = other_bump => PtokenError::InvalidVaultStateAddress,
                signer,
            ),
            Err(PtokenError::InvalidVaultStateAddress.into()),
        );
        let too_many_seeds: [&[u8]; pinocchio::address::MAX_SEEDS] =
            [&[]; pinocchio::address::MAX_SEEDS];
        assert_eq!(
            ensure_account!(&canonical, seeds = &too_many_seeds, bump = canonical_bump),
            Err(ProgramError::InvalidSeeds),
        );
    }

    #[test]
    fn validates_mints_and_read_only_token_accounts_for_both_token_programs() {
        use mollusk_svm_programs_token_2022::{
            create_account_for_mint, create_account_for_token_account,
        };
        use spl_token_interface::state::{
            Account as TokenFixture, AccountState, Mint as MintFixture,
        };

        #[repr(C)]
        struct AccountWithData<const LEN: usize> {
            header: RuntimeAccount,
            data: [u8; LEN],
        }

        let mint_address = Address::new_from_array([1; 32]);
        let authority_address = Address::new_from_array([2; 32]);
        let mint_data = create_account_for_mint(MintFixture {
            decimals: 6,
            is_initialized: true,
            ..MintFixture::default()
        })
        .data;
        let token_data = create_account_for_token_account(TokenFixture {
            mint: mint_address.to_bytes().into(),
            owner: authority_address.to_bytes().into(),
            state: AccountState::Initialized,
            ..TokenFixture::default()
        })
        .data;

        for program_id in [TOKEN_ID, TOKEN_2022_ID] {
            let mut program = RuntimeAccount {
                address: program_id.clone(),
                ..RuntimeAccount::default()
            };
            let mut authority = RuntimeAccount {
                address: authority_address.clone(),
                ..RuntimeAccount::default()
            };
            let mut mint = AccountWithData::<{ Mint::LEN }> {
                header: RuntimeAccount {
                    address: mint_address.clone(),
                    owner: program_id.clone(),
                    borrow_state: NOT_BORROWED,
                    data_len: Mint::LEN as u64,
                    ..RuntimeAccount::default()
                },
                data: mint_data.as_slice().try_into().unwrap(),
            };
            let mut token = AccountWithData::<{ Account::LEN }> {
                header: RuntimeAccount {
                    owner: program_id,
                    borrow_state: NOT_BORROWED,
                    data_len: Account::LEN as u64,
                    ..RuntimeAccount::default()
                },
                data: token_data.as_slice().try_into().unwrap(),
            };

            // SAFETY: The headers are live and each is followed by its declared data.
            let (program, authority, mint, token) = unsafe {
                (
                    AccountView::new_unchecked(&raw mut program),
                    AccountView::new_unchecked(&raw mut authority),
                    AccountView::new_unchecked((&raw mut mint).cast()),
                    AccountView::new_unchecked((&raw mut token).cast()),
                )
            };

            assert_eq!(ensure_mint!(&mint, program = &program, decimals = 6), Ok(6));
            assert_eq!(
                ensure_mint!(&mint, program = &program, decimals = 9),
                Err(ProgramError::InvalidAccountData),
            );
            assert_eq!(
                ensure_token_account!(
                    &token,
                    program = &program,
                    authority = &authority,
                    mint = &mint
                ),
                Ok(()),
            );
            assert_eq!(
                ensure_token_account!(
                    &token,
                    mut,
                    program = &program,
                    authority = &authority,
                    mint = &mint
                ),
                Err(PtokenError::AccountNotWritable.into()),
            );
        }
    }
}
