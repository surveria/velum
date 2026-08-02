use serde::{Deserialize, Serialize};

use crate::{
    compare::{EngineOutcome, OutcomeStatus},
    reference_gaps::{OracleDecision, OracleUnavailableReason, ReferenceAnalysis},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseClassification {
    Match,
    #[serde(alias = "mismatch")]
    CorrectnessMismatch,
    CorrectnessUnverified,
    #[serde(alias = "slow")]
    PerformanceSlow,
    VelumTimeout,
    VelumCrash,
    VelumResourceLimit,
    Engine262Timeout,
    Engine262Crash,
    Engine262Unsupported,
    V8Timeout,
    V8Crash,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseFinding {
    CorrectnessMismatch,
    CorrectnessUnverified,
    PerformanceSlow,
    VelumTimeout,
    VelumCrash,
    VelumResourceLimit,
    Engine262Timeout,
    Engine262Crash,
    Engine262Unsupported,
    V8Timeout,
    V8Crash,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleEngine {
    Engine262,
    V8Fallback,
}

impl OracleEngine {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Engine262 => "engine262",
            Self::V8Fallback => "v8_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JsErrorClass {
    AggregateError,
    ReferenceError,
    SyntaxError,
    RangeError,
    TypeError,
    EvalError,
    UriError,
    Error,
}

impl JsErrorClass {
    fn from_name(name: Option<&str>) -> Option<Self> {
        match name {
            Some("AggregateError") => Some(Self::AggregateError),
            Some("ReferenceError") => Some(Self::ReferenceError),
            Some("SyntaxError") => Some(Self::SyntaxError),
            Some("RangeError") => Some(Self::RangeError),
            Some("TypeError") => Some(Self::TypeError),
            Some("EvalError") => Some(Self::EvalError),
            Some("URIError") => Some(Self::UriError),
            Some("Error") => Some(Self::Error),
            Some(_) | None => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum EquivalenceBasis {
    SuccessfulOutputSha256,
    JsErrorClass { class: JsErrorClass },
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutcomeDifference {
    Status {
        velum: OutcomeStatus,
        oracle: OutcomeStatus,
    },
    SuccessfulOutput {
        velum_sha256: String,
        oracle_sha256: String,
        velum_bytes: u64,
        oracle_bytes: u64,
    },
    JsErrorClass {
        velum: JsErrorClass,
        oracle: JsErrorClass,
        velum_name: String,
        oracle_name: String,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum UnverifiedReason {
    NoReliableOracle {
        reasons: Vec<OracleUnavailableReason>,
    },
    VelumIncomplete {
        status: OutcomeStatus,
    },
    VelumResourceLimit,
    OracleIncomplete {
        oracle: OracleEngine,
        status: OutcomeStatus,
    },
    UnclassifiedJsError {
        oracle: OracleEngine,
        velum_name: Option<String>,
        oracle_name: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum CorrectnessEvaluation {
    Equivalent {
        oracle: OracleEngine,
        basis: EquivalenceBasis,
    },
    Mismatch {
        oracle: OracleEngine,
        difference: OutcomeDifference,
    },
    Unverified {
        reason: UnverifiedReason,
    },
    #[default]
    LegacyUnspecified,
}

impl CorrectnessEvaluation {
    #[must_use]
    pub const fn is_mismatch(&self) -> bool {
        matches!(self, Self::Mismatch { .. })
    }

    #[must_use]
    pub const fn is_unverified(&self) -> bool {
        matches!(self, Self::Unverified { .. })
    }

    #[must_use]
    pub const fn oracle(&self) -> Option<OracleEngine> {
        match self {
            Self::Equivalent { oracle, .. }
            | Self::Mismatch { oracle, .. }
            | Self::Unverified {
                reason: UnverifiedReason::OracleIncomplete { oracle, .. },
            } => Some(*oracle),
            Self::Unverified { .. } | Self::LegacyUnspecified => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CompletedOutcomeComparison {
    Equivalent(EquivalenceBasis),
    Different(OutcomeDifference),
    UnclassifiedJsError {
        left_name: Option<String>,
        right_name: Option<String>,
    },
}

#[must_use]
pub fn evaluate(
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reference: &ReferenceAnalysis,
    velum_resource_limit: bool,
) -> CorrectnessEvaluation {
    if velum_resource_limit {
        return CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::VelumResourceLimit,
        };
    }
    if !velum.is_completed() {
        return CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::VelumIncomplete {
                status: velum.status,
            },
        };
    }
    let (oracle_engine, oracle_outcome) = match &reference.oracle {
        OracleDecision::Engine262 => (OracleEngine::Engine262, engine262),
        OracleDecision::V8Fallback => (OracleEngine::V8Fallback, v8),
        OracleDecision::Unavailable { reasons } => {
            return CorrectnessEvaluation::Unverified {
                reason: UnverifiedReason::NoReliableOracle {
                    reasons: reasons.clone(),
                },
            };
        }
        OracleDecision::LegacyUnspecified => return CorrectnessEvaluation::LegacyUnspecified,
    };
    if !oracle_outcome.is_completed() {
        return CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::OracleIncomplete {
                oracle: oracle_engine,
                status: oracle_outcome.status,
            },
        };
    }
    match compare_completed_outcomes(velum, oracle_outcome) {
        CompletedOutcomeComparison::Equivalent(basis) => CorrectnessEvaluation::Equivalent {
            oracle: oracle_engine,
            basis,
        },
        CompletedOutcomeComparison::Different(difference) => CorrectnessEvaluation::Mismatch {
            oracle: oracle_engine,
            difference,
        },
        CompletedOutcomeComparison::UnclassifiedJsError {
            left_name,
            right_name,
        } => CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::UnclassifiedJsError {
                oracle: oracle_engine,
                velum_name: left_name,
                oracle_name: right_name,
            },
        },
    }
}

#[must_use]
pub fn compare_completed_outcomes(
    left: &EngineOutcome,
    right: &EngineOutcome,
) -> CompletedOutcomeComparison {
    if left.status != right.status {
        return CompletedOutcomeComparison::Different(OutcomeDifference::Status {
            velum: left.status,
            oracle: right.status,
        });
    }
    match left.status {
        OutcomeStatus::Ok if left.stdout_sha256 == right.stdout_sha256 => {
            CompletedOutcomeComparison::Equivalent(EquivalenceBasis::SuccessfulOutputSha256)
        }
        OutcomeStatus::Ok => {
            CompletedOutcomeComparison::Different(OutcomeDifference::SuccessfulOutput {
                velum_sha256: left.stdout_sha256.clone(),
                oracle_sha256: right.stdout_sha256.clone(),
                velum_bytes: left.stdout_bytes,
                oracle_bytes: right.stdout_bytes,
            })
        }
        OutcomeStatus::JsError => compare_js_errors(left, right),
        OutcomeStatus::Timeout | OutcomeStatus::Crash => {
            CompletedOutcomeComparison::Different(OutcomeDifference::Status {
                velum: left.status,
                oracle: right.status,
            })
        }
    }
}

#[must_use]
pub fn outcomes_equivalent(left: &EngineOutcome, right: &EngineOutcome) -> bool {
    matches!(
        compare_completed_outcomes(left, right),
        CompletedOutcomeComparison::Equivalent(_)
    )
}

fn compare_js_errors(left: &EngineOutcome, right: &EngineOutcome) -> CompletedOutcomeComparison {
    let left_class = JsErrorClass::from_name(left.error_name.as_deref());
    let right_class = JsErrorClass::from_name(right.error_name.as_deref());
    let (Some(left_class), Some(right_class)) = (left_class, right_class) else {
        return CompletedOutcomeComparison::UnclassifiedJsError {
            left_name: left.error_name.clone(),
            right_name: right.error_name.clone(),
        };
    };
    if left_class == right_class {
        return CompletedOutcomeComparison::Equivalent(EquivalenceBasis::JsErrorClass {
            class: left_class,
        });
    }
    CompletedOutcomeComparison::Different(OutcomeDifference::JsErrorClass {
        velum: left_class,
        oracle: right_class,
        velum_name: left.error_name.clone().unwrap_or_default(),
        oracle_name: right.error_name.clone().unwrap_or_default(),
    })
}
