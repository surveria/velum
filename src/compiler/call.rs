use crate::{
    api::native_call::NativeCallTarget,
    ast::{Expr, Expression, StaticCallSiteId},
    error::{Error, Result},
    value::{ErrorName, Value},
};

use super::{BytecodeCallSite, BytecodeCompiler, BytecodeInstruction, has_spread_arg};

impl BytecodeCompiler<'_> {
    pub(super) fn compile_call_expr(
        &mut self,
        callee: &Expression,
        site: StaticCallSiteId,
        args: &[Expression],
    ) -> Result<()> {
        if has_spread_arg(args) {
            return self.compile_spread_call_expr(callee, args);
        }
        if let Some(expected) = assert_throws_expected_error(callee, args)? {
            let callback = args
                .get(1)
                .ok_or_else(|| Error::runtime("assert.throws requires a callback"))?;
            self.compile_expr(callback)?;
            if let Some(message) = args.get(2) {
                self.compile_expr(message)?;
            }
            if args.get(3).is_some() {
                return Err(Error::runtime(
                    "assert.throws supports at most three arguments",
                ));
            }
            self.emit(BytecodeInstruction::AssertThrows {
                expected,
                has_message: args.get(2).is_some(),
            });
            return Ok(());
        }

        match callee.kind() {
            Expr::Identifier(name) if name.as_str() == "print" => {
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::Print {
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::Identifier(name) => {
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallBinding {
                    callee: self.compile_binding(name)?,
                    native: NativeCallTarget::from_binding_name(name.as_str()),
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
            Expr::SuperMember { property, access } => {
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallSuperMember {
                    property: Self::compile_property(property, *access),
                    arg_count: args.len(),
                });
                Ok(())
            }
            Expr::Parenthesized(callee) => self.compile_call_expr(callee, site, args),
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

    fn compile_spread_call_expr(&mut self, callee: &Expression, args: &[Expression]) -> Result<()> {
        match callee.kind() {
            Expr::Identifier(name) => {
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallBindingSpread {
                    callee: self.compile_binding(name)?,
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
            Expr::SuperMember { property, access } => {
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallSuperMemberSpread {
                    property: Self::compile_property(property, *access),
                });
                Ok(())
            }
            Expr::Parenthesized(callee) => self.compile_spread_call_expr(callee, args),
            _ => {
                self.compile_expr(callee)?;
                let spread_flags = self.compile_spread_parts(args)?;
                self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
                self.emit(BytecodeInstruction::CallValueSpread);
                Ok(())
            }
        }
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
        Expr::StringLiteral(value) => NativeCallTarget::from_property_name(value.as_str()),
        Expr::Literal(
            value @ (Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_)),
        ) => NativeCallTarget::from_property_name(&value.to_string()),
        _ => None,
    }
}

fn assert_throws_expected_error(
    callee: &Expression,
    args: &[Expression],
) -> Result<Option<ErrorName>> {
    let Expr::Member {
        object, property, ..
    } = callee.kind()
    else {
        return Ok(None);
    };
    if !matches!(object.kind(), Expr::Identifier(name) if name.as_str() == "assert")
        || property.as_str() != "throws"
    {
        return Ok(None);
    }
    let Some(expected) = args.first() else {
        return Err(Error::runtime("assert.throws requires an expected error"));
    };
    let Expr::Identifier(name) = expected.kind() else {
        return Err(Error::runtime(
            "assert.throws first argument must be an error constructor",
        ));
    };
    ErrorName::from_constructor_name(name)
        .ok_or_else(|| {
            Error::runtime(format!(
                "assert.throws error constructor '{name}' is not supported"
            ))
        })
        .map(Some)
}
