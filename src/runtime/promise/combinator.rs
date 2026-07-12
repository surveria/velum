#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseCombinatorKind {
    All,
    AllSettled,
    Any,
    Race,
}

impl PromiseCombinatorKind {
    pub(in crate::runtime) const ALL: [Self; 4] =
        [Self::All, Self::AllSettled, Self::Any, Self::Race];

    pub(in crate::runtime) const fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::AllSettled => "allSettled",
            Self::Any => "any",
            Self::Race => "race",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseCombinatorElementKind {
    AllResolve,
    AllSettledFulfill,
    AllSettledReject,
    AnyReject,
}
