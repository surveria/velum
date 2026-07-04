# Test262 Corpus

This directory is reserved for ECMAScript conformance testing based on Test262.

The repository tracks a small active subset under `active/` so CI can measure progress immediately. The full upstream Test262 checkout should be provided externally through runner configuration instead of being vendored into this repository by default.

The runner reports both implemented active cases and unavailable corpus areas as explicit rows, so skipped coverage remains visible.
