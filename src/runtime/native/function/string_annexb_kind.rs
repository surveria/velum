#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum StringAnnexBFunctionKind {
    Anchor,
    Big,
    Blink,
    Bold,
    Fixed,
    FontColor,
    FontSize,
    Italics,
    Link,
    Small,
    Strike,
    Sub,
    Substr,
    Sup,
}

impl StringAnnexBFunctionKind {
    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::Big => "big",
            Self::Blink => "blink",
            Self::Bold => "bold",
            Self::Fixed => "fixed",
            Self::FontColor => "fontcolor",
            Self::FontSize => "fontsize",
            Self::Italics => "italics",
            Self::Link => "link",
            Self::Small => "small",
            Self::Strike => "strike",
            Self::Sub => "sub",
            Self::Substr => "substr",
            Self::Sup => "sup",
        }
    }

    pub(super) const fn length(self) -> f64 {
        match self {
            Self::Big
            | Self::Blink
            | Self::Bold
            | Self::Fixed
            | Self::Italics
            | Self::Small
            | Self::Strike
            | Self::Sub
            | Self::Sup => 0.0,
            Self::Anchor | Self::FontColor | Self::FontSize | Self::Link => 1.0,
            Self::Substr => 2.0,
        }
    }
}
