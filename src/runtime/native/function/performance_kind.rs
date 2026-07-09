use super::kind::NativeFunctionKind;

pub(in crate::runtime::native) const PERFORMANCE_NAME: &str = "performance";
pub(in crate::runtime::native) const PERFORMANCE_NOW_NAME: &str = "now";

const PERFORMANCE_NOW_LENGTH: f64 = 0.0;

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn performance_length(self) -> Option<f64> {
        match self {
            Self::PerformanceNow => Some(PERFORMANCE_NOW_LENGTH),
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn performance_name(
        self,
    ) -> Option<&'static str> {
        match self {
            Self::PerformanceNow => Some(PERFORMANCE_NOW_NAME),
            _ => None,
        }
    }
}
