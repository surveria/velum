#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseCombinatorKind {
    All,
    AllKeyed,
    AllSettled,
    AllSettledKeyed,
    Any,
    Race,
}

impl PromiseCombinatorKind {
    pub(in crate::runtime) const ALL: [Self; 6] = [
        Self::All,
        Self::AllKeyed,
        Self::AllSettled,
        Self::AllSettledKeyed,
        Self::Any,
        Self::Race,
    ];

    pub(in crate::runtime) const fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::AllKeyed => "allKeyed",
            Self::AllSettled => "allSettled",
            Self::AllSettledKeyed => "allSettledKeyed",
            Self::Any => "any",
            Self::Race => "race",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseCombinatorElementKind {
    AllResolve,
    AllKeyedResolve,
    AllSettledFulfill,
    AllSettledReject,
    AllSettledKeyedFulfill,
    AllSettledKeyedReject,
    AnyReject,
}
