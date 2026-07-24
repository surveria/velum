use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    FnArg, ImplItem, ImplItemFn, ItemImpl, Pat, PatIdent, Result, Signature, Type, spanned::Spanned,
};

use crate::parse::{metadata, path_flag, string_value, take_js_metadata};

#[derive(Clone, Copy, Eq, PartialEq)]
enum MethodKind {
    Constructor,
    Method,
    Getter,
    Setter,
    StaticMethod,
}

struct MethodOptions {
    kind: MethodKind,
    name: Option<String>,
    raw: bool,
}

impl MethodOptions {
    fn parse(method: &mut ImplItemFn) -> Result<Option<Self>> {
        let metadata = take_js_metadata(&mut method.attrs)?;
        if metadata.is_empty() {
            return Ok(None);
        }
        let mut kind = None;
        let mut name = None;
        let mut raw = false;
        for meta in metadata {
            let candidate = if path_flag(&meta, "constructor") {
                Some(MethodKind::Constructor)
            } else if path_flag(&meta, "method") {
                Some(MethodKind::Method)
            } else if path_flag(&meta, "getter") {
                Some(MethodKind::Getter)
            } else if path_flag(&meta, "setter") {
                Some(MethodKind::Setter)
            } else if path_flag(&meta, "static_method") {
                Some(MethodKind::StaticMethod)
            } else {
                None
            };
            if let Some(candidate) = candidate {
                if kind.replace(candidate).is_some() {
                    return Err(syn::Error::new(
                        meta.span(),
                        "a method may have only one JavaScript export kind",
                    ));
                }
            } else if let Some(value) = string_value(&meta, "name")? {
                name = Some(value);
            } else if path_flag(&meta, "raw") {
                raw = true;
            } else {
                return Err(syn::Error::new(
                    meta.span(),
                    "unsupported method #[js] option",
                ));
            }
        }
        let Some(kind) = kind else {
            return Err(syn::Error::new(
                method.sig.span(),
                "method #[js] requires an export kind",
            ));
        };
        if raw && kind != MethodKind::Method {
            return Err(syn::Error::new(
                method.sig.span(),
                "raw is supported only for synchronous prototype methods",
            ));
        }
        Ok(Some(Self { kind, name, raw }))
    }
}

struct Arguments {
    declarations: Vec<TokenStream>,
    values: Vec<TokenStream>,
    js_count: usize,
}

