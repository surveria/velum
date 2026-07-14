use std::rc::Rc;

use crate::runtime::private::{PrivateSlot, PrivateSlotValue};
use crate::{
    bytecode::{
        BytecodeAddress, BytecodeClass, BytecodeClassDefinitionElement, BytecodeClassMember,
        BytecodeClassMemberKey, BytecodeClassMemberKind, BytecodeClassStaticElement,
        BytecodeNewTargetMode,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::control::Completion,
    runtime::function::{BytecodeFunctionInit, FunctionSuperBinding, ResolvedClassField},
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
    },
    value::{FunctionId, ObjectId, Value},
};

use super::state::BytecodeState;

mod auto_accessor;

impl Context {
    pub(super) fn eval_bytecode_create_class(
        &mut self,
        state: &mut BytecodeState,
        class: &BytecodeClass,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let element_input_count = class.members.iter().try_fold(0_usize, |count, member| {
            class_element_input_count(count, member.decorator_count, &member.key)
        })?;
        let element_input_count = class
            .fields
            .iter()
            .try_fold(element_input_count, |count, field| {
                class_element_input_count(count, field.decorator_count, &field.key)
            })?;
        let element_inputs = state.stack.pop_many(element_input_count)?;
        let heritage = if class.heritage {
            Some(state.stack.pop()?)
        } else {
            None
        };
        let heritage = heritage
            .map(|value| self.resolve_class_heritage(value))
            .transpose()?;
        let class_decorators = state.stack.pop_many(class.decorator_count)?;

        let constructor = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: class.constructor_id,
            name: class.name.as_ref(),
            bytecode: &class.constructor,
            constructable: true,
            kind: crate::syntax::FunctionKind::Ordinary,
            class_constructor: true,
            prototype_parent: heritage.as_ref().and_then(ClassHeritage::prototype_parent),
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(constructor_id) = &constructor else {
            return Err(Error::runtime("class constructor creation failed"));
        };
        self.set_function_default_derived_constructor(
            *constructor_id,
            class.default_derived_constructor,
        )?;
        if let Some(binding) = &class.inner_name_binding {
            self.eval_bytecode_declaration(
                binding,
                crate::syntax::DeclKind::Const,
                Some(constructor.clone()),
            )?;
            self.retain_function_class_name_environment(*constructor_id, binding)?;
        }
        let Some(prototype_id) = self.function_constructor_prototype(*constructor_id)? else {
            return Err(Error::runtime("class prototype object is not available"));
        };
        if heritage
            .as_ref()
            .is_some_and(|heritage| matches!(heritage.constructor, Value::Undefined))
        {
            self.objects
                .set_prototype_value(prototype_id, &Value::Null)?;
        }
        if let Some(heritage) = &heritage
            && !matches!(heritage.constructor, Value::Undefined)
        {
            self.set_function_static_parent(*constructor_id, heritage.constructor.clone())?;
        }
        self.set_function_super_binding(
            *constructor_id,
            Rc::new(FunctionSuperBinding {
                constructor: heritage
                    .as_ref()
                    .map(|heritage| heritage.constructor.clone()),
                home_object: Value::Object(prototype_id),
                own_constructor: Some(*constructor_id),
                this_value: std::cell::RefCell::new(None),
                allow_direct_eval_super_call: std::cell::Cell::new(heritage.is_some()),
            }),
        )?;
        let static_super_binding = Rc::new(FunctionSuperBinding {
            constructor: None,
            home_object: constructor.clone(),
            own_constructor: None,
            this_value: std::cell::RefCell::new(None),
            allow_direct_eval_super_call: std::cell::Cell::new(false),
        });
        let targets = ClassInstallationTargets {
            constructor: constructor.clone(),
            constructor_id: *constructor_id,
            prototype_id,
        };
        let static_fields = self.install_class_elements(class, &targets, element_inputs)?;
        self.evaluate_class_static_elements(
            class,
            &constructor,
            *constructor_id,
            &static_fields,
            &static_super_binding,
        )?;

        let constructor = self.apply_callable_decorators(
            constructor,
            class_decorators,
            DecoratorContext::Class {
                name: class.name.as_ref().map_or("", |name| name.as_str()),
            },
        )?;
        state.stack.push(constructor);
        state.pc = next;
        Ok(None)
    }

