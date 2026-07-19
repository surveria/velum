#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    ast::{
        ClassAutoAccessor, ClassAutoAccessorFunction, Expr, Expression, FunctionParam, Statement,
        Stmt,
    },
    error::Result,
};

use super::{Parser, class_private::PrivateElementKind};

impl Parser {
    pub(super) fn build_public_auto_accessor(
        &mut self,
        is_static: bool,
        member_offset: usize,
    ) -> Result<ClassAutoAccessor> {
        let backing_name = self.static_name(format!("#%auto_accessor_{member_offset}"))?;
        self.declare_private_name(
            &backing_name,
            PrivateElementKind::Field,
            is_static,
            member_offset,
        )?;
        let span = self.previous_span();
        self.record_private_name_use(&backing_name, span)?;
        let getter_value = Expression::new(
            Expr::PrivateMember {
                object: Box::new(Expression::new(Expr::This, span)),
                name: backing_name.clone(),
            },
            span,
        );
        let getter = ClassAutoAccessorFunction {
            id: self.static_function()?,
            params: Vec::new().into(),
            body: vec![Statement::new(Stmt::Return(Some(getter_value)), span)].into(),
        };

        self.record_private_name_use(&backing_name, span)?;
        let value_binding =
            self.static_binding_name(format!("%auto_accessor_value_{member_offset}"))?;
        let setter_value = Expression::new(
            Expr::PrivateAssignment {
                object: Box::new(Expression::new(Expr::This, span)),
                name: backing_name.clone(),
                expr: Box::new(Expression::new(
                    Expr::Identifier(value_binding.clone()),
                    span,
                )),
            },
            span,
        );
        let setter = ClassAutoAccessorFunction {
            id: self.static_function()?,
            params: vec![FunctionParam::new(value_binding, None)].into(),
            body: vec![Statement::new(Stmt::Expr(setter_value), span)].into(),
        };
        Ok(ClassAutoAccessor {
            backing_name,
            getter,
            setter,
        })
    }
}