impl Arguments {
    fn parse(signature: &Signature, skip_receiver: bool) -> Result<Self> {
        let mut declarations = Vec::new();
        let mut values = Vec::new();
        let mut js_count = 0_usize;
        for (position, argument) in signature.inputs.iter().enumerate() {
            if skip_receiver && position == 0 && matches!(argument, FnArg::Receiver(_)) {
                continue;
            }
            let FnArg::Typed(argument) = argument else {
                return Err(syn::Error::new(
                    argument.span(),
                    "unexpected method receiver",
                ));
            };
            let Pat::Ident(PatIdent {
                ident,
                by_ref: None,
                subpat: None,
                ..
            }) = argument.pat.as_ref()
            else {
                return Err(syn::Error::new(
                    argument.pat.span(),
                    "exported arguments must use simple identifier patterns",
                ));
            };
            let argument_type = argument.ty.as_ref();
            if is_type_named(argument_type, "HostCall") {
                if signature.asyncness.is_some() {
                    return Err(syn::Error::new(
                        argument_type.span(),
                        "async methods cannot retain HostCall across await",
                    ));
                }
                declarations.push(quote! {
                    let #ident: #argument_type = call;
                });
            } else if is_type_named(argument_type, "HostAsyncContext") {
                if signature.asyncness.is_none() {
                    return Err(syn::Error::new(
                        argument_type.span(),
                        "HostAsyncContext is available only to async methods",
                    ));
                }
                declarations.push(quote! {
                    let #ident: #argument_type = call.async_context()?;
                });
            } else {
                if signature.asyncness.is_some() && matches!(argument_type, Type::Reference(_)) {
                    return Err(syn::Error::new(
                        argument_type.span(),
                        "async JavaScript arguments must be owned",
                    ));
                }
                let index = js_count;
                let label = ident.to_string();
                declarations.push(quote! {
                    let #ident: #argument_type = call.argument(#index, #label)?;
                });
                js_count = js_count
                    .checked_add(1)
                    .ok_or_else(|| syn::Error::new(argument.span(), "argument count overflowed"))?;
            }
            values.push(quote! { #ident });
        }
        Ok(Self {
            declarations,
            values,
            js_count,
        })
    }
}

pub fn expand(attributes: TokenStream, item: TokenStream) -> Result<TokenStream> {
    if !metadata(attributes)?.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "host_methods does not accept options",
        ));
    }
    let mut item: ItemImpl = syn::parse2(item)?;
    if item.trait_.is_some() {
        return Err(syn::Error::new(
            item.span(),
            "host_methods requires an inherent impl block",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "generic host method impls are not supported yet",
        ));
    }

    let mut constructor = None;
    let mut installers = Vec::new();
    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        let Some(options) = MethodOptions::parse(method)? else {
            continue;
        };
        if options.kind == MethodKind::Constructor {
            if constructor.is_some() {
                return Err(syn::Error::new(
                    method.sig.span(),
                    "host class has more than one constructor",
                ));
            }
            constructor = Some(constructor_tokens(&item.self_ty, method)?);
        } else {
            installers.push(method_tokens(&item.self_ty, method, &options)?);
        }
    }
    let Some(constructor) = constructor else {
        return Err(syn::Error::new(
            item.span(),
            "host class requires one #[js(constructor)] method",
        ));
    };
    let self_type = &item.self_ty;
    Ok(quote! {
        #item

        impl ::velum::HostClassDefinition for #self_type {
            type Payload = ::std::sync::Arc<#self_type>;

            fn host_class() -> ::velum::HostClass<Self::Payload> {
                #constructor
                let class =
                    <#self_type as ::velum::HostClassMetadata>::install_fields(class);
                #(#installers)*
                class
            }
        }
    })
}

fn constructor_tokens(self_type: &Type, method: &ImplItemFn) -> Result<TokenStream> {
    ensure_associated(&method.sig, "constructor")?;
    if method.sig.asyncness.is_some() {
        return Err(syn::Error::new(
            method.sig.span(),
            "a host-class constructor cannot be async",
        ));
    }
    let arguments = Arguments::parse(&method.sig, false)?;
    let declarations = arguments.declarations;
    let values = arguments.values;
    let length = u16::try_from(arguments.js_count)
        .map_err(|_| syn::Error::new(method.sig.span(), "constructor has too many arguments"))?;
    let identifier = &method.sig.ident;
    Ok(quote! {
        let class = ::velum::HostClass::new(
            <#self_type as ::velum::HostClassMetadata>::JS_CLASS_NAME,
            |call| {
                #(#declarations)*
                let instance = <#self_type>::#identifier(#(#values),*)?;
                instance.into_shared()
            },
        )
        .with_constructor_length(#length);
    })
}

fn method_tokens(
    self_type: &Type,
    method: &ImplItemFn,
    options: &MethodOptions,
) -> Result<TokenStream> {
    let identifier = &method.sig.ident;
    let js_name = options.name.as_ref().map_or_else(
        || {
            let rust_name = identifier.to_string();
            quote! {
                <#self_type as ::velum::HostClassMetadata>::member_name(#rust_name)
            }
        },
        |name| quote! { #name },
    );
    let has_receiver = matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)));
    let needs_receiver = matches!(
        options.kind,
        MethodKind::Method | MethodKind::Getter | MethodKind::Setter
    );
    if needs_receiver {
        ensure_shared_receiver(&method.sig)?;
    } else {
        ensure_associated(&method.sig, "static method")?;
    }
    let arguments = Arguments::parse(&method.sig, has_receiver)?;
    validate_accessor_arguments(options.kind, &method.sig, arguments.js_count)?;
    let declarations = arguments.declarations;
    let values = arguments.values;
    let length = u16::try_from(arguments.js_count)
        .map_err(|_| syn::Error::new(method.sig.span(), "method has too many arguments"))?;

    if method.sig.asyncness.is_some() {
        if options.raw {
            return Err(syn::Error::new(
                method.sig.span(),
                "raw is supported only for synchronous prototype methods",
            ));
        }
        return async_method_tokens(
            self_type,
            identifier,
            &js_name,
            options.kind,
            &declarations,
            &values,
            length,
        );
    }
    let invocation = if needs_receiver {
        quote! { <#self_type>::#identifier(payload.as_ref(), #(#values),*) }
    } else {
        quote! { <#self_type>::#identifier(#(#values),*) }
    };
    let installer = match options.kind {
        MethodKind::Method if options.raw => quote! {
            let class = class.method_with_result(#js_name, #length, |payload, call| {
                #(#declarations)*
                #invocation
            });
        },
        MethodKind::Method => quote! {
            let class = class.method_with_length(#js_name, #length, |payload, call| {
                #(#declarations)*
                #invocation
            });
        },
        MethodKind::Getter => quote! {
            let class = class.getter(#js_name, |payload, call| {
                #(#declarations)*
                #invocation
            });
        },
        MethodKind::Setter => quote! {
            let class = class.setter(#js_name, |payload, call| {
                #(#declarations)*
                #invocation
            });
        },
        MethodKind::StaticMethod => quote! {
            let class = class.static_method(#js_name, #length, |call| {
                #(#declarations)*
                #invocation
            });
        },
        MethodKind::Constructor => {
            return Err(syn::Error::new(
                method.sig.span(),
                "constructor was routed as a regular method",
            ));
        }
    };
    Ok(installer)
}

