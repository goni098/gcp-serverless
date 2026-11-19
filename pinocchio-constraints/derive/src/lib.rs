use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, Fields, GenericParam, Ident, Result, Token, Type,
    parse_macro_input,
};

mod spl;

/// Bind accounts in struct field order and validate each field's constraints.
#[proc_macro_derive(
    Accounts,
    attributes(account, program, mint, token_account, associated_token, instruction)
)]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[derive(Clone, Copy, PartialEq)]
enum ConstraintKind {
    Signer,
    Writable,
    Initialized,
    Uninitialized,
    Owner,
    Seeds,
    Bump,
    Program,
    SplProgram,
}

struct Constraint {
    name: Ident,
    kind: ConstraintKind,
    value: Option<Expr>,
    error: Option<Expr>,
}

impl Parse for Constraint {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.call(Ident::parse_any)?;
        let kind = match name.to_string().as_str() {
            "signer" => ConstraintKind::Signer,
            "mut" => ConstraintKind::Writable,
            "initialized" => ConstraintKind::Initialized,
            "uninitialized" => ConstraintKind::Uninitialized,
            "owner" => ConstraintKind::Owner,
            "seeds" => ConstraintKind::Seeds,
            "bump" => ConstraintKind::Bump,
            _ => {
                return Err(Error::new(
                    name.span(),
                    "expected `signer`, `mut`, `initialized`, `uninitialized`, `owner`, `seeds`, or `bump`",
                ));
            }
        };
        let value = if matches!(kind, ConstraintKind::Owner | ConstraintKind::Seeds)
            || (kind == ConstraintKind::Bump && input.peek(Token![=]))
        {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self {
            name,
            kind,
            value,
            error: parse_custom_error(input)?,
        })
    }
}

fn parse_custom_error(input: ParseStream<'_>) -> Result<Option<Expr>> {
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        Ok(Some(input.parse()?))
    } else {
        Ok(None)
    }
}

fn parse_program(input: ParseStream<'_>) -> Result<Constraint> {
    let name = input.parse::<Ident>()?;
    let (kind, value) = match name.to_string().as_str() {
        "system" => (ConstraintKind::Program, None),
        "spl" => {
            if !cfg!(feature = "spl") {
                return Err(Error::new(
                    name.span(),
                    "`program(spl)` requires the `pinocchio-constraints/spl` feature",
                ));
            }
            (ConstraintKind::SplProgram, None)
        }
        "id" => {
            input.parse::<Token![=]>()?;
            (ConstraintKind::Program, Some(input.parse()?))
        }
        _ => {
            return Err(Error::new(
                name.span(),
                "expected `system`, `spl`, or `id = <address>`",
            ));
        }
    };
    let error = parse_custom_error(input)?;
    if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
    }
    Ok(Constraint {
        name,
        kind,
        value,
        error,
    })
}

#[cfg(all(test, not(feature = "spl")))]
#[test]
fn spl_program_requires_the_spl_feature() {
    let attribute: Attribute = syn::parse_quote!(#[program(spl)]);
    let error = attribute.parse_args_with(parse_program).err().unwrap();
    assert_eq!(
        error.to_string(),
        "`program(spl)` requires the `pinocchio-constraints/spl` feature",
    );
}

enum FieldConstraint {
    Account(Constraint),
    Spl(Tokens),
}

impl FieldConstraint {
    fn account(&self) -> Option<&Constraint> {
        match self {
            Self::Account(constraint) => Some(constraint),
            Self::Spl(_) => None,
        }
    }
}

fn parse_constraints(
    attributes: &[Attribute],
    field: &Ident,
    runtime: &Tokens,
) -> Result<Vec<FieldConstraint>> {
    let mut constraints = Vec::new();
    let mut has_account_type = false;
    for attribute in attributes {
        let path = attribute.path();
        if path.is_ident("account") {
            constraints.extend(
                attribute
                    .parse_args_with(Punctuated::<Constraint, Token![,]>::parse_terminated)?
                    .into_iter()
                    .map(FieldConstraint::Account),
            );
            continue;
        }
        let is_program = path.is_ident("program");
        if !is_program
            && !path.is_ident("mint")
            && !path.is_ident("token_account")
            && !path.is_ident("associated_token")
        {
            continue;
        }
        if has_account_type {
            return Err(Error::new_spanned(
                path,
                "a field can use only one of `#[program(...)]`, `#[mint(...)]`, `#[token_account(...)]`, or `#[associated_token(...)]`",
            ));
        }
        has_account_type = true;
        if is_program {
            constraints.push(FieldConstraint::Account(
                attribute.parse_args_with(parse_program)?,
            ));
        } else {
            constraints.push(FieldConstraint::Spl(spl::expand(
                attribute, field, runtime,
            )?));
        }
    }
    let mut seen = Vec::new();
    for constraint in constraints.iter().filter_map(FieldConstraint::account) {
        if seen.contains(&constraint.kind) {
            return Err(Error::new(
                constraint.name.span(),
                "duplicate account constraint",
            ));
        }
        let conflicts = match constraint.kind {
            ConstraintKind::Initialized => seen.contains(&ConstraintKind::Uninitialized),
            ConstraintKind::Uninitialized => seen.contains(&ConstraintKind::Initialized),
            _ => false,
        };
        if conflicts {
            return Err(Error::new(
                constraint.name.span(),
                "`initialized` and `uninitialized` cannot be used on the same field",
            ));
        }
        seen.push(constraint.kind);
    }
    Ok(constraints)
}

fn with_custom_error(check: Tokens, error: Option<&Expr>, runtime: &Tokens) -> Tokens {
    match error {
        Some(error) => quote!(
            #check.map_err(|_| ::core::convert::Into::<#runtime::ProgramError>::into(#error))?
        ),
        None => quote!(#check?),
    }
}

struct InstructionArg {
    name: Ident,
    ty: Type,
}

impl Parse for InstructionArg {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("expected one `name: Type` instruction argument binding"));
        }
        Ok(Self { name, ty })
    }
}

