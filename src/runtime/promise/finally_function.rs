#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseFinallyFunctionKind {
    Then,
    Catch,
    ValueThunk,
    Thrower,
}

impl PromiseFinallyFunctionKind {
    pub(in crate::runtime) const fn length(self) -> f64 {
        match self {
            Self::Then | Self::Catch => 1.0,
            Self::ValueThunk | Self::Thrower => 0.0,
        }
    }
}
