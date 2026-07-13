mod builder;
mod layout;
mod scope_rules;
mod upvalues;

use builder::LayoutBuilder;
pub use scope_rules::{for_init_needs_lexical_scope, for_init_needs_per_iteration_scope};

pub enum RootLayoutMode {
    Script,
    SloppyEval,
    StrictEval,
}
