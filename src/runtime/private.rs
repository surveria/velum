use std::rc::Rc;

use crate::{
    bytecode::BytecodePrivateName,
    error::{Error, Result},
    syntax::StaticName,
    value::Value,
};

use super::{Context, bytecode::state::BytecodeState};

#[derive(Debug)]
struct PrivateEnvironmentIdentity;

#[derive(Clone, Debug)]
pub(in crate::runtime) struct PrivateNameId {
    environment: Rc<PrivateEnvironmentIdentity>,
    index: u32,
}

impl PrivateNameId {
    const fn new(environment: Rc<PrivateEnvironmentIdentity>, index: u32) -> Self {
        Self { environment, index }
    }
}

impl PartialEq for PrivateNameId {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Rc::ptr_eq(&self.environment, &other.environment)
    }
}

impl Eq for PrivateNameId {}

#[derive(Debug)]
pub(in crate::runtime) struct PrivateEnvironment {
    identity: Rc<PrivateEnvironmentIdentity>,
    names: Rc<[StaticName]>,
    parent: Option<Rc<Self>>,
}

impl PrivateEnvironment {
    pub(in crate::runtime) fn new(names: Rc<[StaticName]>, parent: Option<Rc<Self>>) -> Self {
        Self {
            identity: Rc::new(PrivateEnvironmentIdentity),
            names,
            parent,
        }
    }

    pub(in crate::runtime) fn resolve(&self, name: &str) -> Result<PrivateNameId> {
        if let Some(index) = self
            .names
            .iter()
            .position(|candidate| candidate.as_str() == name)
        {
            let index = u32::try_from(index)
                .map_err(|_| Error::limit("private name index exceeded supported range"))?;
            return Ok(PrivateNameId::new(self.identity.clone(), index));
        }
        let Some(parent) = &self.parent else {
            return Err(Error::runtime(format!(
                "private name '{name}' is not available in the current class environment"
            )));
        };
        parent.resolve(name)
    }

    pub(in crate::runtime) fn own_name(&self, index: u32) -> Result<PrivateNameId> {
        let index_usize = usize::try_from(index)
            .map_err(|_| Error::limit("private name index exceeded supported range"))?;
        if self.names.get(index_usize).is_none() {
            return Err(Error::runtime("private name index is not defined"));
        }
        Ok(PrivateNameId::new(self.identity.clone(), index))
    }

    pub(in crate::runtime) fn parent(&self) -> Option<Rc<Self>> {
        self.parent.clone()
    }
}

#[derive(Clone, Debug)]
pub(in crate::runtime) enum PrivateSlotValue {
    Field(Value),
    Method(Value),
    Accessor {
        getter: Option<Value>,
        setter: Option<Value>,
    },
}

impl PrivateSlotValue {
    pub(in crate::runtime) fn values(&self) -> impl Iterator<Item = &Value> {
        let (first, second) = match self {
            Self::Field(value) | Self::Method(value) => (Some(value), None),
            Self::Accessor { getter, setter } => (getter.as_ref(), setter.as_ref()),
        };
        first.into_iter().chain(second)
    }

