# Test262 Corpus

This directory is reserved for ECMAScript conformance testing based on Test262.

The repository tracks a small active subset under `active/` so CI can measure progress immediately. The full upstream Test262 checkout is materialized under `target/test262` by `scripts/prepare-test262.sh` from the official Test262 repository at commit `64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1`. The preparation step then applies the tracked upstream correction from Test262 commit `f2d1435644797268dca1f7988cad5a4e89ccd8d2`; this keeps the corpus base stable while fixing the `Promise.allSettledKeyed` descriptor test that reused properties after `verifyProperty` deleted them.

The tracked `staging-annex-b-arguments-object` correction aligns two pending SpiderMonkey staging imports with the normative `annexB/language/function-code/block-decl-func-skip-arguments.js` case in the same corpus. The normative case requires the implicit `arguments` object to suppress the Annex B variable replacement, while the two staging assertions required the opposite result for the same grammar shape. Keeping this correction explicit preserves one conforming engine behavior and makes the pinned corpus provenance reproducible.

The runner discovers every `test/**/*.js` file from that pinned checkout and reports total coverage against the full corpus. To keep reports compact, detailed corpus tables list failed cases only; passed and intentionally skipped cases are summarized by count and skip reason.

`manifest.tsv` is the enable-list for upstream Test262 files that the default run executes today. Tests not listed there are counted as skipped in the full-corpus summary, grouped by Test262 area, until the engine supports enough semantics to enable them deliberately.

Manifest modes:

- `run` executes a positive Test262 case and passes when evaluation completes without output or error.
- `negative-parse` executes a negative parse Test262 case and passes only when lexing or parsing fails.
- `skip` records an explicit unsupported feature area with a concrete reason.
