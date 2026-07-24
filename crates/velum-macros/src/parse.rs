use proc_macro2::TokenStream;
use syn::{
    Attribute, Expr, ExprLit, Lit, Meta, Result, Token, punctuated::Punctuated, spanned::Spanned,
};

pub fn metadata(tokens: TokenStream) -> Result<Vec<Meta>> {
    use syn::parse::Parser;

    Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(tokens)
        .map(|values| values.into_iter().collect())
}

pub fn take_js_metadata(attributes: &mut Vec<Attribute>) -> Result<Vec<Meta>> {
    let mut retained = Vec::new();
    let mut metadata = Vec::new();
    for attribute in core::mem::take(attributes) {
        if attribute.path().is_ident("js") {
            let Meta::List(list) = attribute.meta else {
                return Err(syn::Error::new(attribute.span(), "expected #[js(...)]"));
            };
            metadata.extend(self::metadata(list.tokens)?);
        } else {
            retained.push(attribute);
        }
    }
    *attributes = retained;
    Ok(metadata)
}

pub fn string_value(meta: &Meta, key: &str) -> Result<Option<String>> {
    let Meta::NameValue(value) = meta else {
        return Ok(None);
    };
    if !value.path.is_ident(key) {
        return Ok(None);
    }
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = &value.value
    else {
        return Err(syn::Error::new(
            value.value.span(),
            format!("'{key}' must be a string literal"),
        ));
    };
    Ok(Some(value.value()))
}

pub fn path_flag(meta: &Meta, key: &str) -> bool {
    matches!(meta, Meta::Path(path) if path.is_ident(key))
}

pub fn lower_camel_case(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut uppercase_next = false;
    for character in name.chars() {
        if character == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            output.extend(character.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(character);
        }
    }
    output
}
