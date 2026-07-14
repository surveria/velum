pub mod declaration;
mod global_declaration;
pub mod location;
pub mod scope;
mod static_binding_lookup;
mod with_environment;
pub(in crate::runtime) use with_environment::WithBindingReference;
pub mod static_bindings;
