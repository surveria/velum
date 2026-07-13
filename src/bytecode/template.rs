use crate::syntax::StaticString;

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeTemplateElement {
    cooked: Option<StaticString>,
    raw: StaticString,
}

impl BytecodeTemplateElement {
    pub(crate) const fn new(cooked: Option<StaticString>, raw: StaticString) -> Self {
        Self { cooked, raw }
    }

    pub const fn cooked(&self) -> Option<&StaticString> {
        self.cooked.as_ref()
    }

    pub const fn raw(&self) -> &StaticString {
        &self.raw
    }
}
