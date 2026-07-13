#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum SuperContext {
    Forbidden,
    Property,
    PropertyAndCall,
}

impl SuperContext {
    pub(super) const fn new(allow_property: bool, allow_call: bool) -> Self {
        if allow_call {
            Self::PropertyAndCall
        } else if allow_property {
            Self::Property
        } else {
            Self::Forbidden
        }
    }

    pub(super) const fn allows_property(self) -> bool {
        matches!(self, Self::Property | Self::PropertyAndCall)
    }

    pub(super) const fn allows_call(self) -> bool {
        matches!(self, Self::PropertyAndCall)
    }
}