    fn install_class_elements(
        &mut self,
        class: &BytecodeClass,
        targets: &ClassInstallationTargets,
        inputs: Vec<Value>,
    ) -> Result<Vec<ResolvedClassField>> {
        let mut inputs = inputs.into_iter();
        let mut instance_private_slots: Vec<PrivateSlot> = Vec::new();
        let mut instance_fields = Vec::new();
        let mut static_fields = Vec::new();
        for element in class.definition_order.iter() {
            match element {
                BytecodeClassDefinitionElement::Member(index) => {
                    let member = class
                        .members
                        .get(*index)
                        .ok_or_else(|| Error::runtime("class member definition disappeared"))?;
                    self.install_ordered_class_member(
                        class,
                        member,
                        targets,
                        &mut inputs,
                        &mut instance_private_slots,
                    )?;
                }
                BytecodeClassDefinitionElement::Field(index) => {
                    let field = class
                        .fields
                        .get(*index)
                        .ok_or_else(|| Error::runtime("class field definition disappeared"))?;
                    let decorators = take_class_input_values(
                        &mut inputs,
                        field.decorator_count,
                        "class field decorator",
                    )?;
                    let computed_key = take_class_computed_key(
                        &mut inputs,
                        &field.key,
                        "class field key disappeared",
                    )?;
                    let resolved = self.resolve_class_field(
                        class,
                        field,
                        decorators,
                        computed_key.as_ref(),
                        targets,
                    )?;
                    if field.is_static {
                        static_fields.push(resolved);
                    } else {
                        instance_fields.push(resolved);
                    }
                }
            }
        }
        if !instance_private_slots.is_empty() {
            self.set_function_class_private_slots(
                targets.constructor_id,
                instance_private_slots.into(),
            )?;
        }
        if !instance_fields.is_empty() {
            self.set_function_class_fields(targets.constructor_id, instance_fields.into())?;
        }
        if inputs.next().is_some() {
            return Err(Error::runtime("unused class element evaluation input"));
        }
        Ok(static_fields)
    }

    fn install_ordered_class_member(
        &mut self,
        class: &BytecodeClass,
        member: &BytecodeClassMember,
        targets: &ClassInstallationTargets,
        inputs: &mut std::vec::IntoIter<Value>,
        instance_private_slots: &mut Vec<PrivateSlot>,
    ) -> Result<()> {
        let decorators =
            take_class_input_values(inputs, member.decorator_count, "class member decorator")?;
        let computed_key =
            take_class_computed_key(inputs, &member.key, "class computed member key disappeared")?;
        let (function_id, private_slot) = self.install_class_member(
            member,
            decorators,
            computed_key.as_ref(),
            targets.constructor_id,
            targets.prototype_id,
            &class.private_names,
        )?;
        if let Some(private_slot) = private_slot {
            if member.is_static {
                self.add_or_merge_private_slot_to_value(
                    &targets.constructor,
                    private_slot.id,
                    private_slot.value,
                )?;
            } else if let Some(existing) = instance_private_slots
                .iter_mut()
                .find(|slot| slot.id == private_slot.id)
            {
                existing.value.merge_accessor(private_slot.value)?;
            } else {
                instance_private_slots.push(private_slot);
            }
        }
        let home_object = if member.is_static {
            targets.constructor.clone()
        } else {
            Value::Object(targets.prototype_id)
        };
        self.set_function_super_binding(
            function_id,
            Rc::new(FunctionSuperBinding {
                constructor: None,
                home_object,
                own_constructor: None,
                this_value: std::cell::RefCell::new(None),
                allow_direct_eval_super_call: std::cell::Cell::new(false),
            }),
        )
    }

    fn evaluate_class_static_elements(
        &mut self,
        class: &BytecodeClass,
        constructor: &Value,
        constructor_id: FunctionId,
        static_fields: &[ResolvedClassField],
        super_binding: &Rc<FunctionSuperBinding>,
    ) -> Result<()> {
        for element in class.static_element_order.iter() {
            match element {
                BytecodeClassStaticElement::Field(index) => {
                    let field = static_fields
                        .get(*index)
                        .ok_or_else(|| Error::runtime("static class field disappeared"))?;
                    self.evaluate_class_static_field(
                        constructor,
                        constructor_id,
                        field,
                        super_binding,
                    )?;
                }
                BytecodeClassStaticElement::Block(index) => {
                    let block = class
                        .static_blocks
                        .get(*index)
                        .ok_or_else(|| Error::runtime("class static block disappeared"))?;
                    self.push_class_evaluation(
                        constructor.clone(),
                        super_binding.clone(),
                        self.current_private_environment(),
                        false,
                    )?;
                    let completion = self.eval_bytecode_block(block);
                    self.pop_temporary_this()?;
                    completion?.into_result()?;
                }
            }
        }
        Ok(())
    }

