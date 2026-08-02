use serde::{Deserialize, Serialize};

use crate::compare::EngineOutcome;
use crate::reference_gap_date as date;
use crate::reference_gap_globals as globals;
use crate::reference_gap_locale as locale;
use crate::reference_gap_predicates as predicates;
use crate::reference_gap_rab as rab;
use crate::reference_gap_syntax as syntax_gaps;
use crate::reference_gap_v8 as v8_gaps;

pub use crate::reference_gap_predicates::outcomes_equivalent;
#[cfg(test)]
pub use crate::reference_gap_predicates::source_contains_resource_management_syntax;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceGapReason {
    MissingEngine262Global,
    MissingAnnexBEscapeGlobal,
    ResizableArrayBufferReferenceDivergence,
    SuperPropertySyntaxGap,
    ResourceManagementSyntaxUnsupported,
    ResourceManagementSymbolsUnsupported,
    AnnexBStringLegacyMethodMissing,
    AnnexBStringLegacyWithV8Alignment,
    AnnexBStringLegacyV8FallbackUnavailable,
    AnnexBRegexpCompileMissing,
    RegexpCompileWithV8Alignment,
    ImmutableArrayBufferMethodUnsupported,
    ImmutableArrayBufferMethodWithV8Alignment,
    LegacyDateMethodMissing,
    LegacyDateCallGap,
    DateTemporalInstantUnsupported,
    LocaleValidationGap,
    StringCaseLocaleValidationGap,
    LocaleCompareValidationGap,
    TemplateLiteralOctalEscapeGap,
    LocaleValidationWithV8Alignment,
    GsabPreventExtensionsWithoutOracle,
    WebAssemblyHostApiGap,
    SharedArrayBufferAlignmentConflict,
    ResizableArrayBufferAlignmentConflict,
    LegacyDecimalEscapeWithV8Alignment,
    InvalidDecimalDigitsWithV8Alignment,
    InvalidIdentityEscapeWithV8Alignment,
    InvalidQuantifierWithV8Alignment,
    UserConstructorThrowWithV8Alignment,
    LegacyControlEscapeWithV8Alignment,
    LegacyQuantifiedLookaheadWithV8Alignment,
    ClosingBracketRegexpWithV8Alignment,
    SharedArrayBufferZeroLengthSlice,
    NativeFunctionThrowStringification,
    FuzzilliIntrospectionUnstable,
    Engine262SyntaxErrorReferenceDivergence,
}

