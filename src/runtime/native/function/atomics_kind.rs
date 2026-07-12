pub(in crate::runtime::native) const ATOMICS_NAME: &str = "Atomics";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum AtomicsFunctionKind {
    Add,
    And,
    CompareExchange,
    Exchange,
    IsLockFree,
    Load,
    Notify,
    Or,
    Pause,
    Store,
    Sub,
    Wait,
    WaitAsync,
    Xor,
}

impl AtomicsFunctionKind {
    pub(in crate::runtime::native) const ALL: [Self; 14] = [
        Self::Add,
        Self::And,
        Self::CompareExchange,
        Self::Exchange,
        Self::IsLockFree,
        Self::Load,
        Self::Notify,
        Self::Or,
        Self::Pause,
        Self::Store,
        Self::Sub,
        Self::Wait,
        Self::WaitAsync,
        Self::Xor,
    ];

    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::Add => 0,
            Self::And => 1,
            Self::CompareExchange => 2,
            Self::Exchange => 3,
            Self::IsLockFree => 4,
            Self::Load => 5,
            Self::Notify => 6,
            Self::Or => 7,
            Self::Pause => 8,
            Self::Store => 9,
            Self::Sub => 10,
            Self::Wait => 11,
            Self::WaitAsync => 12,
            Self::Xor => 13,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Pause => 0.0,
            Self::IsLockFree => 1.0,
            Self::Load => 2.0,
            Self::CompareExchange | Self::Wait | Self::WaitAsync => 4.0,
            Self::Add
            | Self::And
            | Self::Exchange
            | Self::Or
            | Self::Notify
            | Self::Store
            | Self::Sub
            | Self::Xor => 3.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::And => "and",
            Self::CompareExchange => "compareExchange",
            Self::Exchange => "exchange",
            Self::IsLockFree => "isLockFree",
            Self::Load => "load",
            Self::Notify => "notify",
            Self::Or => "or",
            Self::Pause => "pause",
            Self::Store => "store",
            Self::Sub => "sub",
            Self::Wait => "wait",
            Self::WaitAsync => "waitAsync",
            Self::Xor => "xor",
        }
    }
}