fn validate_accessor_arguments(
    kind: MethodKind,
    signature: &Signature,
    argument_count: usize,
) -> Result<()> {
    let message = match kind {
        MethodKind::Getter if argument_count != 0 => {
            Some("a JavaScript getter cannot accept value arguments")
        }
        MethodKind::Setter if argument_count != 1 => {
            Some("a JavaScript setter requires exactly one value argument")
        }
        _ => None,
    };
    if let Some(message) = message {
        return Err(syn::Error::new(signature.span(), message));
    }
    Ok(())
}

fn async_method_tokens(
    self_type: &Type,
    identifier: &syn::Ident,
    js_name: &TokenStream,
    kind: MethodKind,
    declarations: &[TokenStream],
    values: &[TokenStream],
    length: u16,
) -> Result<TokenStream> {
    match kind {
        MethodKind::Method => Ok(quote! {
            let class = class.async_method(#js_name, #length, |payload, call| {
                #(#declarations)*
                let payload = ::std::sync::Arc::clone(payload);
                ::core::result::Result::Ok(async move {
                    <#self_type>::#identifier(payload.as_ref(), #(#values),*).await
                })
            });
        }),
        MethodKind::StaticMethod => Ok(quote! {
            let class = class.static_async_method(#js_name, #length, |call| {
                #(#declarations)*
                ::core::result::Result::Ok(async move {
                    <#self_type>::#identifier(#(#values),*).await
                })
            });
        }),
        MethodKind::Getter | MethodKind::Setter => Err(syn::Error::new(
            identifier.span(),
            "async accessors are not supported; export an async method",
        )),
        MethodKind::Constructor => Err(syn::Error::new(
            identifier.span(),
            "a host-class constructor cannot be async",
        )),
    }
}

fn ensure_shared_receiver(signature: &Signature) -> Result<()> {
    let Some(FnArg::Receiver(receiver)) = signature.inputs.first() else {
        return Err(syn::Error::new(
            signature.span(),
            "prototype methods require an &self receiver",
        ));
    };
    if receiver.reference.is_none() || receiver.mutability.is_some() {
        return Err(syn::Error::new(
            receiver.span(),
            "prototype methods require an immutable &self receiver",
        ));
    }
    Ok(())
}

fn ensure_associated(signature: &Signature, label: &str) -> Result<()> {
    if matches!(signature.inputs.first(), Some(FnArg::Receiver(_))) {
        return Err(syn::Error::new(
            signature.span(),
            format!("{label} must not have a self receiver"),
        ));
    }
    Ok(())
}

fn is_type_named(argument_type: &Type, name: &str) -> bool {
    let Type::Path(path) = argument_type else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}
