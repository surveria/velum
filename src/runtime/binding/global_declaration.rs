use crate::{
    bytecode::BytecodeHoistPlan,
    error::{Error, Result},
    runtime::Context,
    syntax::DeclKind,
    value::ErrorName,
};

impl Context {
    pub(in crate::runtime::binding) fn validate_global_declaration_instantiation(
        &mut self,
        plan: &BytecodeHoistPlan,
    ) -> Result<()> {
        for (binding, _) in plan.lexical_declarations() {
            let name = binding.as_str();
            if self.global_name_has_lexical_declaration(name)
                || self
                    .global_lexical_conflict_descriptor(name)?
                    .is_some_and(|descriptor| !descriptor.configurable().is_yes())
            {
                return Err(Error::exception(
                    ErrorName::SyntaxError,
                    format!("'{name}' has already been declared"),
                ));
            }
        }
        for binding in plan.var_declarations() {
            self.ensure_global_name_is_not_lexical(binding.as_str())?;
            if !self.can_declare_global_var(binding.as_str())? {
                return Err(Error::exception(
                    ErrorName::TypeError,
                    format!("global variable '{binding}' cannot be declared"),
                ));
            }
        }
        for declaration in plan.function_declarations() {
            let name = declaration.name().name().as_str();
            self.ensure_global_name_is_not_lexical(name)?;
            if !self.can_declare_global_function(name)? {
                return Err(Error::exception(
                    ErrorName::TypeError,
                    format!("global function '{name}' cannot be declared"),
                ));
            }
        }
        Ok(())
    }

    fn global_name_has_lexical_declaration(&self, name: &str) -> bool {
        self.atom(name).is_some_and(|atom| {
            self.realm
                .globals
                .get(atom)
                .is_some_and(|binding| binding.kind() != DeclKind::Var)
        })
    }

    fn global_lexical_conflict_descriptor(
        &mut self,
        name: &str,
    ) -> Result<Option<crate::runtime::object::OwnPropertyDescriptor>> {
        if self.realm.global_object.is_some() {
            return self.global_own_property_descriptor(name);
        }
        self.global_binding_property_descriptor(name)
    }

    pub(in crate::runtime::binding) fn ensure_global_name_is_not_lexical(
        &self,
        name: &str,
    ) -> Result<()> {
        if !self.global_name_has_lexical_declaration(name) {
            return Ok(());
        }
        Err(Error::exception(
            ErrorName::SyntaxError,
            format!("'{name}' has already been declared"),
        ))
    }
}
