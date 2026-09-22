//! Opt-in mixed workloads and independently shaped holdouts.

use crate::benchmark_case::BenchmarkCase;

pub(super) fn benchmark_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::prepared(
            "representative_object_transform",
            "tests/corpora/benchmarks/prepared/representative_object_transform.js",
        ),
        BenchmarkCase::prepared(
            "representative_method_dispatch",
            "tests/corpora/benchmarks/prepared/representative_method_dispatch.js",
        ),
        BenchmarkCase::prepared(
            "representative_json_ingestion",
            "tests/corpora/benchmarks/prepared/representative_json_ingestion.js",
        ),
        BenchmarkCase::prepared(
            "representative_string_processing",
            "tests/corpora/benchmarks/prepared/representative_string_processing.js",
        ),
        BenchmarkCase::prepared(
            "representative_collection_index",
            "tests/corpora/benchmarks/prepared/representative_collection_index.js",
        ),
        BenchmarkCase::prepared(
            "representative_tree_allocation",
            "tests/corpora/benchmarks/prepared/representative_tree_allocation.js",
        ),
        BenchmarkCase::prepared(
            "holdout_object_transform",
            "tests/corpora/benchmarks/prepared/holdout_object_transform.js",
        ),
        BenchmarkCase::prepared(
            "holdout_method_dispatch",
            "tests/corpora/benchmarks/prepared/holdout_method_dispatch.js",
        ),
        BenchmarkCase::prepared(
            "holdout_json_ingestion",
            "tests/corpora/benchmarks/prepared/holdout_json_ingestion.js",
        ),
        BenchmarkCase::prepared(
            "holdout_string_processing",
            "tests/corpora/benchmarks/prepared/holdout_string_processing.js",
        ),
        BenchmarkCase::prepared(
            "holdout_collection_index",
            "tests/corpora/benchmarks/prepared/holdout_collection_index.js",
        ),
        BenchmarkCase::prepared(
            "holdout_tree_allocation",
            "tests/corpora/benchmarks/prepared/holdout_tree_allocation.js",
        ),
    ]
}