    fn evaluate_class_static_field(
        &mut self,
        constructor: &Value,
        constructor_id: FunctionId,
        field: &ResolvedClassField,
        super_binding: &Rc<FunctionSuperBinding>,
    ) -> Result<()> {
        self.push_class_evaluation(
            constructor.clone(),
            super_binding.clone(),
            self.current_private_environment(),
            true,
        )?;
        let initializer = match field {
            ResolvedClassField::Public { initializer, .. }
            | ResolvedClassField::Private { initializer, .. }
            | ResolvedClassField::AutoAccessor { initializer, .. } => initializer,
        };
        let value = initializer
            .as_ref()
            .map_or(Ok(Completion::Normal(Value::Undefined)), |initializer| {
                self.eval_bytecode_block(initializer)
            });
        self.pop_temporary_this()?;
        let mut value = value?.into_result()?;
        for initializer in field.decorator_initializers() {
            value = self
                .semantic_call(initializer, &[value], constructor.clone())?
                .into_result()?;
        }
        match field {
            ResolvedClassField::Public {
                key,
                name,
                infer_name,
                ..
            } => {
                if *infer_name {
                    self.set_function_name(&value, name, None)?;
                }
                let update = DataPropertyUpdate::new(
                    Some(value),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                );
                self.define_function_property_key(
                    constructor_id,
                    name,
                    *key,
                    PropertyUpdate::Data(update),
                )
            }
            ResolvedClassField::Private { name, .. } => self.add_private_slot_to_value(
                constructor,
                name.clone(),
                PrivateSlotValue::Field(value),
            ),
            ResolvedClassField::AutoAccessor { backing_name, .. } => self
                .add_private_slot_to_value(
                    constructor,
                    backing_name.clone(),
                    PrivateSlotValue::Field(value),
                ),
        }
    }

