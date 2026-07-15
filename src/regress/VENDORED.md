# Vendored regress

This directory contains the complete library source of `regress` 0.11.1, an
ECMAScript-oriented regular expression compiler and executor maintained at
<https://github.com/ridiculousfish/regress>.

## Provenance

- crates.io package: `regress` 0.11.1
- upstream Git commit recorded by the package: `7e64ad5e6807b5503e5cc97a79e0f129b23c556b`
- crates.io archive SHA-256: `158a764437582235e3501f683b93a0a6f8d825d04a789dbe5ed30b8799b8908a`
- imported into Velum: 2026-07-11
- license: MIT OR Apache-2.0

`LICENSE-MIT`, `LICENSE-APACHE`, `README.md`, `Cargo.toml.orig`, and every
library source file were imported from the published package. The tracked
`VENDORED-SOURCE-SHA256SUMS` file records the current reviewed source state.

## Local integration

The root engine uses this crate through a local `path` dependency with default
features disabled and `prohibit-unsafe`, `std`, and `utf16` enabled. The local
`Cargo.toml` removes published-package test declarations and marks the package
as non-publishable; `Cargo.toml.orig` preserves the upstream development
manifest.

The source is intentionally kept in its upstream file layout so future fixes
can be compared and forwarded. Large upstream and generated Rust files are
covered by `scripts/check-vendored-regress.sh` and its source manifest instead
of the project-owned 800-line source-file gate.

## Local deviations

- `unicode.rs` and `parse.rs` recognize the ECMAScript `Unknown` and `Zzzz`
  aliases for both `Script` and `Script_Extensions`. The resulting character
  class is the union of the `Cn`, `Co`, and `Cs` general categories, including
  direct support for negated `\P` escapes.
- RegExp modifier groups preserve their scoped `ignoreCase` semantics in word
  boundaries, backreferences, and Unicode property escapes throughout the IR
  and executor instruction stream.
- Non-Unicode pattern parsing retains UTF-16 code units for matching while
  named capture-group identifiers decode valid surrogate pairs as Unicode
  identifier characters.
- Duplicate named capture groups use one shared backreference instruction that
  selects the participating alternative before matching its captured text.
- `Regex::retained_payload_bytes` reports the compiled program's logical
  dynamic storage so the embedding VM can enforce its retained-payload policy.

When changing vendored source, update `VENDORED-SOURCE-SHA256SUMS` in the same
commit and describe the semantic deviation here or in the owning pull request.
