use crate::{RetainedValue, api::embedding::Vm, error::Result};

use crate::runtime::EmbeddingObjectPrototype;

#[derive(Clone, Copy, Debug)]
pub enum ObjectPrototypeOption<'value> {
    Default,
    Null,
    Explicit(&'value RetainedValue),
}

impl<'value> ObjectPrototypeOption<'value> {
    pub const fn into_embedding(self) -> EmbeddingObjectPrototype<'value> {
        match self {
            Self::Default => EmbeddingObjectPrototype::Default,
            Self::Null => EmbeddingObjectPrototype::Null,
            Self::Explicit(prototype) => EmbeddingObjectPrototype::Explicit(prototype),
        }
    }
}

/// Creation policy for an ordinary JavaScript object.
#[derive(Clone, Copy, Debug)]
pub struct ObjectOptions<'value> {
    prototype: ObjectPrototypeOption<'value>,
}

impl ObjectOptions<'_> {
    /// Creates options that use the current realm's `Object.prototype`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            prototype: ObjectPrototypeOption::Default,
        }
    }

    /// Creates an object whose `[[Prototype]]` is `null`.
    #[must_use]
    pub const fn with_null_prototype(mut self) -> Self {
        self.prototype = ObjectPrototypeOption::Null;
        self
    }
}

impl<'value> ObjectOptions<'value> {
    /// Uses this VM-local object or retained `null` as the new object's
    /// `[[Prototype]]`.
    #[must_use]
    pub const fn with_prototype(mut self, prototype: &'value RetainedValue) -> Self {
        self.prototype = ObjectPrototypeOption::Explicit(prototype);
        self
    }
}

impl Default for ObjectOptions<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    /// Creates an empty ordinary object with the current realm's default
    /// `Object.prototype` and returns an explicit retained root.
    ///
    /// # Errors
    /// Fails for object or retained-root limits and VM storage failures.
    pub fn create_object(&mut self) -> Result<RetainedValue> {
        self.create_object_with_options(ObjectOptions::new())
    }

    /// Creates an empty ordinary object with the selected semantic prototype.
    ///
    /// Property, descriptor, prototype, Proxy, and later call behavior use
    /// the same ordinary semantic owners as objects created by JavaScript.
    ///
    /// # Errors
    /// Fails for foreign, stale, or primitive prototype handles, object or
    /// retained-root limits, and VM storage failures.
    pub fn create_object_with_options(
        &mut self,
        options: ObjectOptions<'_>,
    ) -> Result<RetainedValue> {
        self.embedding_context_mut()
            .create_embedding_object(options.prototype.into_embedding())
    }
}
