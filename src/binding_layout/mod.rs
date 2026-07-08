mod builder;
mod layout;
mod metadata;
mod scope_rules;
pub mod types;
mod upvalues;

use builder::LayoutBuilder;

pub use layout::BindingLayout;
pub use types::{BindingOperand, DeclarationRef, FunctionScopeId, LocalSlot, ScopeId, UpvalueSlot};