fn expand(input: DeriveInput) -> Result<Tokens> {
    let mut instruction_arg = None;
    for attribute in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("instruction"))
    {
        if instruction_arg.is_some() {
            return Err(Error::new_spanned(
                attribute,
                "duplicate `#[instruction(...)]`",
            ));
        }
        instruction_arg = Some(attribute.parse_args::<InstructionArg>()?);
    }

    let name = input.ident;
    let visibility = input.vis;
    let Data::Struct(data) = input.data else {
        return Err(Error::new(name.span(), "Accounts requires a struct"));
    };
    let Fields::Named(fields) = data.fields else {
        return Err(Error::new(name.span(), "Accounts requires named fields"));
    };
    let mut params = input.generics.params.iter();
    let (Some(GenericParam::Lifetime(param)), None) = (params.next(), params.next()) else {
        return Err(Error::new(
            name.span(),
            "Accounts requires one account lifetime",
        ));
    };
    let lifetime = &param.lifetime;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let runtime = quote!(::pinocchio_constraints);
    let program_id = Ident::new("__program_id", Span::mixed_site());
    let accounts = Ident::new("__accounts", Span::mixed_site());
    let remaining_accounts = Ident::new("__remaining_accounts", Span::mixed_site());
    let validated_bumps = Ident::new("__validated_bumps", Span::mixed_site());

    let mut bindings = Vec::with_capacity(fields.named.len());
    let mut requires_mut = false;
    let mut checks = Vec::new();
    let mut bump_fields = Vec::new();
    let mut bump_initializers = Vec::new();
    for (index, field) in fields.named.into_iter().enumerate() {
        let Type::Reference(reference) = &field.ty else {
            return Err(Error::new_spanned(
                field.ty,
                "expected an AccountView reference",
            ));
        };
        if reference.lifetime.as_ref() != Some(lifetime) {
            return Err(Error::new_spanned(
                field.ty,
                "expected a reference with the account lifetime",
            ));
        }
        requires_mut |= reference.mutability.is_some();

        let field_name = field.ident.expect("named struct field");
        if let Some(attribute) = field
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("instruction"))
        {
            return Err(Error::new_spanned(
                attribute,
                "place `#[instruction(...)]` on the accounts struct",
            ));
        }
        if let Some(arg) = &instruction_arg
            && arg.name.unraw() == field_name.unraw()
        {
            return Err(Error::new(
                arg.name.span(),
                "instruction argument binding conflicts with an account field",
            ));
        }
        let constraints = parse_constraints(&field.attrs, &field_name, &runtime)?;
        let seeds = constraints
            .iter()
            .filter_map(FieldConstraint::account)
            .find(|constraint| constraint.kind == ConstraintKind::Seeds);
        let bump = constraints
            .iter()
            .filter_map(FieldConstraint::account)
            .find(|constraint| constraint.kind == ConstraintKind::Bump);
        let bump_binding = format_ident!("__bump_{}", index, span = Span::mixed_site());
        let mut pda_check = match (seeds, bump) {
            (Some(seeds), Some(bump)) => {
                if seeds.error.is_some() && bump.error.is_some() {
                    return Err(Error::new(
                        bump.name.span(),
                        "specify one custom error for the PDA constraint",
                    ));
                }
                let seeds_value = seeds.value.as_ref().expect("seeds requires a value");
                let seeds_value = match seeds_value {
                    Expr::Reference(_) => quote!(#seeds_value),
                    _ => quote!(&(#seeds_value)),
                };
                let bump_value = match &bump.value {
                    Some(value) => quote!(::core::option::Option::Some(#value)),
                    None => quote!(::core::option::Option::None),
                };
                let check = quote!(
                    #runtime::ensure_pda(#field_name, #program_id, #seeds_value, #bump_value)
                );
                let check = with_custom_error(
                    check,
                    seeds.error.as_ref().or(bump.error.as_ref()),
                    &runtime,
                );
                bump_fields.push(quote!(
                    /// Validated PDA bump for this account.
                    pub #field_name: u8,
                ));
                bump_initializers.push(quote!(#field_name: #bump_binding));
                Some(quote!(let #bump_binding = #check;))
            }
            (Some(seeds), None) => {
                return Err(Error::new(seeds.name.span(), "`seeds` requires `bump`"));
            }
            (None, Some(bump)) => {
                return Err(Error::new(bump.name.span(), "`bump` requires `seeds`"));
            }
            (None, None) => None,
        };

        for constraint in constraints {
            let constraint = match constraint {
                FieldConstraint::Account(constraint) => constraint,
                FieldConstraint::Spl(check) => {
                    checks.push(check);
                    continue;
                }
            };
            let check = match constraint.kind {
                ConstraintKind::Signer => quote!(#runtime::ensure_signer(#field_name)),
                ConstraintKind::Writable => quote!(#runtime::ensure_writable(#field_name)),
                ConstraintKind::Initialized => quote!(#runtime::ensure_initialized(#field_name)),
                ConstraintKind::Uninitialized => {
                    quote!(#runtime::ensure_uninitialized(#field_name))
                }
                ConstraintKind::Owner => {
                    let owner = constraint.value.expect("owner requires a value");
                    quote!(#runtime::ensure_owner(#field_name, #owner))
                }
                ConstraintKind::Program => match constraint.value {
                    Some(id) => quote!(#runtime::ensure_program(#field_name, #id)),
                    None => quote!(#runtime::ensure_system_program(#field_name)),
                },
                ConstraintKind::SplProgram => {
                    quote!(#runtime::spl::ensure_spl_program(#field_name))
                }
                ConstraintKind::Seeds | ConstraintKind::Bump => {
                    checks.extend(pda_check.take());
                    continue;
                }
            };
            let check = with_custom_error(check, constraint.error.as_ref(), &runtime);
            checks.push(quote!(#check;));
        }
        bindings.push(field_name);
    }

    let (bumps_item, bumps_type, bumps_value) = if bump_fields.is_empty() {
        (quote!(), quote!(()), quote!(()))
    } else {
        let bumps_name = format_ident!("{name}Bumps");
        (
            quote!(
                /// Validated PDA bumps for this instruction's accounts.
                #visibility struct #bumps_name { #(#bump_fields)* }
            ),
            quote!(#bumps_name),
            quote!(#bumps_name { #(#bump_initializers),* }),
        )
    };

    let (slice_mutability, shared_reborrows) = if requires_mut {
        // Slice matching binds each view mutably. Expose shared views to constraint
        // expressions until their checks finish, then use the original bindings.
        (quote!(mut), quote!(#(let #bindings = &*#bindings;)*))
    } else {
        (quote!(), quote!())
    };
    let account_slice = quote!(&#lifetime #slice_mutability [#runtime::AccountView]);
    let uses_args_trait = requires_mut || instruction_arg.is_some();
    let (accounts_trait, method, slice_type) = if uses_args_trait {
        (
            quote!(AccountsWithArgs),
            quote!(try_accounts_with_args),
            quote!(type AccountSlice = #account_slice;),
        )
    } else {
        (quote!(Accounts), quote!(try_accounts), quote!())
    };
    let without_args_impl = (requires_mut && instruction_arg.is_none()).then(|| {
        quote!(impl #impl_generics #runtime::AccountsWithoutArgs<#lifetime> for #name #type_generics #where_clause {})
    });
    let (args_type, args_parameter) = match instruction_arg {
        Some(InstructionArg { name, ty }) => (
            quote!(type Args = #ty;),
            quote!(#[allow(unused_variables)] #name: &Self::Args,),
        ),
        None if requires_mut => (
            quote!(
                type Args = ();
            ),
            quote!(_: &Self::Args,),
        ),
        None => (quote!(), quote!()),
    };

    Ok(quote! {
        #bumps_item
        #without_args_impl
        impl #impl_generics #runtime::#accounts_trait<#lifetime> for #name #type_generics #where_clause {
            #slice_type
            #args_type
            type Bumps = #bumps_type;

            fn #method(
                #program_id: &#runtime::Address,
                #accounts: #account_slice,
                #args_parameter
            ) -> ::core::result::Result<(Self, Self::Bumps, #account_slice), #runtime::ProgramError> {
                let [#(#bindings,)* #remaining_accounts @ ..] = #accounts else {
                    return ::core::result::Result::Err(#runtime::ProgramError::NotEnoughAccountKeys);
                };
                let #validated_bumps = {
                    #shared_reborrows
                    #(#checks)*
                    #bumps_value
                };
                ::core::result::Result::Ok((Self { #(#bindings),* }, #validated_bumps, #remaining_accounts))
            }
        }
    })
}
