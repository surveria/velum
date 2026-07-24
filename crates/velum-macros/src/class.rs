use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, ItemStruct, Result, spanned::Spanned};

use crate::parse::{lower_camel_case, metadata, path_flag, string_value, take_js_metadata};

#[derive(Clone, Copy)]
enum RenameRule {
    Preserve,
    LowerCamel,
}

struct ClassOptions {
    name: String,
    rename_rule: RenameRule,
}

impl ClassOptions {
    fn parse(tokens: TokenStream) -> Result<Self> {
        let mut name = None;
        let mut rename_rule = RenameRule::Preserve;
        for meta in metadata(tokens)? {
            if let Some(value) = string_value(&meta, "name")? {
                name = Some(value);
            } else if let Some(value) = string_value(&meta, "rename_all")? {
                rename_rule = match value.as_str() {
                    "camelCase" => RenameRule::LowerCamel,
                    _ => {
                        return Err(syn::Error::new(
                            meta.span(),
                            "rename_all currently supports only \"camelCase\"",
                        ));
                    }
                };
            } else {
                return Err(syn::Error::new(
                    meta.span(),
                    "unsupported host_class option",
                ));
            }
        }
        let Some(name) = name else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "host_class requires name = \"JavaScriptName\"",
            ));
        };
        if name.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "host class name must not be empty",
            ));
        }
        Ok(Self { name, rename_rule })
    }

    fn field_name(&self, rust_name: &str) -> String {
        match self.rename_rule {
            RenameRule::Preserve => rust_name.to_owned(),
            RenameRule::LowerCamel => lower_camel_case(rust_name),
        }
    }
}

pub fn expand(attributes: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let options = ClassOptions::parse(attributes)?;
    let mut item: ItemStruct = syn::parse2(item)?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "generic host classes are not supported yet",
        ));
    }
    let Fields::Named(fields) = &mut item.fields else {
        return Err(syn::Error::new(
            item.fields.span(),
            "host_class requires a struct with named fields",
        ));
    };

    let mut getters = Vec::new();
    for field in &mut fields.named {
        let metadata = take_js_metadata(&mut field.attrs)?;
        if metadata.is_empty() {
            continue;
        }
        let mut exported = false;
        let mut skipped = false;
        let mut explicit_name = None;
        for meta in metadata {
            if path_flag(&meta, "get") {
                exported = true;
            } else if path_flag(&meta, "skip") {
                skipped = true;
            } else if let Some(value) = string_value(&meta, "name")? {
                explicit_name = Some(value);
            } else {
                return Err(syn::Error::new(
                    meta.span(),
                    "unsupported field #[js] option",
                ));
            }
        }
        if exported && skipped {
            return Err(syn::Error::new(
                field.span(),
                "a host-class field cannot be both exported and skipped",
            ));
        }
        if skipped {
            continue;
        }
        if !exported {
            return Err(syn::Error::new(
                field.span(),
                "field #[js] requires get or skip",
            ));
        }
        let Some(identifier) = &field.ident else {
            return Err(syn::Error::new(
                field.span(),
                "host-class field has no name",
            ));
        };
        let rust_name = identifier.to_string();
        let js_name = explicit_name.unwrap_or_else(|| options.field_name(&rust_name));
        getters.push(quote! {
            let class = class.getter(#js_name, |payload, _call| {
                ::core::result::Result::Ok(payload.#identifier.clone())
            });
        });
    }

    let identifier = &item.ident;
    let js_name = options.name;
    let lower_camel_case_members = matches!(options.rename_rule, RenameRule::LowerCamel);
    Ok(quote! {
        #item

        impl ::velum::HostClassMetadata for #identifier {
            const JS_CLASS_NAME: &'static str = #js_name;
            const LOWER_CAMEL_CASE_MEMBERS: bool = #lower_camel_case_members;

            fn install_fields(
                class: ::velum::HostClass<::std::sync::Arc<Self>>,
            ) -> ::velum::HostClass<::std::sync::Arc<Self>> {
                #(#getters)*
                class
            }
        }
    })
}
