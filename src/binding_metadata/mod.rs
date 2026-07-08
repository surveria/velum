mod layout;
mod metadata;
pub mod types;

pub use layout::{BindingLayout, BindingLayoutParts};
pub use types::{BindingOperand, DeclarationRef, FunctionScopeId, LocalSlot, ScopeId, UpvalueSlot};