impl ReferenceGapReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingEngine262Global => "missing_engine262_global",
            Self::MissingAnnexBEscapeGlobal => "missing_annex_b_escape_global",
            Self::ResizableArrayBufferReferenceDivergence => {
                "resizable_array_buffer_reference_divergence"
            }
            Self::SuperPropertySyntaxGap => "super_property_syntax_gap",
            Self::ResourceManagementSyntaxUnsupported => "resource_management_syntax_unsupported",
            Self::ResourceManagementSymbolsUnsupported => "resource_management_symbols_unsupported",
            Self::AnnexBStringLegacyMethodMissing => "annex_b_string_legacy_method_missing",
            Self::AnnexBStringLegacyWithV8Alignment => "annex_b_string_legacy_with_v8_alignment",
            Self::AnnexBStringLegacyV8FallbackUnavailable => {
                "annex_b_string_legacy_v8_fallback_unavailable"
            }
            Self::AnnexBRegexpCompileMissing => "annex_b_regexp_compile_missing",
            Self::RegexpCompileWithV8Alignment => "regexp_compile_with_v8_alignment",
            Self::ImmutableArrayBufferMethodUnsupported => {
                "immutable_array_buffer_method_unsupported"
            }
            Self::ImmutableArrayBufferMethodWithV8Alignment => {
                "immutable_array_buffer_method_with_v8_alignment"
            }
            Self::LegacyDateMethodMissing => "legacy_date_method_missing",
            Self::LegacyDateCallGap => "legacy_date_call_gap",
            Self::DateTemporalInstantUnsupported => "date_temporal_instant_unsupported",
            Self::LocaleValidationGap => "locale_validation_gap",
            Self::StringCaseLocaleValidationGap => "string_case_locale_validation_gap",
            Self::LocaleCompareValidationGap => "locale_compare_validation_gap",
            Self::TemplateLiteralOctalEscapeGap => "template_literal_octal_escape_gap",
            Self::LocaleValidationWithV8Alignment => "locale_validation_with_v8_alignment",
            Self::GsabPreventExtensionsWithoutOracle => "gsab_prevent_extensions_without_oracle",
            Self::WebAssemblyHostApiGap => "webassembly_host_api_gap",
            Self::SharedArrayBufferAlignmentConflict => "shared_array_buffer_alignment_conflict",
            Self::ResizableArrayBufferAlignmentConflict => {
                "resizable_array_buffer_alignment_conflict"
            }
            Self::LegacyDecimalEscapeWithV8Alignment => "legacy_decimal_escape_with_v8_alignment",
            Self::InvalidDecimalDigitsWithV8Alignment => "invalid_decimal_digits_with_v8_alignment",
            Self::InvalidIdentityEscapeWithV8Alignment => {
                "invalid_identity_escape_with_v8_alignment"
            }
            Self::InvalidQuantifierWithV8Alignment => "invalid_quantifier_with_v8_alignment",
            Self::UserConstructorThrowWithV8Alignment => "user_constructor_throw_with_v8_alignment",
            Self::LegacyControlEscapeWithV8Alignment => "legacy_control_escape_with_v8_alignment",
            Self::LegacyQuantifiedLookaheadWithV8Alignment => {
                "legacy_quantified_lookahead_with_v8_alignment"
            }
            Self::ClosingBracketRegexpWithV8Alignment => "closing_bracket_regexp_with_v8_alignment",
            Self::SharedArrayBufferZeroLengthSlice => "shared_array_buffer_zero_length_slice",
            Self::NativeFunctionThrowStringification => "native_function_throw_stringification",
            Self::FuzzilliIntrospectionUnstable => "fuzzilli_introspection_unstable",
            Self::Engine262SyntaxErrorReferenceDivergence => {
                "engine262_syntax_error_reference_divergence"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleUnavailableReason {
    ResourceManagementSyntaxUnsupported,
    WebAssemblyHostApiUnsupported,
    SharedArrayBufferAlignmentConflict,
    ResizableArrayBufferAlignmentConflict,
    LegacyDecimalEscapeWithV8Alignment,
    InvalidDecimalDigitsWithV8Alignment,
    InvalidIdentityEscapeWithV8Alignment,
    InvalidQuantifierWithV8Alignment,
    UserConstructorThrowWithV8Alignment,
    LegacyControlEscapeWithV8Alignment,
    LegacyQuantifiedLookaheadWithV8Alignment,
    ClosingBracketRegexpWithV8Alignment,
    AnnexBStringLegacyWithV8Alignment,
    RegexpCompileWithV8Alignment,
    LocaleValidationWithV8Alignment,
    GsabPreventExtensionsConflict,
    AnnexBStringLegacyV8FallbackUnavailable,
    SharedArrayBufferZeroLengthSliceConflict,
    NativeFunctionThrowStringificationUnstable,
    FuzzilliIntrospectionUnstable,
    ResourceManagementSymbolsAmbiguous,
    ImmutableArrayBufferMethodMissingFromReferences,
    ImmutableArrayBufferMethodWithV8Alignment,
    DateTemporalInstantMissingFromReferences,
    V8MissingMapGroupBy,
    V8MissingMapGetOrInsert,
    V8MissingDateTemporalInstant,
    V8MissingArrayFromAsync,
    V8MissingArrayBufferTransfer,
    V8MissingArrayBufferTransferToFixedLength,
    V8FallbackFeatureUnavailable,
    LegacyRecord,
}

impl OracleUnavailableReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceManagementSyntaxUnsupported => "resource_management_syntax_unsupported",
            Self::WebAssemblyHostApiUnsupported => "webassembly_host_api_unsupported",
            Self::SharedArrayBufferAlignmentConflict => "shared_array_buffer_alignment_conflict",
            Self::ResizableArrayBufferAlignmentConflict => {
                "resizable_array_buffer_alignment_conflict"
            }
            Self::LegacyDecimalEscapeWithV8Alignment => "legacy_decimal_escape_with_v8_alignment",
            Self::InvalidDecimalDigitsWithV8Alignment => "invalid_decimal_digits_with_v8_alignment",
            Self::InvalidIdentityEscapeWithV8Alignment => {
                "invalid_identity_escape_with_v8_alignment"
            }
            Self::InvalidQuantifierWithV8Alignment => "invalid_quantifier_with_v8_alignment",
            Self::UserConstructorThrowWithV8Alignment => "user_constructor_throw_with_v8_alignment",
            Self::LegacyControlEscapeWithV8Alignment => "legacy_control_escape_with_v8_alignment",
            Self::LegacyQuantifiedLookaheadWithV8Alignment => {
                "legacy_quantified_lookahead_with_v8_alignment"
            }
            Self::ClosingBracketRegexpWithV8Alignment => "closing_bracket_regexp_with_v8_alignment",
            Self::AnnexBStringLegacyWithV8Alignment => "annex_b_string_legacy_with_v8_alignment",
            Self::RegexpCompileWithV8Alignment => "regexp_compile_with_v8_alignment",
            Self::LocaleValidationWithV8Alignment => "locale_validation_with_v8_alignment",
            Self::GsabPreventExtensionsConflict => "gsab_prevent_extensions_conflict",
            Self::AnnexBStringLegacyV8FallbackUnavailable => {
                "annex_b_string_legacy_v8_fallback_unavailable"
            }
            Self::SharedArrayBufferZeroLengthSliceConflict => {
                "shared_array_buffer_zero_length_slice_conflict"
            }
            Self::NativeFunctionThrowStringificationUnstable => {
                "native_function_throw_stringification_unstable"
            }
            Self::FuzzilliIntrospectionUnstable => "fuzzilli_introspection_unstable",
            Self::ResourceManagementSymbolsAmbiguous => "resource_management_symbols_ambiguous",
            Self::ImmutableArrayBufferMethodMissingFromReferences => {
                "immutable_array_buffer_method_missing_from_references"
            }
            Self::ImmutableArrayBufferMethodWithV8Alignment => {
                "immutable_array_buffer_method_with_v8_alignment"
            }
            Self::DateTemporalInstantMissingFromReferences => {
                "date_temporal_instant_missing_from_references"
            }
            Self::V8MissingMapGroupBy => "v8_missing_map_group_by",
            Self::V8MissingMapGetOrInsert => "v8_missing_map_get_or_insert",
            Self::V8MissingDateTemporalInstant => "v8_missing_date_temporal_instant",
            Self::V8MissingArrayFromAsync => "v8_missing_array_from_async",
            Self::V8MissingArrayBufferTransfer => "v8_missing_array_buffer_transfer",
            Self::V8MissingArrayBufferTransferToFixedLength => {
                "v8_missing_array_buffer_transfer_to_fixed_length"
            }
            Self::V8FallbackFeatureUnavailable => "v8_fallback_feature_unavailable",
            Self::LegacyRecord => "legacy_record",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "selection", rename_all = "snake_case")]
