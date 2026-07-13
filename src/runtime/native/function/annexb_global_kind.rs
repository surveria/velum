#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum AnnexBGlobalFunctionKind {
    Escape,
    Unescape,
}

impl AnnexBGlobalFunctionKind {
    pub(in crate::runtime) fn from_name(name: &str) -> Option<Self> {
        match name {
            "escape" => Some(Self::Escape),
            "unescape" => Some(Self::Unescape),
            _ => None,
        }
    }

    pub(in crate::runtime) const fn index(self) -> usize {
        match self {
            Self::Escape => 0,
            Self::Unescape => 1,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Escape => "escape",
            Self::Unescape => "unescape",
        }
    }
}
