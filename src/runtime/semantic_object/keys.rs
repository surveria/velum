use crate::{
    error::{Error, Result},
    runtime::Context,
    storage::symbol::JsSymbol,
    value::Value,
};

impl Context {
    /// Spec-shaped `EnumerateObjectProperties` key snapshot. Each prototype is
    /// queried through semantic internal methods so Proxy traps and exotic
    /// property descriptors remain observable.
    pub(in crate::runtime) fn semantic_enumerable_property_keys(
        &mut self,
        target: &Value,
    ) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut current = target.clone();
        loop {
            for key in self.semantic_own_enumerable_string_keys(&current)? {
                self.step()?;
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            let Some(prototype) = self.semantic_get_prototype(&current)? else {
                return Err(Error::type_error(
                    "enumeration target cannot be converted to an object",
                ));
            };
            if matches!(prototype, Value::Null) {
                return Ok(keys);
            }
            current = prototype;
        }
    }

    /// Shared `EnumerableOwnProperties` key selection for string-keyed users.
    pub(in crate::runtime) fn semantic_own_enumerable_string_keys(
        &mut self,
        target: &Value,
    ) -> Result<Vec<String>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return match target {
                Value::String(_) | Value::HeapString(_) => self.enumerable_keys(target),
                Value::Undefined | Value::Null => Err(Error::type_error(
                    "Object.keys target cannot be converted to an object",
                )),
                Value::Bool(_)
                | Value::Number(_)
                | Value::BigInt(_)
                | Value::Symbol(_)
                | Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_) => Ok(Vec::new()),
            };
        };
        match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => self.proxy_enumerable_keys(*id),
            Value::Object(id) => self.objects.own_keys(*id, &self.atoms),
            Value::Function(id) => self.function_enumerable_keys(*id),
            Value::NativeFunction(id) => self.native_function_enumerable_keys(*id),
            Value::HostFunction(_) => Err(Error::runtime(
                "Object.keys target cannot be converted to an object",
            )),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Ok(Vec::new()),
        }
    }

    /// Shared string-key projection of `[[OwnPropertyKeys]]`.
    pub(in crate::runtime) fn semantic_own_property_names(
        &mut self,
        target: &Value,
    ) -> Result<Vec<String>> {
        let keys = self.semantic_own_property_keys(target)?;
        Ok(keys
            .into_iter()
            .filter_map(|key| match key {
                Value::String(name) => Some(name),
                Value::HeapString(name) => Some(name.as_str().to_owned()),
                _ => None,
            })
            .collect())
    }

    /// Shared Symbol-key projection of `[[OwnPropertyKeys]]`.
    pub(in crate::runtime) fn semantic_own_property_symbols(
        &mut self,
        target: &Value,
    ) -> Result<Vec<JsSymbol>> {
        let keys = self.semantic_own_property_keys(target)?;
        Ok(keys
            .into_iter()
            .filter_map(|key| match key {
                Value::Symbol(symbol) => Some(symbol),
                _ => None,
            })
            .collect())
    }

    /// Shared object-like `[[OwnPropertyKeys]]` dispatch preserving both
    /// string and Symbol keys in observable order.
    pub(in crate::runtime) fn semantic_own_property_keys(
        &mut self,
        target: &Value,
    ) -> Result<Vec<Value>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return match target {
                Value::String(_) | Value::HeapString(_) => self
                    .enumerable_keys(target)?
                    .into_iter()
                    .map(|name| self.heap_string_value(&name))
                    .collect(),
                Value::Bool(_)
                | Value::Number(_)
                | Value::BigInt(_)
                | Value::Symbol(_)
                | Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_) => Ok(Vec::new()),
                Value::Undefined | Value::Null => Err(Error::type_error(
                    "own property keys target cannot be converted to an object",
                )),
            };
        };
        match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => self.proxy_own_property_keys(*id),
            Value::Object(id) => self.ordinary_own_property_keys(*id),
            Value::Function(id) => {
                let (names, symbols) = self.function_own_keys(*id)?;
                self.function_property_key_values(names, symbols)
            }
            Value::NativeFunction(id) => {
                let (names, symbols) = self.native_function_own_keys(*id)?;
                self.function_property_key_values(names, symbols)
            }
            Value::HostFunction(_) => Err(Error::runtime(
                "own property keys target cannot be converted to an object",
            )),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Ok(Vec::new()),
        }
    }

    fn ordinary_own_property_keys(&mut self, id: crate::value::ObjectId) -> Result<Vec<Value>> {
        let names = self.objects.own_property_names(id, &self.atoms)?;
        let symbols = self.objects.own_property_symbols(id, &self.symbols)?;
        let mut keys = self.property_name_values(names)?;
        keys.extend(symbols.into_iter().map(Value::Symbol));
        Ok(keys)
    }

    fn property_name_values(&mut self, names: Vec<String>) -> Result<Vec<Value>> {
        names
            .into_iter()
            .map(|name| self.heap_string_value(&name))
            .collect()
    }

    fn function_property_key_values(
        &mut self,
        names: Vec<String>,
        symbols: Vec<crate::storage::symbol::SymbolId>,
    ) -> Result<Vec<Value>> {
        let mut keys = self.property_name_values(names)?;
        for symbol in symbols {
            keys.push(Value::Symbol(self.symbols.get(symbol)?.clone()));
        }
        Ok(keys)
    }
}