pub enum OracleDecision {
    Engine262,
    V8Fallback,
    Unavailable {
        reasons: Vec<OracleUnavailableReason>,
    },
    LegacyUnspecified,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct ReferenceAnalysis {
    pub engine262_gaps: Vec<ReferenceGapReason>,
    pub oracle: OracleDecision,
}

impl Default for ReferenceAnalysis {
    fn default() -> Self {
        Self {
            engine262_gaps: Vec::new(),
            oracle: OracleDecision::LegacyUnspecified,
        }
    }
}

#[must_use]
pub fn analyze(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> ReferenceAnalysis {
    let engine262_gaps = engine262_gap_reasons(source, velum, engine262, v8);
    let oracle = oracle_decision(source, engine262, v8, !engine262_gaps.is_empty());
    ReferenceAnalysis {
        engine262_gaps,
        oracle,
    }
}

#[must_use]
pub fn is_engine262_unsupported(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    !engine262_gap_reasons(source, velum, engine262, v8).is_empty()
}

#[must_use]
pub fn correctness_oracle<'a>(
    source: &str,
    engine262: &'a EngineOutcome,
    v8: &'a EngineOutcome,
    engine262_unsupported: bool,
) -> Option<&'a EngineOutcome> {
    match oracle_decision(source, engine262, v8, engine262_unsupported) {
        OracleDecision::Engine262 => Some(engine262),
        OracleDecision::V8Fallback => Some(v8),
        OracleDecision::Unavailable { .. } | OracleDecision::LegacyUnspecified => None,
    }
}

