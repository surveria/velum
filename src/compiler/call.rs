use crate::{
    api::native_call::NativeCallTarget,
    ast::{Expr, Expression, StaticCallSiteId},
    error::Result,
    value::Value,
};

use super::{
    BytecodeCallSite, BytecodeCompiler, BytecodeInstruction, InstructionIndex, has_spread_arg,
};

#[derive(Clone, Copy)]
struct OptionalCallTarget {
    base_nullish_jump: Option<InstructionIndex>,
    has_receiver: bool,
}

impl BytecodeCompiler<'_> {
    pub(super) fn compile_tail_call_expr(&mut self, expr: &Expression) -> Result<bool> {
        let Expr::Call {
            callee,
            strict: true,
            args,
            ..
        } = expr.kind()
        else {
            return Ok(false);
        };
        if has_spread_arg(args) {
            return Ok(false);
        }
        match callee.kind() {
            Expr::Identifier(name) => {
                let native = NativeCallTarget::from_binding_name(name.as_str());
                if native == Some(NativeCallTarget::Eval) {
                    return Ok(false);
                }
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::TailCallBinding {
                    callee: self.compile_binding(name)?,
                    native,
                    strict: true,
                    arg_count: args.len(),
                });
            }
            Expr::Parenthesized(callee) => {
                let nested = Expression::new(
                    Expr::Call {
                        callee: callee.clone(),
                        site: match expr.kind() {
                            Expr::Call { site, .. } => *site,
                            _ => return Ok(false),
                        },
                        strict: true,
                        args: args.clone(),
                    },
                    expr.span(),
                );
                return self.compile_tail_call_expr(&nested);
            }
            Expr::Member { .. }
            | Expr::ComputedMember { .. }
            | Expr::PrivateMember { .. }
            | Expr::SuperMember { .. }
            | Expr::SuperComputedMember { .. } => return Ok(false),
            _ => {
                self.compile_expr(callee)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::TailCallValue {
                    arg_count: args.len(),
                });
            }
        }
        Ok(true)
    }

    pub(super) fn compile_call_expr(
        &mut self,
        callee: &Expression,
        site: StaticCallSiteId,
        strict: bool,
        args: &[Expression],
    ) -> Result<()> {
        if let Some(result) = self.compile_wrapped_call(callee, site, strict, args) {
            return result;
        }
        if has_spread_arg(args) {
            return self.compile_spread_call_expr(callee, strict, args);
        }
        match callee.kind() {
            Expr::Identifier(name) => {
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallBinding {
                    callee: self.compile_binding(name)?,
                    native: NativeCallTarget::from_binding_name(name.as_str()),
                    strict,
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallStaticMember {
                    property: Self::compile_property(property, *access),
                    native: NativeCallTarget::from_property_name(property.as_str()),
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::OptionalMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                let nullish_jump = self.emit_jump_if_nullish_keep();
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallStaticMember {
                    property: Self::compile_property(property, *access),
                    native: NativeCallTarget::from_property_name(property.as_str()),
                    arg_count: args.len(),
                });
                self.finish_optional_call(nullish_jump)
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallComputedMember {
                    property: Self::compile_dynamic_property(*access),
                    native: computed_property_native_target(property),
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::PrivateMember { object, name } => {
                self.compile_expr(object)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallPrivateMember {
                    property: crate::bytecode::BytecodePrivateName::new(name.clone()),
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::SuperMember { property, access } => {
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallSuperMember {
                    property: Self::compile_property(property, *access),
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::SuperComputedMember { property, access } => {
                self.compile_expr(property)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallComputedSuperMember {
                    property: Self::compile_dynamic_property(*access),
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::Parenthesized(callee) => self.compile_call_expr(callee, site, strict, args),
            _ => {
                self.compile_expr(callee)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallValue {
                    site: BytecodeCallSite::new(site),
                    arg_count: args.len(),
                });
                Ok(())
            }
        }
    }

    fn compile_wrapped_call(
        &mut self,
        callee: &Expression,
        site: StaticCallSiteId,
        strict: bool,
        args: &[Expression],
    ) -> Option<Result<()>> {
        match callee.kind() {
            Expr::OptionalChain(expression) => {
                Some(self.compile_parenthesized_optional_chain_call(expression, site, args))
            }
            Expr::Parenthesized(callee) => Some(self.compile_call_expr(callee, site, strict, args)),
            _ => None,
        }
    }

    pub(super) fn compile_optional_call_expr(
        &mut self,
        callee: &Expression,
        site: StaticCallSiteId,
        args: &[Expression],
    ) -> Result<()> {
        let target = self.compile_optional_call_target(callee)?;
        let callee_nullish_jump = self.emit_jump_if_nullish_keep();
        if has_spread_arg(args) {
            let spread_flags = self.compile_spread_parts(args)?;
            self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
            self.emit(if target.has_receiver {
                BytecodeInstruction::CallValueWithReceiverSpread
            } else {
                BytecodeInstruction::CallValueSpread
            });
        } else {
            self.compile_args(args)?;
            self.emit(if target.has_receiver {
                BytecodeInstruction::CallValueWithReceiver {
                    site: BytecodeCallSite::new(site),
                    arg_count: args.len(),
                }
            } else {
                BytecodeInstruction::CallValue {
                    site: BytecodeCallSite::new(site),
                    arg_count: args.len(),
                }
            });
        }
        self.finish_optional_invocation(target, callee_nullish_jump)
    }

    fn compile_optional_call_target(&mut self, callee: &Expression) -> Result<OptionalCallTarget> {
        let mut base_nullish_jump = None;
        let has_receiver = match callee.kind() {
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_optional_static_call_target(object, property, *access, false)?;
                true
            }
            Expr::OptionalMember {
                object,
                property,
                access,
            } => {
                base_nullish_jump =
                    self.compile_optional_static_call_target(object, property, *access, true)?;
                true
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::Duplicate);
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember {
                    property: Self::compile_dynamic_property(*access),
                });
                true
            }
            Expr::OptionalComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                base_nullish_jump = Some(self.emit_jump_if_nullish_keep());
                self.emit(BytecodeInstruction::Duplicate);
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::ComputedMember {
                    property: Self::compile_dynamic_property(*access),
                });
                true
            }
            Expr::PrivateMember { object, name } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::Duplicate);
                self.emit(BytecodeInstruction::PrivateMember {
                    property: crate::bytecode::BytecodePrivateName::new(name.clone()),
                });
                true
            }
            Expr::OptionalPrivateMember { object, name } => {
                self.compile_expr(object)?;
                base_nullish_jump = Some(self.emit_jump_if_nullish_keep());
                self.emit(BytecodeInstruction::Duplicate);
                self.emit(BytecodeInstruction::PrivateMember {
                    property: crate::bytecode::BytecodePrivateName::new(name.clone()),
                });
                true
            }
            Expr::SuperMember { property, access } => {
                self.emit(BytecodeInstruction::LoadThis);
                self.emit(BytecodeInstruction::SuperMember {
                    property: Self::compile_property(property, *access),
                });
                true
            }
            Expr::SuperComputedMember { property, access } => {
                self.emit(BytecodeInstruction::LoadThis);
                self.emit(BytecodeInstruction::ComputedSuperMember {
                    expression: crate::bytecode::BytecodeBlock::compile_expression(
                        property,
                        self.layout,
                    )?,
                    property: Self::compile_dynamic_property(*access),
                });
                true
            }
            Expr::Parenthesized(callee) => {
                return self.compile_optional_call_target(callee);
            }
            _ => {
                self.compile_expr(callee)?;
                false
            }
        };
        Ok(OptionalCallTarget {
            base_nullish_jump,
            has_receiver,
        })
    }

    fn compile_optional_static_call_target(
        &mut self,
        object: &Expression,
        property: &crate::ast::StaticName,
        access: crate::ast::StaticPropertyAccessId,
        optional: bool,
    ) -> Result<Option<InstructionIndex>> {
        self.compile_expr(object)?;
        let nullish_jump = optional.then(|| self.emit_jump_if_nullish_keep());
        self.emit(BytecodeInstruction::Duplicate);
        self.emit(BytecodeInstruction::StaticMember {
            property: Self::compile_property(property, access),
        });
        Ok(nullish_jump)
    }

    fn finish_optional_invocation(
        &mut self,
        target: OptionalCallTarget,
        callee_nullish_jump: InstructionIndex,
    ) -> Result<()> {
        let end_jump = self.emit_jump();
        let callee_nullish_address = self.current_address();
        self.patch_jump(callee_nullish_jump, callee_nullish_address)?;
        self.emit(BytecodeInstruction::Pop);
        if target.has_receiver {
            self.emit(BytecodeInstruction::Pop);
        }
        self.emit(BytecodeInstruction::PushUndefined);
        let callee_branch_end = target.base_nullish_jump.map(|_| self.emit_jump());
        if let Some(base_nullish_jump) = target.base_nullish_jump {
            let base_nullish_address = self.current_address();
            self.patch_jump(base_nullish_jump, base_nullish_address)?;
            self.emit(BytecodeInstruction::Pop);
            self.emit(BytecodeInstruction::PushUndefined);
        }
        let end_address = self.current_address();
        self.patch_jump(end_jump, end_address)?;
        if let Some(callee_branch_end) = callee_branch_end {
            self.patch_jump(callee_branch_end, end_address)?;
        }
        Ok(())
    }

    fn compile_spread_call_expr(
        &mut self,
        callee: &Expression,
        strict: bool,
        args: &[Expression],
    ) -> Result<()> {
        match callee.kind() {
            Expr::Identifier(name) => {
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallBindingSpread {
                    callee: self.compile_binding(name)?,
                    native: NativeCallTarget::from_binding_name(name.as_str()),
                    strict,
                });
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallStaticMemberSpread {
                    property: Self::compile_property(property, *access),
                });
                Ok(())
            }
            Expr::OptionalMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                let nullish_jump = self.emit_jump_if_nullish_keep();
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallStaticMemberSpread {
                    property: Self::compile_property(property, *access),
                });
                self.finish_optional_call(nullish_jump)
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallComputedMemberSpread {
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(())
            }
            Expr::PrivateMember { object, name } => {
                self.compile_expr(object)?;
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallPrivateMemberSpread {
                    property: crate::bytecode::BytecodePrivateName::new(name.clone()),
                });
                Ok(())
            }
            Expr::SuperMember { property, access } => {
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallSuperMemberSpread {
                    property: Self::compile_property(property, *access),
                });
                Ok(())
            }
            Expr::SuperComputedMember { property, access } => {
                self.compile_expr(property)?;
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallComputedSuperMemberSpread {
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(())
            }
            Expr::Parenthesized(callee) => self.compile_spread_call_expr(callee, strict, args),
            _ => {
                self.compile_expr(callee)?;
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallValueSpread);
                Ok(())
            }
        }
    }

    fn finish_optional_call(&mut self, nullish_jump: InstructionIndex) -> Result<()> {
        let end_jump = self.emit_jump();
        let nullish_address = self.current_address();
        self.patch_jump(nullish_jump, nullish_address)?;
        self.emit(BytecodeInstruction::Pop);
        self.emit(BytecodeInstruction::PushUndefined);
        let end_address = self.current_address();
        self.patch_jump(end_jump, end_address)
    }

    pub(super) fn compile_args(&mut self, args: &[Expression]) -> Result<()> {
        for arg in args {
            self.compile_expr(arg)?;
        }
        Ok(())
    }
}

fn computed_property_native_target(property: &Expression) -> Option<NativeCallTarget> {
    match property.kind() {
        Expr::StringLiteral { value, .. } => NativeCallTarget::from_property_name(value.as_str()),
        Expr::Literal(
            value @ (Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_)),
        ) => NativeCallTarget::from_property_name(&value.to_string()),
        _ => None,
    }
}
