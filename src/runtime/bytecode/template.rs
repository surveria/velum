use crate::{
    bytecode::{BytecodeAddress, BytecodeCallSite, BytecodeTemplateElement},
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
            PropertyWritable,
        },
    },
    value::{ObjectId, Value},
};

use super::state::BytecodeState;

const TEMPLATE_RAW_PROPERTY: &str = "raw";

impl Context {
    pub(super) fn eval_bytecode_get_template_object(
        &mut self,
        state: &mut BytecodeState,
        site: BytecodeCallSite,
        quasis: &[BytecodeTemplateElement],
        next: BytecodeAddress,
    ) -> Result<Option<crate::runtime::control::Completion>> {
        let value = if let Some(value) = self.cached_template_object(site.site())? {
            value
        } else {
            let value = self.create_template_object(quasis)?;
            self.remember_template_object(site.site(), value.clone())?;
            value
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn create_template_object(&mut self, quasis: &[BytecodeTemplateElement]) -> Result<Value> {
        if quasis.is_empty() {
            return Err(Error::runtime("template object has no elements"));
        }
        let mut cooked = Vec::with_capacity(quasis.len());
        let mut raw = Vec::with_capacity(quasis.len());
        for quasi in quasis {
            cooked.push(if let Some(value) = quasi.cooked() {
                self.static_string_value(value)?
            } else {
                Value::Undefined
            });
            raw.push(self.static_string_value(quasi.raw())?);
        }
        let raw = self.create_array_from_elements(raw)?;
        let template = self.create_array_from_elements(cooked)?;
        let raw_id = object_id(&raw, "template raw array")?;
        let template_id = object_id(&template, "template cooked array")?;
        let property = self.intern_property_key(TEMPLATE_RAW_PROPERTY)?;
        self.objects.define_property(
            template_id,
            property,
            TEMPLATE_RAW_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(raw),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )?;
        self.objects.freeze(raw_id)?;
        self.objects.freeze(template_id)?;
        Ok(template)
    }
}

fn object_id(value: &Value, description: &str) -> Result<ObjectId> {
    let Value::Object(id) = value else {
        return Err(Error::runtime(format!("{description} is not an object")));
    };
    Ok(*id)
}
