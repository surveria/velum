#![deny(unsafe_code)]

use proc_macro::TokenStream;

mod class;
mod methods;
mod parse;

/// Marks a named Rust struct as the payload of a JavaScript host class.
///
/// `name` selects the JavaScript constructor name. The optional
/// `rename_all = "camelCase"` rule applies to exported fields and methods.
/// Struct fields remain Rust-only unless marked with `#[js(get)]`.
#[proc_macro_attribute]
pub fn host_class(attributes: TokenStream, item: TokenStream) -> TokenStream {
    class::expand(attributes.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Exports explicitly annotated inherent methods for a host class.
///
/// Supported markers are `constructor`, `method`, `getter`, `setter`, and
/// `static_method`. Rust `async fn` methods automatically use Velum's host
/// future and JavaScript Promise bridge.
#[proc_macro_attribute]
pub fn host_methods(attributes: TokenStream, item: TokenStream) -> TokenStream {
    methods::expand(attributes.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