    fn install_class_member(
        &mut self,
        member: &BytecodeClassMember,
        decorators: Vec<Value>,
        computed_key: Option<&Value>,
        constructor_id: FunctionId,
        prototype_id: ObjectId,
        private_names: &[crate::syntax::StaticName],
    ) -> Result<(FunctionId, Option<PrivateSlot>)> {
        let function = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: member.id,
            name: None,
            bytecode: &member.bytecode,
            constructable: false,
            kind: member.function_kind,
            class_constructor: false,
            prototype_parent: None,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(function_id) = function.clone() else {
            return Err(Error::runtime("class member creation failed"));
        };

        let prefix = match member.kind {
            BytecodeClassMemberKind::Method => None,
            BytecodeClassMemberKind::Getter => Some(crate::syntax::AccessorKind::Getter),
            BytecodeClassMemberKind::Setter => Some(crate::syntax::AccessorKind::Setter),
        };
        if let BytecodeClassMemberKey::Private { index } = member.key {
            let index_usize = usize::try_from(index)
                .map_err(|_| Error::limit("private name index exceeded supported range"))?;
            let name = private_names
                .get(index_usize)
                .ok_or_else(|| Error::runtime("private class member name disappeared"))?;
            self.set_function_name(&function, name.as_str(), prefix)?;
            let function = self.apply_callable_decorators(
                function,
                decorators,
                DecoratorContext::Element {
                    kind: class_member_decorator_kind(member.kind),
                    name: name.as_str(),
                    is_static: member.is_static,
                    is_private: true,
                },
            )?;
            let value = match member.kind {
                BytecodeClassMemberKind::Method => PrivateSlotValue::Method(function),
                BytecodeClassMemberKind::Getter => PrivateSlotValue::Accessor {
                    getter: Some(function),
                    setter: None,
                },
                BytecodeClassMemberKind::Setter => PrivateSlotValue::Accessor {
                    getter: None,
                    setter: Some(function),
                },
            };
            return Ok((
                function_id,
                Some(PrivateSlot {
                    id: self.resolve_own_private_name(index)?,
                    value,
                }),
            ));
        }

        let (key, name, function_name) =
            self.class_member_property_key(&member.key, computed_key)?;
        self.set_function_name(&function, &function_name, prefix)?;
        let function = self.apply_callable_decorators(
            function,
            decorators,
            DecoratorContext::Element {
                kind: class_member_decorator_kind(member.kind),
                name: &name,
                is_static: member.is_static,
                is_private: false,
            },
        )?;

        let update = match member.kind {
            BytecodeClassMemberKind::Method => PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(function),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            BytecodeClassMemberKind::Getter => {
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    Some(function),
                    None,
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ))
            }
            BytecodeClassMemberKind::Setter => {
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    None,
                    Some(function),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ))
            }
        };
        if member.is_static {
            self.define_function_property_key(constructor_id, &name, key, update)?;
            return Ok((function_id, None));
        }
        self.objects.define_property(
            prototype_id,
            key,
            &name,
            update,
            self.limits.max_object_properties,
        )?;
        Ok((function_id, None))
    }

    fn class_member_property_key(
        &mut self,
        key: &BytecodeClassMemberKey,
        computed_key: Option<&Value>,
    ) -> Result<(PropertyKey, String, String)> {
        match (key, computed_key) {
            (BytecodeClassMemberKey::Static(name), _) => {
                let name = name.as_str().to_owned();
                Ok((self.intern_property_key(&name)?, name.clone(), name))
            }
            (BytecodeClassMemberKey::Computed, Some(value)) => {
                let mut property = self.dynamic_property_key(value)?;
                let function_name = self.function_name_from_property(&property)?;
                let key = self.intern_dynamic_property_key(&mut property)?;
                Ok((key, property.name().to_owned(), function_name))
            }
            (BytecodeClassMemberKey::Computed, None) => {
                Err(Error::runtime("class computed member key disappeared"))
            }
            (BytecodeClassMemberKey::Private { .. }, _) => {
                Err(Error::runtime("private class element has no property key"))
            }
        }
    }

    fn apply_callable_decorators(
        &mut self,
        initial: Value,
        decorators: Vec<Value>,
        context: DecoratorContext<'_>,
    ) -> Result<Value> {
        let mut value = initial;
        for decorator in decorators.into_iter().rev() {
            if !self.semantic_is_callable(&decorator)? {
                return Err(Error::type_error("class decorator is not callable"));
            }
            let context_value = self.decorator_context_object(context)?;
            let result = self
                .semantic_call(
                    &decorator,
                    &[value.clone(), context_value],
                    Value::Undefined,
                )?
                .into_result()?;
            if matches!(result, Value::Undefined) {
                continue;
            }
            if !self.semantic_is_callable(&result)? {
                return Err(Error::type_error(
                    "class or method decorator must return a callable value or undefined",
                ));
            }
            value = result;
        }
        Ok(value)
    }

    fn apply_field_decorators(
        &mut self,
        decorators: Vec<Value>,
        kind: &'static str,
        name: &str,
        is_static: bool,
        is_private: bool,
    ) -> Result<Vec<Value>> {
        let mut initializers = Vec::new();
        let context = DecoratorContext::Element {
            kind,
            name,
            is_static,
            is_private,
        };
        for decorator in decorators.into_iter().rev() {
            if !self.semantic_is_callable(&decorator)? {
                return Err(Error::type_error("class field decorator is not callable"));
            }
            let context_value = self.decorator_context_object(context)?;
            let result = self
                .semantic_call(
                    &decorator,
                    &[Value::Undefined, context_value],
                    Value::Undefined,
                )?
                .into_result()?;
            if matches!(result, Value::Undefined) {
                continue;
            }
            if !self.semantic_is_callable(&result)? {
                return Err(Error::type_error(
                    "class field decorator must return a callable value or undefined",
                ));
            }
            initializers.push(result);
        }
        Ok(initializers)
    }

    fn decorator_context_object(&mut self, context: DecoratorContext<'_>) -> Result<Value> {
        let kind_key = self.intern_property_key(DECORATOR_KIND_PROPERTY)?;
        let name_key = self.intern_property_key(DECORATOR_NAME_PROPERTY)?;
        let kind_value = self.heap_string_value(context.kind())?;
        let name_value = self.heap_string_value(context.name())?;
        let mut properties = vec![
            ObjectPropertyInit::new_data(
                kind_key,
                DECORATOR_KIND_PROPERTY,
                kind_value,
                PropertyEnumerable::Yes,
            ),
            ObjectPropertyInit::new_data(
                name_key,
                DECORATOR_NAME_PROPERTY,
                name_value,
                PropertyEnumerable::Yes,
            ),
        ];
        if let DecoratorContext::Element {
            is_static,
            is_private,
            ..
        } = context
        {
            let static_key = self.intern_property_key(DECORATOR_STATIC_PROPERTY)?;
            let private_key = self.intern_property_key(DECORATOR_PRIVATE_PROPERTY)?;
            properties.push(ObjectPropertyInit::new_data(
                static_key,
                DECORATOR_STATIC_PROPERTY,
                Value::Bool(is_static),
                PropertyEnumerable::Yes,
            ));
            properties.push(ObjectPropertyInit::new_data(
                private_key,
                DECORATOR_PRIVATE_PROPERTY,
                Value::Bool(is_private),
                PropertyEnumerable::Yes,
            ));
        }
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn resolve_class_heritage(&mut self, value: Value) -> Result<ClassHeritage> {
        if matches!(value, Value::Null) {
            return Ok(ClassHeritage {
                constructor: Value::Undefined,
                prototype_id: None,
            });
        }
        if !self.semantic_is_constructor(&value)? {
            return Err(Error::type_error(format!(
                "class heritage '{value}' is not a constructor"
            )));
        }
        let prototype = self.get_named(&value, CLASS_PROTOTYPE_PROPERTY)?;
        let prototype_id = match prototype {
            Value::Object(id) => Some(id),
            Value::Null => None,
            other => {
                return Err(Error::type_error(format!(
                    "class heritage prototype '{other}' is not an object or null"
                )));
            }
        };
        Ok(ClassHeritage {
            constructor: value,
            prototype_id,
        })
    }
}

