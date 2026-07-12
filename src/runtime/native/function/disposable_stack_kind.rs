pub(in crate::runtime) const DISPOSABLE_STACK_NAME: &str = "DisposableStack";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum DisposableStackFunctionKind {
    Constructor,
    Adopt,
    Defer,
    Dispose,
    DisposedGetter,
    Move,
    Use,
}

impl DisposableStackFunctionKind {
    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::Constructor => 0,
            Self::Adopt => 1,
            Self::Defer => 2,
            Self::Dispose => 3,
            Self::DisposedGetter => 4,
            Self::Move => 5,
            Self::Use => 6,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Constructor | Self::Dispose | Self::DisposedGetter | Self::Move => 0.0,
            Self::Defer | Self::Use => 1.0,
            Self::Adopt => 2.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => DISPOSABLE_STACK_NAME,
            Self::Adopt => "adopt",
            Self::Defer => "defer",
            Self::Dispose => "dispose",
            Self::DisposedGetter => "get disposed",
            Self::Move => "move",
            Self::Use => "use",
        }
    }
}
