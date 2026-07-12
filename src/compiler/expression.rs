use std::rc::Rc;

use super::{
    BinaryOp, BytecodeBinding, BytecodeBlock, BytecodeCompiler, BytecodeInstruction,
    BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    BytecodeNumericUnaryOp, Error, Expr, Expression, NativeCallTarget, Result, StaticBinding,
    StaticPropertyAccessId, StaticString, UnaryOp, checked_template_part_count,
    constructor_binding_expr, has_spread_arg,
};

impl BytecodeCompiler<'_> {
    fn compile_super_call(&mut self, args: &[Expression]) -> Result<()> {
        if has_spread_arg(args) {
            let spread_flags = self.compile_spread_parts(args)?;
            self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
            self.emit(BytecodeInstruction::CallSuperSpread);
        } else {
            self.compile_args(args)?;
            self.emit(BytecodeInstruction::CallSuper {
                arg_count: args.len(),
            });
        }
        Ok(())
    }

    pub(super) fn compile_expr(&mut self, expr: &Expression) -> Result<()> {
        self.with_source_span(expr.span(), |compiler| {
            compiler.compile_expr_kind(expr.kind())
        })
    }

    fn compile_expr_kind(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Literal(value) => {
                self.emit(BytecodeInstruction::PushLiteral(value.clone()));
            }
            Expr::StringLiteral(value) => {
                self.emit(BytecodeInstruction::PushString(value.clone()));
            }
            Expr::TemplateLiteral {
                quasis,
                expressions,
            } => return self.compile_template_literal(quasis, expressions),
            Expr::RegExpLiteral { pattern, flags } => {
                self.emit(BytecodeInstruction::CreateRegExp {
                    pattern: pattern.clone(),
                    flags: flags.clone(),
                });
            }
            Expr::This => {
                self.emit(BytecodeInstruction::LoadThis);
            }
            Expr::NewTarget => {
                self.emit(BytecodeInstruction::LoadNewTarget);
            }
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::LoadBinding(
                    self.compile_binding(name)?,
                ));
            }
            Expr::Class(class) => return self.compile_class_literal(class),
            Expr::SuperCall { args } => return self.compile_super_call(args),
            Expr::SuperMember { property, access } => self.compile_super_member(property, *access),
            Expr::SuperComputedMember { property, access } => {
                return self.compile_super_computed_member(property, *access);
            }
            Expr::Spread(_) => return Err(Self::spread_outside_literal_error()),
            Expr::Parenthesized(expr) => return self.compile_expr(expr),
            Expr::Sequence(expressions) => return self.compile_sequence_expr(expressions),
            Expr::Await(_) | Expr::Yield { .. } => return self.compile_suspend_expr(expr),
            Expr::Unary { op, expr } => return self.compile_unary_expr(*op, expr),
            Expr::Binary {
                op,
                left,
                right,
                property_access,
            } => return self.compile_binary_expr(*op, left, right, *property_access),
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => return self.compile_conditional_expr(condition, consequent, alternate),
            Expr::Assignment { .. }
            | Expr::DestructuringAssignment { .. }
            | Expr::Update { .. }
            | Expr::CompoundAssignment { .. }
            | Expr::PropertyAssignment { .. }
            | Expr::ComputedPropertyAssignment { .. }
            | Expr::SuperPropertyAssignment { .. }
            | Expr::SuperComputedPropertyAssignment { .. } => {
                return self.compile_mutation_expr(expr);
            }
            Expr::Member {
                object,
                property,
                access,
            } => return self.compile_static_member_expr(object, property, *access),
            Expr::ComputedMember {
                object,
                property,
                access,
            } => return self.compile_computed_member_expr(object, property, *access),
            Expr::PrivateMember { .. }
            | Expr::PrivateAssignment { .. }
            | Expr::PrivateIn { .. } => return self.compile_private_expression(expr),
            Expr::Object(properties) => return self.compile_object_literal(properties),
            Expr::ArrayHole => return Err(Error::runtime("array elision outside array literal")),
            Expr::Array(elements) => return self.compile_array_literal(elements),
            Expr::Call {
                callee,
                site,
                strict,
                args,
            } => {
                return self.compile_call_expr(callee, *site, *strict, args);
            }
            Expr::Function { .. } | Expr::ArrowFunction { .. } | Expr::MethodFunction { .. } => {
                return self.compile_function_literal(expr);
            }
            Expr::New { constructor, args } => self.compile_new_expr(constructor, args)?,
        }
        Ok(())
    }

    fn compile_super_computed_member(
        &mut self,
        property: &Expression,
        access: crate::ast::StaticPropertyAccessId,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::ComputedSuperMember {
            expression: crate::bytecode::BytecodeBlock::compile_expression(property, self.layout)?,
            property: Self::compile_dynamic_property(access),
        });
        Ok(())
    }

    fn compile_super_member(
        &mut self,
        property: &crate::ast::StaticName,
        access: crate::ast::StaticPropertyAccessId,
    ) {
        self.emit(BytecodeInstruction::SuperMember {
            property: Self::compile_property(property, access),
        });
    }

    fn compile_suspend_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Await(expr) => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::Await);
            }
            Expr::Yield { expr, delegate } => {
                if let Some(expr) = expr {
                    self.compile_expr(expr)?;
                } else {
                    self.emit(BytecodeInstruction::PushUndefined);
                }
                self.emit(BytecodeInstruction::Yield {
                    delegate: *delegate,
                });
            }
            _ => return Err(Error::runtime("expected suspending expression")),
        }
        Ok(())
    }

    fn compile_mutation_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Assignment {
                name,
                strict,
                infer_name,
                expr,
            } => self.compile_binding_assignment_expr(name, *strict, *infer_name, expr),
            Expr::DestructuringAssignment {
                pattern,
                strict,
                expr,
            } => {
                self.compile_expr(expr)?;
                let pattern = self.compile_assignment_pattern(pattern, *strict)?;
                self.emit(BytecodeInstruction::DestructurePattern {
                    pattern: std::rc::Rc::new(pattern),
                    mode: crate::bytecode::BytecodeDestructureMode::Assignment,
                });
                Ok(())
            }
            Expr::Update {
                op,
                prefix,
                strict,
                expr,
            } => self.compile_update_expr(*op, *prefix, *strict, expr),
            Expr::CompoundAssignment {
                op,
                strict,
                target,
                expr,
            } => self.compile_compound_assignment(*op, *strict, target, expr),
            Expr::PropertyAssignment {
                object,
                property,
                access,
                strict,
                expr,
            } => self.compile_static_property_assignment(object, property, *access, *strict, expr),
            Expr::ComputedPropertyAssignment {
                object,
                property,
                access,
                strict,
                expr,
            } => {
                self.compile_computed_property_assignment(object, property, *access, *strict, expr)
            }
            Expr::SuperPropertyAssignment {
                property,
                access,
                strict,
                expr,
            } => self.compile_super_property_assignment(
                crate::bytecode::BytecodeSuperProperty::Static(Self::compile_property(
                    property, *access,
                )),
                *strict,
                expr,
            ),
            Expr::SuperComputedPropertyAssignment {
                property,
                access,
                strict,
                expr,
            } => self.compile_super_property_assignment(
                crate::bytecode::BytecodeSuperProperty::Computed {
                    expression: crate::bytecode::BytecodeBlock::compile_expression(
                        property,
                        self.layout,
                    )?,
                    operand: Self::compile_dynamic_property(*access),
                },
                *strict,
                expr,
            ),
            _ => Err(Error::runtime("expected mutation expression")),
        }
    }

    fn compile_sequence_expr(&mut self, expressions: &[Expression]) -> Result<()> {
        let Some((last, leading)) = expressions.split_last() else {
            return Err(Error::runtime("sequence expression cannot be empty"));
        };
        for expression in leading {
            self.compile_expr(expression)?;
            self.emit(BytecodeInstruction::Pop);
        }
        self.compile_expr(last)
    }

    fn spread_outside_literal_error() -> Error {
        Error::runtime("spread is only valid in call arguments and literals")
    }

    fn compile_template_literal(
        &mut self,
        quasis: &[StaticString],
        expressions: &[Expression],
    ) -> Result<()> {
        let mut part_count = 0usize;
        for (index, quasi) in quasis.iter().enumerate() {
            if !quasi.as_str().is_empty() {
                self.emit(BytecodeInstruction::PushString(quasi.clone()));
                part_count = checked_template_part_count(part_count)?;
            }
            if let Some(expression) = expressions.get(index) {
                self.compile_expr(expression)?;
                part_count = checked_template_part_count(part_count)?;
            }
        }
        self.emit(BytecodeInstruction::TemplateConcat { part_count });
        Ok(())
    }

    fn compile_binding_assignment_expr(
        &mut self,
        name: &StaticBinding,
        strict: bool,
        infer_name: bool,
        expr: &Expression,
    ) -> Result<()> {
        let binding = BytecodeBinding::compile_write(name, self.layout, strict)?;
        let resolve_before_value = binding.with_environment_count() > 0
            || binding.operand() == crate::binding_metadata::BindingOperand::Unresolved;
        if resolve_before_value {
            self.emit(BytecodeInstruction::ResolveBinding(binding.clone()));
        }
        if infer_name {
            self.compile_expr_with_inferred_name(expr, name.name())?;
        } else {
            self.compile_expr(expr)?;
        }
        self.emit(if resolve_before_value {
            BytecodeInstruction::StoreResolvedBinding(binding)
        } else {
            BytecodeInstruction::StoreBinding(binding)
        });
        Ok(())
    }

    fn compile_new_expr(&mut self, constructor: &Expression, args: &[Expression]) -> Result<()> {
        if has_spread_arg(args) {
            self.compile_expr(constructor)?;
            let spread_flags = self.compile_spread_parts(args)?;
            self.emit(BytecodeInstruction::CollectSpreadArgs { spread_flags });
            self.emit(BytecodeInstruction::ConstructValueSpread);
            return Ok(());
        }
        if let Some(binding) = constructor_binding_expr(constructor) {
            self.compile_args(args)?;
            self.emit(BytecodeInstruction::Construct {
                constructor: self.compile_binding(binding)?,
                native: NativeCallTarget::from_binding_name(binding.as_str()),
                arg_count: args.len(),
            });
            return Ok(());
        }
        self.compile_expr(constructor)?;
        self.compile_args(args)?;
        self.emit(BytecodeInstruction::ConstructValue {
            arg_count: args.len(),
        });
        Ok(())
    }

    fn compile_unary_expr(&mut self, op: UnaryOp, expr: &Expression) -> Result<()> {
        match op {
            UnaryOp::Not | UnaryOp::Negate | UnaryOp::Plus | UnaryOp::BitNot | UnaryOp::Void => {
                self.compile_expr(expr)?;
                if let Some(op) = BytecodeNumericUnaryOp::from_unary(op) {
                    self.emit(BytecodeInstruction::NumberUnary(op));
                } else {
                    self.emit(BytecodeInstruction::Unary(op));
                }
            }
            UnaryOp::Typeof => self.compile_typeof_expr(expr)?,
            UnaryOp::Delete => self.compile_delete_expr(expr)?,
        }
        Ok(())
    }

    fn compile_typeof_expr(&mut self, expr: &Expression) -> Result<()> {
        match expr.kind() {
            Expr::Parenthesized(expr) => self.compile_typeof_expr(expr),
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::TypeOfBinding(
                    self.compile_binding(name)?,
                ));
                Ok(())
            }
            _ => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::TypeOfValue);
                Ok(())
            }
        }
    }

    fn compile_binary_expr(
        &mut self,
        op: BinaryOp,
        left: &Expression,
        right: &Expression,
        property_access: Option<StaticPropertyAccessId>,
    ) -> Result<()> {
        match op {
            BinaryOp::LogicalAnd => self.compile_logical_and(left, right),
            BinaryOp::LogicalOr => self.compile_logical_or(left, right),
            BinaryOp::NullishCoalescing => self.compile_nullish_coalescing(left, right),
            _ => {
                if op == BinaryOp::In
                    && let Some(access) = property_access
                    && let Some(property) = Self::expr_static_string(left)
                {
                    self.compile_expr(right)?;
                    self.emit(BytecodeInstruction::InStaticProperty {
                        property: property.clone(),
                        access: Self::compile_dynamic_property(access),
                    });
                    return Ok(());
                }
                if op == BinaryOp::Add
                    && property_access.is_none()
                    && self.compile_string_concat_chain(left, right)?
                {
                    return Ok(());
                }
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                if property_access.is_none()
                    && let Some(op) = BytecodeNumericBinaryOp::from_binary(op)
                {
                    self.emit(BytecodeInstruction::NumberBinary(op));
                } else if property_access.is_none()
                    && let Some(op) = BytecodeNumericCompareOp::from_binary(op)
                {
                    self.emit(BytecodeInstruction::NumberCompare(op));
                } else if property_access.is_none()
                    && let Some(op) = BytecodeNumericEqualityOp::from_binary(op)
                {
                    self.emit(BytecodeInstruction::NumberEquality(op));
                } else {
                    self.emit(BytecodeInstruction::Binary {
                        op,
                        property_access: property_access.map(Self::compile_dynamic_property),
                    });
                }
                Ok(())
            }
        }
    }

    fn compile_string_concat_chain(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> Result<bool> {
        let mut operands = Vec::new();
        Self::collect_left_add_operands(left, &mut operands);
        operands.push(right);

        if !Self::first_add_is_static_string_concat(operands.as_slice()) {
            return Ok(false);
        }

        let mut iter = operands.into_iter().peekable();
        let Some(first) = iter.next() else {
            return Ok(false);
        };
        let Some(second) = iter.next() else {
            return Ok(false);
        };

        self.compile_expr(first)?;
        self.emit_string_concat_operand(second, iter.peek().is_none())?;

        while let Some(operand) = iter.next() {
            self.emit_string_concat_operand(operand, iter.peek().is_none())?;
        }

        Ok(true)
    }

    fn emit_string_concat_operand(
        &mut self,
        operand: &Expression,
        final_result: bool,
    ) -> Result<()> {
        if let Some(text) = Self::expr_static_string(operand) {
            self.emit(BytecodeInstruction::StringConcatStatic {
                text: text.clone(),
                final_result,
            });
            return Ok(());
        }

        self.compile_expr(operand)?;
        self.emit(BytecodeInstruction::StringConcat { final_result });
        Ok(())
    }

    fn collect_left_add_operands<'a>(expr: &'a Expression, operands: &mut Vec<&'a Expression>) {
        match expr.kind() {
            Expr::Parenthesized(expr) => Self::collect_left_add_operands(expr, operands),
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                right,
                property_access: None,
            } => {
                Self::collect_left_add_operands(left, operands);
                operands.push(right);
            }
            _ => operands.push(expr),
        }
    }

    fn first_add_is_static_string_concat(operands: &[&Expression]) -> bool {
        let mut iter = operands.iter();
        let Some(first) = iter.next() else {
            return false;
        };
        let Some(second) = iter.next() else {
            return false;
        };
        Self::expr_static_string(first).is_some() || Self::expr_static_string(second).is_some()
    }

    fn expr_static_string(expr: &Expression) -> Option<&StaticString> {
        match expr.kind() {
            Expr::StringLiteral(value) => Some(value),
            Expr::Parenthesized(expr) => Self::expr_static_string(expr),
            _ => None,
        }
    }

    fn compile_logical_and(&mut self, left: &Expression, right: &Expression) -> Result<()> {
        self.compile_expr(left)?;
        let end_jump = self.emit_jump_if_false_keep();
        self.emit(BytecodeInstruction::Pop);
        self.compile_expr(right)?;
        let end = self.current_address();
        self.patch_jump(end_jump, end)
    }

    fn compile_nullish_coalescing(&mut self, left: &Expression, right: &Expression) -> Result<()> {
        self.compile_expr(left)?;
        self.emit(BytecodeInstruction::NullishCoalescing {
            right: BytecodeBlock::compile_expression(right, self.layout)?,
        });
        Ok(())
    }

    fn compile_logical_or(&mut self, left: &Expression, right: &Expression) -> Result<()> {
        self.compile_expr(left)?;
        let end_jump = self.emit_jump_if_true_keep();
        self.emit(BytecodeInstruction::Pop);
        self.compile_expr(right)?;
        let end = self.current_address();
        self.patch_jump(end_jump, end)
    }

    fn compile_conditional_expr(
        &mut self,
        condition: &Expression,
        consequent: &Expression,
        alternate: &Expression,
    ) -> Result<()> {
        self.compile_expr(condition)?;
        let false_jump = self.emit_jump_if_false();
        self.compile_expr(consequent)?;
        let end_jump = self.emit_jump();
        let alternate_address = self.current_address();
        self.patch_jump(false_jump, alternate_address)?;
        self.compile_expr(alternate)?;
        let end_address = self.current_address();
        self.patch_jump(end_jump, end_address)
    }

    fn compile_array_literal(&mut self, elements: &[Expression]) -> Result<()> {
        if has_spread_arg(elements) {
            return self.compile_array_literal_spread(elements);
        }
        let holes = self.compile_array_literal_elements(elements)?;
        self.emit(BytecodeInstruction::ArrayLiteral {
            len: elements.len(),
            holes: holes.into(),
        });
        Ok(())
    }

    fn compile_array_literal_spread(&mut self, elements: &[Expression]) -> Result<()> {
        let mut spread_flags = Vec::with_capacity(elements.len());
        let mut holes = Vec::with_capacity(elements.len());
        for element in elements {
            match element.kind() {
                Expr::ArrayHole => {
                    spread_flags.push(false);
                    holes.push(true);
                }
                Expr::Spread(inner) => {
                    self.compile_expr(inner)?;
                    spread_flags.push(true);
                    holes.push(false);
                }
                _ => {
                    self.compile_expr(element)?;
                    spread_flags.push(false);
                    holes.push(false);
                }
            }
        }
        self.emit(BytecodeInstruction::ArrayLiteralSpread {
            spread_flags: spread_flags.into(),
            holes: holes.into(),
        });
        Ok(())
    }

    fn compile_array_literal_elements(&mut self, elements: &[Expression]) -> Result<Vec<bool>> {
        let mut holes = Vec::with_capacity(elements.len());
        for element in elements {
            if matches!(element.kind(), Expr::ArrayHole) {
                holes.push(true);
                continue;
            }
            if matches!(element.kind(), Expr::Spread(_)) {
                return Err(Error::runtime(
                    "array spread was not compiled on spread path",
                ));
            }
            holes.push(false);
            self.compile_expr(element)?;
        }
        Ok(holes)
    }

    /// Compiles a mixed plain/spread expression list, returning per-slot
    /// spread flags for the matching collection instruction.
    pub(super) fn compile_spread_parts(&mut self, parts: &[Expression]) -> Result<Rc<[bool]>> {
        let mut spread_flags = Vec::with_capacity(parts.len());
        for part in parts {
            if let Expr::Spread(inner) = part.kind() {
                self.compile_expr(inner)?;
                spread_flags.push(true);
            } else {
                self.compile_expr(part)?;
                spread_flags.push(false);
            }
        }
        Ok(spread_flags.into())
    }
}
