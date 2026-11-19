# Pinocchio Constraints

`no_std` account validation for Pinocchio programs. Derive `Accounts` on an instruction's account struct to bind and validate its accounts and pass them to a handler through `Context`. The field attributes are inspired by [Anchor's account constraints](https://www.anchor-lang.com/docs/references/account-constraints).

## Installation

For a program beside this crate, add the local dependency in `Cargo.toml`.

```toml
[dependencies]
pinocchio-constraints = { path = "../pinocchio-constraints" }
```

Enable SPL Token and Token-2022 validation with `features = ["spl"]` on that dependency. The `spl` feature is disabled by default.

## Instruction accounts

```rust
use pinocchio_constraints::{AccountView, Accounts, Address, Context, ProgramError};

#[derive(Accounts)]
struct CreateVault<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,

    #[account(mut, uninitialized, seeds = [b"vault", payer.address().as_ref()], bump)]
    vault: &'acc AccountView,

    #[program(system)]
    system_program: &'acc AccountView,
}

fn process(program_id: &Address, accounts: &[AccountView]) -> Result<(), ProgramError> {
    let ctx = Context::<CreateVault<'_>>::try_from_accounts(program_id, accounts)?;
    let vault = ctx.accounts.vault;
    let bump = ctx.bumps.vault;
    let extra_accounts = ctx.remaining_accounts;
    Ok(())
}
```

The client passes `[payer, vault, system_program]` in that order, then any extra accounts. The derive binds one `&AccountView` per field in declaration order before checking constraints, so a constraint can refer to a field declared later. If the slice has fewer entries than declared fields, binding returns `ProgramError::NotEnoughAccountKeys`. Extra accounts are passed to the handler in `ctx.remaining_accounts` in their original order.

## Account constraints

| Constraint | What it validates | Default `ProgramError` |
| --- | --- | --- |
| `#[account(signer)]` | The runtime marks the account as a signer for the current instruction. | `MissingRequiredSignature` when the signer flag is clear. |
| `#[account(mut)]` | The runtime marks the account as writable for the current instruction. | `Immutable` when the writable flag is clear. |
| `#[account(initialized)]` | The account holds at least one lamport. | `UninitializedAccount` when the balance is zero. |
| `#[account(uninitialized)]` | The account has a zero lamport balance. | `AccountAlreadyInitialized` when the balance is nonzero. |
| `#[account(owner = EXPECTED_OWNER)]` | The account's runtime owner equals the expected program address. | `InvalidAccountOwner` when the addresses differ. |

The two initialization flags check only lamports. General constraints can be combined, as in `#[account(mut, uninitialized)]`.

The `owner` expression accepts an address, an address reference, or a 32-byte array. To use another account's address, write `owner = program.address()`.

### PDA seeds and bump

| Constraint | What it validates | Default `ProgramError` |
| --- | --- | --- |
| `#[account(seeds = seeds, bump)]` | Derives the canonical PDA from the base seeds and instruction program ID and compares it to the field's address. | `InvalidSeeds` if the seeds exceed the limits, derivation fails, or the address differs. |
| `#[account(seeds = seeds, bump = value)]` | Appends the supplied `u8` bump to the base seeds, derives the PDA with the instruction program ID, and compares it to the field's address. This accepts valid noncanonical bumps. | `InvalidSeeds` if the seeds exceed the limits, the bump fails to produce a PDA, or the address differs. |

Supply up to 15 base seeds of at most 32 bytes each, such as `[b"vault", payer.address().as_ref()]`. The validator reserves one seed for the bump. The derive stores the validated bump on `ctx.bumps.<field>`. An accounts struct without any PDA fields uses `()` for `ctx.bumps`.

## Program constraints

| Constraint | What it validates | Default `ProgramError` |
| --- | --- | --- |
| `#[program(system)]` | The account is executable and its address is the System Program ID. | `IncorrectProgramId` if the address differs or the executable flag is clear. |
| `#[program(spl)]` | The account is executable and its address is the SPL Token or Token-2022 program ID. Requires the `spl` feature. | `IncorrectProgramId` if neither ID matches or the executable flag is clear. |
| `#[program(id = EXPECTED_ID)]` | The account is executable and its address matches the supplied ID. | `IncorrectProgramId` if the address differs or the executable flag is clear. |

The `id` expression accepts an address, an address reference, or a 32-byte array.

## SPL constraints

`#[mint(...)]` and `#[token_account(...)]` require the `spl` feature and a `program` argument that refers to an `AccountView`. Each checks that this program is executable, has the SPL Token or Token-2022 program ID, and owns the annotated account. The program ID selects the data layout validator. For extended Token-2022 layouts, the validator checks the account-type marker at byte offset 165 and rejects the multisig length.

```rust
use pinocchio_constraints::{AccountView, Accounts};

#[derive(Accounts)]
struct TransferAccounts<'acc> {
    #[account(signer)]
    payer: &'acc AccountView,

    #[mint(program = token_program, decimals = 6)]
    mint: &'acc AccountView,

    #[account(mut)]
    #[token_account(
        program = token_program,
        mint = mint,
        authority = payer,
    )]
    source: &'acc AccountView,

    token_program: &'acc AccountView,
}
```

### `#[mint(program = token_program)]`

Checks the mint layout and requires the mint's initialization byte to equal `1`. SPL Token requires the exact base mint length. Token-2022 accepts the base length or a layout with the mint account-type marker. Optional arguments add these comparisons.

| Argument | What it validates |
| --- | --- |
| `decimals = value` | The stored decimals equal the supplied `u8`. |
| `authority = account` | The mint authority is present and equals `account.address()`. |
| `freeze_authority = account` | The freeze authority is present and equals `account.address()`. |

| Default `ProgramError` | When it occurs |
| --- | --- |
| `IncorrectProgramId` | The program ID is unsupported, the program account is non-executable, or the mint's runtime owner differs from the program ID. |
| `AccountBorrowFailed` | The mint data cannot be borrowed. |
| `InvalidAccountData` | The layout is invalid, the decimals differ from the expected value, or a requested authority is absent or differs from the expected address. |
| `UninitializedAccount` | The mint's initialization byte is any value other than `1`. |

### `#[token_account(program = token_program)]`

Checks the token account layout and accepts both `Initialized` and `Frozen` states. SPL Token requires the exact base token account length. Token-2022 accepts the base length or a layout with the token account-type marker. Optional arguments add these comparisons.

| Argument | What it validates |
| --- | --- |
| `mint = account` | The token account's stored mint address equals `account.address()`. |
| `authority = account` | The token account's stored owner authority equals `account.address()`. This is the authority recorded in the token data. |

| Default `ProgramError` | When it occurs |
| --- | --- |
| `IncorrectProgramId` | The program ID is unsupported, the program account is non-executable, or the token account's runtime owner differs from the program ID. |
| `AccountBorrowFailed` | The token account data cannot be borrowed. |
| `InvalidAccountData` | The layout or state byte is invalid, or the stored mint or owner authority differs from the expected address. |
| `UninitializedAccount` | The token account's stored state is `Uninitialized`. |

SPL authority arguments compare addresses. Use `#[account(signer)]` on an authority field when the instruction requires its signature.

## Compile-time checks

- Derive `Accounts` on a named struct with one account lifetime and shared `&AccountView` fields.
- Use at most one of `#[program(...)]`, `#[mint(...)]`, and `#[token_account(...)]` per field. Combine it with general `#[account(...)]` constraints as needed.
- Pair `seeds` with `bump`, and choose at most one of `initialized` and `uninitialized` per field.
- Supply known, unique constraints and arguments. SPL attributes require the `spl` feature and a `program` argument.
