use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Error, Expr, Ident, Meta, Result, Token};

use super::{parse_custom_error, with_custom_error};

struct Argument {
    name: Ident,
    value: Expr,
    error: Option<Expr>,
}

impl Parse for Argument {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: Ident = input.parse()?;
        let value = if name == "allow_uninitialized" {
            syn::parse_quote!(true)
        } else {
            input.parse::<Token![=]>()?;
            input.parse()?
        };
        let error = parse_custom_error(input)?;
        Ok(Self { name, value, error })
    }
}

pub(super) fn expand(
    attribute: &Attribute,
    field: &Ident,
    runtime: &TokenStream,
) -> Result<TokenStream> {
    if !cfg!(feature = "spl") {
        return Err(Error::new_spanned(
            attribute.path(),
            "SPL constraints require the `pinocchio-constraints/spl` feature",
        ));
    }

    let path = attribute.path();
    let is_mint = path.is_ident("mint");
    let is_associated = path.is_ident("associated_token");
    let missing_argument = |argument| {
        let name = path.get_ident().expect("SPL attribute has a single name");
        Error::new_spanned(path, format!("`{name}` requires `{argument} = …`"))
    };
    if matches!(&attribute.meta, Meta::Path(_)) {
        return Err(missing_argument("program"));
    }

    let mut program = None;
    let mut decimals = None;
    let mut mint = None;
    let mut authority = None;
    let mut freeze_authority = None;
    let mut allow_uninitialized = None;
    let mut error = None;
    for argument in
        attribute.parse_args_with(Punctuated::<Argument, Token![,]>::parse_terminated)?
    {
        let slot = match argument.name.to_string().as_str() {
            "program" => &mut program,
            "decimals" if is_mint => &mut decimals,
            "mint" if !is_mint => &mut mint,
            "authority" => &mut authority,
            "freeze_authority" if is_mint => &mut freeze_authority,
            "allow_uninitialized" if !is_associated => &mut allow_uninitialized,
            _ => {
                let message = if is_mint {
                    "expected `program`, `decimals`, `authority`, `freeze_authority`, or `allow_uninitialized`"
                } else if is_associated {
                    "expected `program`, `mint`, or `authority`"
                } else {
                    "expected `program`, `mint`, `authority`, or `allow_uninitialized`"
                };
                return Err(Error::new(argument.name.span(), message));
            }
        };
        if slot.is_some() {
            return Err(Error::new(argument.name.span(), "duplicate SPL argument"));
        }
        *slot = Some(argument.value);
        if let Some(custom_error) = argument.error {
            if error.is_some() {
                return Err(Error::new(
                    argument.name.span(),
                    "specify one custom error for this SPL attribute",
                ));
            }
            error = Some(custom_error);
        }
    }

    let program = program.ok_or_else(|| missing_argument("program"))?;
    let allow_uninitialized = allow_uninitialized.is_some();
    let check = if is_mint {
        let decimals = optional(decimals);
        let authority = optional(authority);
        let freeze_authority = optional(freeze_authority);
        quote!(#runtime::spl::validate_mint(
            #field, #program, #decimals, #authority, #freeze_authority, #allow_uninitialized
        ))
    } else if is_associated {
        let mint = mint.ok_or_else(|| missing_argument("mint"))?;
        let authority = authority.ok_or_else(|| missing_argument("authority"))?;
        quote!(#runtime::spl::ensure_associated_token(#field, #program, #mint, #authority))
    } else {
        let authority = optional(authority);
        let mint = optional(mint);
        quote!(#runtime::spl::validate_token_account(
            #field, #program, #authority, #mint, #allow_uninitialized
        ))
    };
    let check = with_custom_error(check, error.as_ref(), runtime);
    Ok(quote!(#check;))
}

#[cfg(all(test, feature = "spl"))]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn mint_requires_a_program() {
        let attributes: [Attribute; 3] = [
            parse_quote!(#[mint]),
            parse_quote!(#[mint()]),
            parse_quote!(#[mint(authority = payer)]),
        ];
        for attribute in attributes {
            let error = expand(&attribute, &parse_quote!(mint), &quote!(runtime)).unwrap_err();
            assert_eq!(error.to_string(), "`mint` requires `program = …`");
        }
    }

    #[test]
    fn associated_token_requires_program_mint_and_authority() {
        let cases: [(Attribute, &str); 3] = [
            (
                parse_quote!(#[associated_token(mint = mint, authority = payer)]),
                "program",
            ),
            (
                parse_quote!(#[associated_token(program = token_program, authority = payer)]),
                "mint",
            ),
            (
                parse_quote!(#[associated_token(program = token_program, mint = mint)]),
                "authority",
            ),
        ];
        for (attribute, argument) in cases {
            let error = expand(&attribute, &parse_quote!(ata), &quote!(runtime)).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("`associated_token` requires `{argument} = …`")
            );
        }
    }
}

fn optional(value: Option<Expr>) -> TokenStream {
    match value {
        Some(value) => quote!(::core::option::Option::Some(#value)),
        None => quote!(::core::option::Option::None),
    }
}