#[derive(Clone, Copy)]
enum DecoratorContext<'a> {
    Class {
        name: &'a str,
    },
    Element {
        kind: &'static str,
        name: &'a str,
        is_static: bool,
        is_private: bool,
    },
}

impl<'a> DecoratorContext<'a> {
    const fn kind(self) -> &'static str {
        match self {
            Self::Class { .. } => "class",
            Self::Element { kind, .. } => kind,
        }
    }

    const fn name(self) -> &'a str {
        match self {
            Self::Class { name } | Self::Element { name, .. } => name,
        }
    }
}

fn class_element_input_count(
    count: usize,
    decorator_count: usize,
    key: &BytecodeClassMemberKey,
) -> Result<usize> {
    count
        .checked_add(decorator_count)
        .and_then(|count| {
            count.checked_add(usize::from(matches!(key, BytecodeClassMemberKey::Computed)))
        })
        .ok_or_else(|| Error::limit("class evaluation input count overflowed"))
}

fn take_class_input_values(
    inputs: &mut std::vec::IntoIter<Value>,
    count: usize,
    description: &str,
) -> Result<Vec<Value>> {
    let values = inputs.by_ref().take(count).collect::<Vec<_>>();
    if values.len() != count {
        return Err(Error::runtime(format!("{description} value disappeared")));
    }
    Ok(values)
}

fn take_class_computed_key(
    inputs: &mut std::vec::IntoIter<Value>,
    key: &BytecodeClassMemberKey,
    missing_message: &str,
) -> Result<Option<Value>> {
    match key {
        BytecodeClassMemberKey::Computed => inputs
            .next()
            .map(Some)
            .ok_or_else(|| Error::runtime(missing_message)),
        BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Private { .. } => Ok(None),
    }
}

const fn class_member_decorator_kind(kind: BytecodeClassMemberKind) -> &'static str {
    match kind {
        BytecodeClassMemberKind::Method => "method",
        BytecodeClassMemberKind::Getter => "getter",
        BytecodeClassMemberKind::Setter => "setter",
    }
}

const DECORATOR_KIND_PROPERTY: &str = "kind";
const DECORATOR_NAME_PROPERTY: &str = "name";
const DECORATOR_STATIC_PROPERTY: &str = "static";
const DECORATOR_PRIVATE_PROPERTY: &str = "private";

const CLASS_PROTOTYPE_PROPERTY: &str = "prototype";

pub(super) struct ClassInstallationTargets {
    constructor: Value,
    constructor_id: FunctionId,
    prototype_id: ObjectId,
}

/// Resolved `extends` heritage: the parent constructor value plus its
/// prototype object used as the parent of the class prototype.
struct ClassHeritage {
    constructor: Value,
    prototype_id: Option<crate::value::ObjectId>,
}

impl ClassHeritage {
    const fn prototype_parent(&self) -> Option<crate::value::ObjectId> {
        self.prototype_id
    }
}