    pub(in crate::runtime) fn merge_accessor(&mut self, incoming: Self) -> Result<()> {
        let Self::Accessor { getter, setter } = incoming else {
            return Err(Error::type_error("duplicate private element"));
        };
        let Self::Accessor {
            getter: current_read,
            setter: current_write,
        } = self
        else {
            return Err(Error::type_error("duplicate private element"));
        };
        if getter.is_some() {
            if current_read.is_some() {
                return Err(Error::type_error("duplicate private getter"));
            }
            *current_read = getter;
        }
        if setter.is_some() {
            if current_write.is_some() {
                return Err(Error::type_error("duplicate private setter"));
            }
            *current_write = setter;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(in crate::runtime) struct PrivateSlot {
    pub(in crate::runtime) id: PrivateNameId,
    pub(in crate::runtime) value: PrivateSlotValue,
}

impl Context {
    pub(in crate::runtime) fn begin_private_environment(
        &mut self,
        state: &mut BytecodeState,
        names: Rc<[StaticName]>,
    ) -> Result<()> {
        let parent = state.private_environment();
        let environment = Some(Rc::new(PrivateEnvironment::new(names, parent)));
        state.replace_private_environment(environment.clone());
        self.set_current_private_environment(environment)?;
        Ok(())
    }

    pub(in crate::runtime) fn resolve_private_name(
        &self,
        name: &BytecodePrivateName,
    ) -> Result<PrivateNameId> {
        let Some(environment) = self.current_private_environment() else {
            return Err(Error::runtime("private name environment is not active"));
        };
        environment.resolve(name.name().as_str())
    }

    pub(in crate::runtime) fn resolve_own_private_name(&self, index: u32) -> Result<PrivateNameId> {
        let Some(environment) = self.current_private_environment() else {
            return Err(Error::runtime("private class environment is not active"));
        };
        environment.own_name(index)
    }

    pub(in crate::runtime) fn leave_private_environment(
        &mut self,
        state: &mut BytecodeState,
    ) -> Result<()> {
        let Some(environment) = state.private_environment() else {
            return Err(Error::runtime("private class environment disappeared"));
        };
        let parent = environment.parent();
        state.replace_private_environment(parent.clone());
        self.set_current_private_environment(parent)
    }

    fn private_target(&self, value: &Value) -> Result<Value> {
        let mut target = value.clone();
        loop {
            let Value::Object(id) = target else {
                return Ok(target);
            };
            let Some(proxy) = self.objects.proxy_value(id)? else {
                return Ok(Value::Object(id));
            };
            let Some(next) = proxy.target() else {
                return Err(Error::type_error(
                    "Cannot access private member through revoked Proxy",
                ));
            };
            target = next.clone();
        }
    }

    fn private_slot_for_value(
        &self,
        value: &Value,
        name: &PrivateNameId,
    ) -> Result<Option<PrivateSlotValue>> {
        match self.private_target(value)? {
            Value::Object(id) => self.objects.private_slot(id, name),
            Value::Function(id) => self.function_private_slot(id, name),
            _ => Ok(None),
        }
    }

    pub(in crate::runtime) fn has_private_slot(
        &self,
        value: &Value,
        name: &PrivateNameId,
    ) -> Result<bool> {
        if !matches!(
            self.private_target(value)?,
            Value::Object(_) | Value::Function(_)
        ) {
            return Err(Error::type_error(
                "right-hand side of private 'in' is not an object",
            ));
        }
        Ok(self.private_slot_for_value(value, name)?.is_some())
    }

    pub(in crate::runtime) fn read_private_slot(
        &mut self,
        receiver: &Value,
        name: &PrivateNameId,
    ) -> Result<Value> {
        let Some(slot) = self.private_slot_for_value(receiver, name)? else {
            return Err(Error::type_error(
                "receiver does not have the required private brand",
            ));
        };
        match slot {
            PrivateSlotValue::Field(value) | PrivateSlotValue::Method(value) => Ok(value),
            PrivateSlotValue::Accessor {
                getter: Some(getter),
                ..
            } => self.call_accessor_getter(&getter, receiver.clone()),
            PrivateSlotValue::Accessor { getter: None, .. } => Err(Error::type_error(
                "private accessor was defined without a getter",
            )),
        }
    }

    pub(in crate::runtime) fn write_private_slot(
        &mut self,
        receiver: &Value,
        name: &PrivateNameId,
        value: Value,
    ) -> Result<()> {
        let Some(slot) = self.private_slot_for_value(receiver, name)? else {
            return Err(Error::type_error(
                "receiver does not have the required private brand",
            ));
        };
        match slot {
            PrivateSlotValue::Field(_) => match self.private_target(receiver)? {
                Value::Object(id) => {
                    if self.objects.set_private_field(id, name, value)? {
                        Ok(())
                    } else {
                        Err(Error::runtime(
                            "private field disappeared during assignment",
                        ))
                    }
                }
                Value::Function(id) => {
                    if self.set_function_private_field(id, name, value)? {
                        Ok(())
                    } else {
                        Err(Error::runtime(
                            "private field disappeared during assignment",
                        ))
                    }
                }
                _ => Err(Error::type_error("private field receiver is not an object")),
            },
            PrivateSlotValue::Accessor {
                setter: Some(setter),
                ..
            } => self
                .call_accessor_function(&setter, receiver.clone(), &[value])
                .map(|_| ()),
            PrivateSlotValue::Accessor { setter: None, .. } => Err(Error::type_error(
                "private accessor was defined without a setter",
            )),
            PrivateSlotValue::Method(_) => Err(Error::type_error("private method is not writable")),
        }
    }

    pub(in crate::runtime) fn add_private_slot_to_value(
        &mut self,
        receiver: &Value,
        name: PrivateNameId,
        value: PrivateSlotValue,
    ) -> Result<()> {
        match self.private_target(receiver)? {
            Value::Object(id) => {
                self.objects
                    .add_private_slot(id, name, value, self.limits.max_object_properties)
            }
            Value::Function(id) => self.add_function_private_slot(id, name, value),
            _ => Err(Error::type_error(
                "private element receiver is not an object",
            )),
        }
    }

    pub(in crate::runtime) fn add_or_merge_private_slot_to_value(
        &mut self,
        receiver: &Value,
        name: PrivateNameId,
        value: PrivateSlotValue,
    ) -> Result<()> {
        if let Some(mut existing) = self.private_slot_for_value(receiver, &name)? {
            existing.merge_accessor(value)?;
            match self.private_target(receiver)? {
                Value::Object(id) => self.objects.replace_private_slot(id, &name, existing),
                Value::Function(id) => self.replace_function_private_slot(id, &name, existing),
                _ => Err(Error::type_error(
                    "private element receiver is not an object",
                )),
            }
        } else {
            self.add_private_slot_to_value(receiver, name, value)
        }
    }
}
