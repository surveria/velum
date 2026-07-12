pub(in crate::runtime) const SHADOW_REALM_NAME: &str = "ShadowRealm";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum ShadowRealmFunctionKind {
    Constructor,
    Evaluate,
    ImportValue,
}

impl ShadowRealmFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Constructor => 0.0,
            Self::Evaluate => 1.0,
            Self::ImportValue => 2.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => SHADOW_REALM_NAME,
            Self::Evaluate => "evaluate",
            Self::ImportValue => "importValue",
        }
    }
}
