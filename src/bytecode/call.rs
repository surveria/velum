use crate::{
    api::native_call::NativeCallTarget,
    ast::Expr,
    error::{Error, Result},
    value::{ErrorName, Value},
};

use super::{BytecodeCompiler, BytecodeInstruction};

impl BytecodeCompiler<'_> {
    pub(super) fn compile_call_expr(&mut self, callee: &Expr, args: &[Expr]) -> Result<()> {
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

        match callee {
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
            Expr::Parenthesized(callee) => self.compile_call_expr(callee, args),
            callee => {
                self.compile_expr(callee)?;
                self.compile_args(args)?;
                self.emit(BytecodeInstruction::CallValue {
                    arg_count: args.len(),
                });
                Ok(())
            }
        }
    }

    pub(super) fn compile_args(&mut self, args: &[Expr]) -> Result<()> {
        for arg in args {
            self.compile_expr(arg)?;
        }
        Ok(())
    }
}

fn computed_property_native_target(property: &Expr) -> Option<NativeCallTarget> {
    match property {
        Expr::StringLiteral(value) => NativeCallTarget::from_property_name(value.as_str()),
        Expr::Literal(
            value @ (Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_)),
        ) => NativeCallTarget::from_property_name(&value.to_string()),
        _ => None,
    }
}

fn assert_throws_expected_error(callee: &Expr, args: &[Expr]) -> Result<Option<ErrorName>> {
    let Expr::Member {
        object, property, ..
    } = callee
    else {
        return Ok(None);
    };
    if !matches!(object.as_ref(), Expr::Identifier(name) if name.as_str() == "assert")
        || property.as_str() != "throws"
    {
        return Ok(None);
    }
    let Some(expected) = args.first() else {
        return Err(Error::runtime("assert.throws requires an expected error"));
    };
    let Expr::Identifier(name) = expected else {
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
