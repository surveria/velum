use crate::{
    error::Result, lexer::TokenStream, runtime::limits::RuntimeLimits, syntax::StaticName,
};

use super::{ClassPrivateScope, ParsedProgram, Parser, SuperContext};

#[derive(Clone, Copy)]
pub enum EvalSuperContext {
    None,
    Property,
    PropertyAndCall,
}

impl EvalSuperContext {
    const fn allows_property(self) -> bool {
        matches!(self, Self::Property | Self::PropertyAndCall)
    }

    const fn allows_call(self) -> bool {
        matches!(self, Self::PropertyAndCall)
    }
}

#[derive(Clone, Copy)]
pub enum EvalClassFieldContext {
    None,
    Initializer,
}

impl EvalClassFieldContext {
    const fn is_initializer(self) -> bool {
        matches!(self, Self::Initializer)
    }
}

#[derive(Clone, Copy)]
pub struct EvalParseContext<'a> {
    strict_mode: bool,
    super_context: EvalSuperContext,
    class_field_context: EvalClassFieldContext,
    private_names: &'a [StaticName],
}

impl<'a> EvalParseContext<'a> {
    pub const fn new(
        strict_mode: bool,
        super_context: EvalSuperContext,
        class_field_context: EvalClassFieldContext,
        private_names: &'a [StaticName],
    ) -> Self {
        Self {
            strict_mode,
            super_context,
            class_field_context,
            private_names,
        }
    }
}

pub fn parse_eval_with_usage_in_context(
    tokens: TokenStream,
    limits: RuntimeLimits,
    context: EvalParseContext<'_>,
) -> Result<ParsedProgram> {
    let mut parser = Parser::new(tokens, limits, context.strict_mode);
    parser.super_context = SuperContext::new(
        context.super_context.allows_property(),
        context.super_context.allows_call(),
    );
    parser.new_target_scope_depth = usize::from(context.class_field_context.is_initializer());
    parser.reject_all_arguments = context.class_field_context.is_initializer();
    if !context.private_names.is_empty() {
        parser
            .class_private_scopes
            .push(ClassPrivateScope::external(context.private_names));
    }
    parser.parse()
}