fn engine262_gap_reasons(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> Vec<ReferenceGapReason> {
    let mut reasons = Vec::new();
    collect_engine262_language_gaps(source, velum, engine262, v8, &mut reasons);
    collect_engine262_library_gaps(source, velum, engine262, v8, &mut reasons);
    collect_engine262_interaction_gaps(source, velum, engine262, v8, &mut reasons);
    reasons
}

fn collect_engine262_language_gaps(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reasons: &mut Vec<ReferenceGapReason>,
) {
    push_reason(
        reasons,
        ReferenceGapReason::MissingEngine262Global,
        predicates::is_engine262_missing_global(engine262),
    );
    push_reason(
        reasons,
        ReferenceGapReason::MissingAnnexBEscapeGlobal,
        globals::is_engine262_missing_annex_b_escape_global(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::ResizableArrayBufferReferenceDivergence,
        predicates::is_resizable_array_buffer_reference_divergence(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::SuperPropertySyntaxGap,
        syntax_gaps::is_engine262_super_property_syntax_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::ResourceManagementSyntaxUnsupported,
        predicates::is_reference_unsupported_resource_management_syntax(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::ResourceManagementSymbolsUnsupported,
        predicates::is_reference_unsupported_resource_management_symbols(
            source, velum, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::AnnexBStringLegacyMethodMissing,
        predicates::is_engine262_missing_annex_b_string_legacy_method(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::AnnexBStringLegacyWithV8Alignment,
        predicates::is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::AnnexBStringLegacyV8FallbackUnavailable,
        predicates::is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::AnnexBRegexpCompileMissing,
        predicates::is_engine262_missing_annex_b_regexp_compile_method(source, velum, engine262),
    );
    push_reason(
        reasons,
        ReferenceGapReason::RegexpCompileWithV8Alignment,
        rab::is_regexp_compile_with_v8_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::ImmutableArrayBufferMethodUnsupported,
        predicates::is_reference_unsupported_immutable_array_buffer_method(
            source, velum, engine262, v8,
        ),
    );
}

fn collect_engine262_library_gaps(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reasons: &mut Vec<ReferenceGapReason>,
) {
    push_reason(
        reasons,
        ReferenceGapReason::ImmutableArrayBufferMethodWithV8Alignment,
        predicates::is_immutable_array_buffer_method_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LegacyDateMethodMissing,
        date::is_engine262_missing_legacy_date_method(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LegacyDateCallGap,
        date::is_engine262_legacy_date_call_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::DateTemporalInstantUnsupported,
        predicates::is_reference_unsupported_date_temporal_instant_method(
            source, velum, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LocaleValidationGap,
        predicates::is_engine262_locale_validation_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::StringCaseLocaleValidationGap,
        locale::is_engine262_string_case_locale_validation_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LocaleCompareValidationGap,
        locale::is_engine262_locale_compare_validation_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::TemplateLiteralOctalEscapeGap,
        predicates::is_engine262_template_literal_octal_escape_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LocaleValidationWithV8Alignment,
        rab::is_locale_validation_gap_with_v8_alignment(source, velum, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::GsabPreventExtensionsWithoutOracle,
        rab::is_gsab_length_tracking_prevent_extensions_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::WebAssemblyHostApiGap,
        predicates::is_webassembly_host_api_gap(source, velum, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::SharedArrayBufferAlignmentConflict,
        predicates::is_shared_array_buffer_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::ResizableArrayBufferAlignmentConflict,
        predicates::is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8),
    );
}

fn collect_engine262_interaction_gaps(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reasons: &mut Vec<ReferenceGapReason>,
) {
    push_reason(
        reasons,
        ReferenceGapReason::LegacyDecimalEscapeWithV8Alignment,
        predicates::is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::InvalidDecimalDigitsWithV8Alignment,
        predicates::is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::InvalidIdentityEscapeWithV8Alignment,
        predicates::is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::InvalidQuantifierWithV8Alignment,
        predicates::is_engine262_invalid_quantifier_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::UserConstructorThrowWithV8Alignment,
        rab::is_user_constructor_throw_with_v8_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LegacyControlEscapeWithV8Alignment,
        predicates::is_legacy_control_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::LegacyQuantifiedLookaheadWithV8Alignment,
        predicates::is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::ClosingBracketRegexpWithV8Alignment,
        predicates::is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        ReferenceGapReason::SharedArrayBufferZeroLengthSlice,
        predicates::is_shared_array_buffer_zero_length_slice_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::NativeFunctionThrowStringification,
        predicates::is_native_function_throw_stringification_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::FuzzilliIntrospectionUnstable,
        predicates::is_fuzzilli_introspection_reference_unstable(source, engine262, v8),
    );
    push_reason(
        reasons,
        ReferenceGapReason::Engine262SyntaxErrorReferenceDivergence,
        predicates::is_engine262_syntax_error_reference_divergence(velum, engine262, v8),
    );
}

fn oracle_decision(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    engine262_unsupported: bool,
) -> OracleDecision {
    if !engine262_unsupported {
        return OracleDecision::Engine262;
    }
    let reasons = oracle_unavailable_reasons(source, engine262, v8);
    if reasons.is_empty() {
        OracleDecision::V8Fallback
    } else {
        OracleDecision::Unavailable { reasons }
    }
}

fn oracle_unavailable_reasons(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> Vec<OracleUnavailableReason> {
    let mut reasons = Vec::new();
    collect_oracle_alignment_gaps(source, engine262, v8, &mut reasons);
    collect_oracle_ambiguity_gaps(source, engine262, v8, &mut reasons);
    collect_oracle_feature_gaps(source, engine262, v8, &mut reasons);
    reasons
}

fn collect_oracle_alignment_gaps(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reasons: &mut Vec<OracleUnavailableReason>,
) {
    push_reason(
        reasons,
        OracleUnavailableReason::ResourceManagementSyntaxUnsupported,
        predicates::is_reference_unsupported_resource_management_syntax(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::WebAssemblyHostApiUnsupported,
        predicates::is_webassembly_host_api_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::SharedArrayBufferAlignmentConflict,
        predicates::is_shared_array_buffer_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::ResizableArrayBufferAlignmentConflict,
        predicates::is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::LegacyDecimalEscapeWithV8Alignment,
        predicates::is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::InvalidDecimalDigitsWithV8Alignment,
        predicates::is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::InvalidIdentityEscapeWithV8Alignment,
        predicates::is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::InvalidQuantifierWithV8Alignment,
        predicates::is_engine262_invalid_quantifier_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::UserConstructorThrowWithV8Alignment,
        rab::is_user_constructor_throw_with_v8_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::LegacyControlEscapeWithV8Alignment,
        predicates::is_legacy_control_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::LegacyQuantifiedLookaheadWithV8Alignment,
        predicates::is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
}

fn collect_oracle_ambiguity_gaps(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reasons: &mut Vec<OracleUnavailableReason>,
) {
    push_reason(
        reasons,
        OracleUnavailableReason::ClosingBracketRegexpWithV8Alignment,
        predicates::is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::AnnexBStringLegacyWithV8Alignment,
        predicates::is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::RegexpCompileWithV8Alignment,
        rab::is_regexp_compile_with_v8_alignment_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::LocaleValidationWithV8Alignment,
        rab::is_locale_validation_with_v8_alignment_without_oracle(source, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::GsabPreventExtensionsConflict,
        rab::is_gsab_length_tracking_prevent_extensions_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::AnnexBStringLegacyV8FallbackUnavailable,
        predicates::is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::SharedArrayBufferZeroLengthSliceConflict,
        predicates::is_shared_array_buffer_zero_length_slice_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::NativeFunctionThrowStringificationUnstable,
        predicates::is_native_function_throw_stringification_without_oracle(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::FuzzilliIntrospectionUnstable,
        predicates::is_fuzzilli_introspection_reference_unstable(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::ResourceManagementSymbolsAmbiguous,
        predicates::source_contains_resource_management_symbol_access(source)
            && predicates::references_complete_equivalently(engine262, v8),
    );
}

fn collect_oracle_feature_gaps(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    reasons: &mut Vec<OracleUnavailableReason>,
) {
    push_reason(
        reasons,
        OracleUnavailableReason::ImmutableArrayBufferMethodMissingFromReferences,
        predicates::is_reference_missing_immutable_array_buffer_method(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::ImmutableArrayBufferMethodWithV8Alignment,
        predicates::is_immutable_array_buffer_method_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        ),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::DateTemporalInstantMissingFromReferences,
        predicates::is_reference_missing_date_temporal_instant_method(source, engine262, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8MissingMapGroupBy,
        v8_gaps::is_v8_missing_map_group_by(v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8MissingMapGetOrInsert,
        v8_gaps::is_v8_missing_map_get_or_insert(source, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8MissingDateTemporalInstant,
        v8_gaps::is_v8_missing_date_to_temporal_instant(source, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8MissingArrayFromAsync,
        v8_gaps::is_v8_missing_array_from_async(source, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8MissingArrayBufferTransfer,
        v8_gaps::is_v8_missing_array_buffer_transfer(source, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8MissingArrayBufferTransferToFixedLength,
        v8_gaps::is_v8_missing_array_buffer_transfer_to_fixed_length(source, v8),
    );
    push_reason(
        reasons,
        OracleUnavailableReason::V8FallbackFeatureUnavailable,
        predicates::is_v8_fallback_unavailable(v8),
    );
}

fn push_reason<T: Copy>(reasons: &mut Vec<T>, reason: T, matches: bool) {
    if matches {
        reasons.push(reason);
    }
}
