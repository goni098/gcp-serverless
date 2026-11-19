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
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let value = input.parse()?;
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

    let is_mint = attribute.path().is_ident("mint");
    let missing_program = || {
        let message = if is_mint {
            "`mint` requires `program = …`"
        } else {
            "`token_account` requires `program = …`"
        };
        Error::new_spanned(attribute.path(), message)
    };
    if matches!(&attribute.meta, Meta::Path(_)) {
        return Err(missing_program());
    }

    let mut program = None;
    let mut decimals = None;
    let mut mint = None;
    let mut authority = None;
    let mut freeze_authority = None;
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
            _ => {
                let message = if is_mint {
                    "expected `program`, `decimals`, `authority`, or `freeze_authority`"
                } else {
                    "expected `program`, `mint`, or `authority`"
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

    let program = program.ok_or_else(missing_program)?;
    let authority = optional(authority);
    let check = if is_mint {
        let decimals = optional(decimals);
        let freeze_authority = optional(freeze_authority);
        quote!(#runtime::spl::ensure_mint_constraints(
            #field, #program, #decimals, #authority, #freeze_authority
        ))
    } else {
        let mint = optional(mint);
        quote!(#runtime::spl::ensure_token_account(#field, #program, #authority, #mint))
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
}

fn optional(value: Option<Expr>) -> TokenStream {
    match value {
        Some(value) => quote!(::core::option::Option::Some(#value)),
        None => quote!(::core::option::Option::None),
    }
}
