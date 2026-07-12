# Front-end: lexer, syntax, parser, AST

> Детальный отчёт аудита архитектуры rs-quickjs. Подсистема: `frontend`. Сгенерировано многоагентным аудитом 2026-07-12; каждая находка уровня medium/high прошла адверсариальную проверку отдельным агентом. Материал на английском (исходный текст аудита).

## Subsystem assessment

The frontend is a clean-layered, safe-Rust recursive-descent pipeline: an eager whole-file lexer produces a Vec<Token> (cooked payloads, spans, a line-terminator bit, and an identifier-escaped bit), the parser threads strict/await/yield/super/new.target context through save-restore combinators and interns names/strings/bindings into side tables consumed by later layers, and the AST is span-complete and genuinely compile-time-only. The discipline around checked arithmetic, resource limits (expression/statement depth, statement count), and exhaustive matches is excellent, and several classic parser traps (postfix-update line terminators, yield*, ??-mixing, private-name scoping) are handled honestly. However, the architecture leans on three systematic shortcuts that concentrate risk: (1) the eager lexer forces a previous-token heuristic for regex-vs-division that is wrong in well-known cases; (2) grammar ambiguities (arrow parameters, destructuring covers, for-in/of heads, sloppy `let`) are resolved by ad-hoc token scans, snapshots, and enumerated special cases rather than a cover grammar, producing O(n^2) scans, a partial-state rollback, and at least one concrete misparse; and (3) the parser desugars semantics into the AST at parse time (destructured params to %pattern% prologues, `undefined` to a literal, export-default to const bindings, default derived constructors to spread calls), duplicating or subtly changing semantics that the compiler/runtime should own. ASI is the weakest spot: the restricted productions for return/throw are simply not enforced. Early errors are implemented as post-hoc AST walks re-run per block, which is both quadratic and structurally fragile. None of this frontend debt is recorded in the AS-01..AS-10 program, which is almost entirely runtime-focused — these are the undocumented liabilities.

## Strengths

- Every token, Expression, and Statement carries a canonical SourceSpan (AstNode<K> wrapper), matching the documented AS-04b2b1 invariant; no offset reconstruction anywhere.
- Context threading is disciplined: with_await_context / with_yield_expression / with_super_context / with_new_target_scope all save-and-restore symmetrically, and class private-name scopes deliberately survive function boundaries with a comment explaining why.
- Resource limits are real: expression depth, statement depth, pattern depth, statement count, and every counter uses checked_add with typed limit errors — no unwrap/panic/unchecked indexing observed anywhere in ~8.9k lines.
- Hard cases are often handled honestly: postfix ++/-- line-terminator restriction, yield* line-terminator rejection, '??' mixing rejection, private #name forward references with bubbling scopes, escaped-'default'/'of'/'using' contextual-keyword rejection via the identifier_escaped token bit.
- Interned StaticName/StaticString/StaticBinding with dense u32 ids give downstream layers (binding_layout, compiler) a stable, cheap vocabulary; UTF-16 code units are preserved for string literal values.
- File-size and module discipline is genuinely good: parsing is decomposed into small single-purpose modules (binary.rs precedence ladder, pattern.rs, class_private.rs) that read like the spec grammar.

## Confirmed findings (adversarially verified)

### [HIGH] Regex-vs-division resolved by a previous-token blacklist in an eager lexer

- **Kind:** `hack` · **Location:** `src/lexer/classification.rs:72` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** confirmed

**Evidence:** token_kind_can_precede_regexp forbids a regex after Identifier, RParen, RBracket, RBrace (lines 72-93); scanner/mod.rs:132-144 consults only tokens.last(). The lexer runs to completion before the parser exists, so no grammatical context is available.

**Why it matters:** This heuristic is known-wrong: `if (x) /re/.test(y)` (regex after the `)` of a statement head), `{ } /re/.test(x)` (regex after a block's `}`), and `yield /re/` inside a generator (yield lexes as Identifier, not a keyword) all mislex as division and fail to parse or parse wrong. The ambiguity is fundamentally parser-fed; an eager token vector cannot decide it, so correctness is capped by a heuristic.

**Verifier notes:** classification.rs:72-93 is exactly the claimed blacklist (Identifier, RParen, RBracket, RBrace, literals, ++/-- forbid a following regex), and scanner/mod.rs:132-144 decides regex-vs-division from tokens.last() alone; lex() is fully eager and parser/mod.rs:50-63 consumes the finished Vec<Token>, with no re-lex or parser-feedback mechanism anywhere in the codebase. All three cited examples verified: RParen and RBrace are blacklisted so `if (x) /re/.test(y)` and `{ } /re/.test(x)` mislex `/` as division, and identifier_kind (classification.rs:27-68) has no "yield" arm, so yield lexes as Identifier (blacklisted) even though the parser supports generators and recognizes yield via that Identifier token (parser/assignment.rs:11,42) — so `yield /re/` mislexes too. No guard, documented invariant, comment, or test acknowledges the limitation.

**Verifier correction:** One tempering nuance: every verified failure mode surfaces as a loud lex/parse error on valid-but-uncommon constructs, not as silent misexecution (the blacklist errs only toward division, and a stray Slash cannot begin a statement or yield operand, so misparses do not slip through quietly). High severity is still fair for a JS engine: a class of spec-valid programs (including minifier-style output like `if(a)/re/.test(b)`) can never parse, and the fix is architectural — regex-vs-division is parser-context-dependent, so an eager token vector cannot decide it.

**Suggestion:** Make slash disambiguation parser-driven: lex lazily with an InputElementRegExp/InputElementDiv goal chosen by the parser, or have the parser request re-lexing of a Slash token as a regex when in expression position. This also unlocks removing the arrow/destructuring token-scan hacks.

### [HIGH] `undefined` is lexed as a reserved keyword and compiled as a literal

- **Kind:** `hack` · **Location:** `src/lexer/scanner/names.rs:67` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** confirmed

**Evidence:** identifier_kind maps "undefined" => TokenKind::Undefined; parser/expression.rs:442 turns it into Expr::Literal(Value::Undefined); consume_identifier has no Undefined arm, so it can never be a binding.

**Why it matters:** `undefined` is an ordinary identifier in ECMAScript. Here `var undefined`, `function f(undefined){}`, catch(undefined), and any shadowing are SyntaxErrors, and every `undefined` reference skips scope resolution entirely. This is a language-level deviation baked into the token model — the deepest possible layer — so every later layer inherits it; it also means the global `undefined` property and with-scope shadowing semantics can never be implemented without reworking the lexer.

**Verifier notes:** The evidence checks out end to end: identifier_kind maps "undefined" to TokenKind::Undefined (src/lexer/classification.rs:67, reached from scanner/names.rs:15; only "async" is exempted when escaped, so even undefined is reserved); parser/expression.rs:442 compiles it straight to Expr::Literal(Value::Undefined) bypassing scope resolution; and consume_identifier (src/parser/mod.rs:235-259) plus consume_binding_identifier have no Undefined arm, so `var undefined`, `function f(undefined){}`, and `catch (undefined)` all hit the fallthrough parse error despite being legal ECMAScript even in strict mode. I found no mitigating guard or documented deviation: docs/ never mentions the choice, the runtime's global builtin table (runtime/native/core.rs:34-102) registers NaN/Infinity/globalThis but not undefined (so `"undefined" in globalThis` is false), and sloppy-mode `with` IS supported (parser/statement.rs:286), making the shadowing impossibility a live semantic wrong-answer, not hypothetical. Severity high is fair for a project whose docs track Test262 and QuickJS differential parity as core goals; the only inaccuracies are the file citation (classification.rs:67, not names.rs:67) and a mild overstatement of the fix cost, neither of which weakens the defect.

**Verifier correction:** The keyword mapping actually lives at src/lexer/classification.rs:67 (identifier_kind), called from src/lexer/scanner/names.rs:15 — names.rs:67 is an unrelated helper. Everything else verifies: expression.rs:442 literalizes the token, consume_identifier (parser/mod.rs:235) rejects it as any binding, escaped forms (undefined) are also captured since only "async" is escape-exempt, no global `undefined` property is ever registered (runtime/native/core.rs builtin table has NaN/Infinity but not undefined, so `"undefined" in globalThis` is false), and sloppy-mode `with` is supported (parser/statement.rs:286) so the unimplementable-shadowing consequence is concrete. Minor overstatement: fixing does not require deep lexer rework — remove the map arm, treat it as Identifier, and register a global constant via the existing NaN/Infinity global_constant_value path — but the deviation does propagate through every token-model consumer (expression.rs, property_name.rs:44, classification.rs:86) as claimed.

**Suggestion:** Lex `undefined` as an Identifier; let binding resolution decide it refers to the (non-writable) global. Keep a compiler fast-path folding a free `undefined` reference to the literal only when binding layout proves it resolves to the global — that is the layered version of the same optimization.

### [HIGH] ASI restricted productions unenforced: return/throw ignore line terminators and never require a terminator

- **Kind:** `risk` · **Location:** `src/parser/statement.rs:599` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** confirmed

**Evidence:** return_statement (597-607) treats the operand as present unless the next token is `;`, `}`, or EOF — no peek_has_line_terminator_before check — and finishes with consume_optional_semicolon; throw_statement (591-595) likewise. Compare optional_jump_label (466) which does check the line terminator for break/continue.

**Why it matters:** `return\n42` parses as `return 42` (spec: ASI splits it into `return; 42;` — observably different return value); `throw\nerr` is accepted (spec: SyntaxError); and because consume_optional_semicolon replaces consume_statement_terminator, `return 1 2` and `throw a b` are silently accepted as two statements with no line break. ASI is exactly the area the task flags: this is the honest-parser test and it currently fails it.

**Verifier notes:** return_statement (src/parser/statement.rs:597-607) decides the operand is present unless the next token is ';', '}' or EOF, with no peek_has_line_terminator_before(0) check, and throw_statement (591-595) parses the operand unconditionally; both end with consume_optional_semicolon (src/parser/mod.rs:454-456), which is an infallible match_kind(Semicolon) rather than the erroring consume_statement_terminator (mod.rs:458-477). The statement dispatcher (statement.rs:67-72) adds no line-terminator guard and the lexer only flags line_terminator_before without inserting semicolons, so no other site enforces the restricted production. Consequently `return\n42` yields `return 42` instead of ASI's `return;` (observably different return value), `throw\nerr` is accepted instead of a SyntaxError, and `return 1 2` / `throw a b` are silently accepted as two statements. The cited contrast is accurate: break/continue via optional_jump_label (statement.rs:462-474) do check the line terminator and then require consume_statement_terminator.

**Suggestion:** Add the [no LineTerminator here] check after return/throw (operand only if !peek_has_line_terminator_before(0) and token can start an expression), and use consume_statement_terminator so missing terminators are errors.

### [HIGH] Parse-time desugaring of destructured/rest parameters into %pattern0%/%rest% synthetic params plus body-prologue var declarations

- **Kind:** `hack` · **Location:** `src/parser/function.rs:137` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** partially-confirmed

**Evidence:** pattern_parameter synthesizes `%pattern{N}%` bindings (const prefix at lines 10-18) and pushes Stmt::PatternDecl{kind: Var} statements that apply_prologue prepends to the body; the same pattern family synthesizes `%superargs%` for default derived constructors (class.rs:42-43, 141-153).

**Why it matters:** The parser is duplicating binding-initialization semantics that belong to binding_layout/compiler, and the duplication is observably wrong: spec parameters initialize strictly left-to-right, so in `function f({a}, b = a){}` the default `b = a` must see the already-destructured `a`; here `a` is only unpacked by a prologue statement that runs after all parameter defaults, so evaluation order is inverted. Sentinel `%`-names also share the user binding namespace, pattern names become `var` (wrong declaration class vs. the parameter environment), and the synthesized `super(...%superargs%)` routes the default derived constructor through the user-patchable array iterator, which the spec's default constructor never does.

**Verifier notes:** The cited code is exactly as described: pattern_parameter (src/parser/function.rs:137-167) and rest_parameter synthesize %pattern{N}%/%rest% params and push Stmt::PatternDecl{kind: Var} statements that apply_prologue prepends to the body, and class.rs:141-153 synthesizes constructor(...%superargs%){super(...%superargs%)}. The core defect is confirmed by the runtime: eval_function_body (src/runtime/function/parameters.rs:220-236) runs apply_function_param_defaults — evaluating all parameter defaults left-to-right — before hoist_bytecode_declarations and before the body prologue, so in `function f({a}, b = a)` the default `b = a` cannot see the destructured `a` (spec requires it can); no guard or eager binding exists (bound_names feeds only early errors). The %superargs% point is also confirmed: super-spread compiles to CollectSpreadArgs → expand_spread_values → get_iterator (runtime/bytecode/spread.rs:78), whose array fast path (abstract_operations/iterator.rs:234-249) is bypassed when Array.prototype[Symbol.iterator] is patched, so the default derived constructor observably invokes the user iterator, contrary to the post-ES2020 spec. However, the sub-claim that %-sentinel names "share the user binding namespace" is refuted — % cannot appear in user identifiers and the design comment (function.rs:10-13) documents this — and the var-declaration-class point is only marginally observable (parameter environment vs body var copies in closure corner cases).

**Verifier correction:** Core claim confirmed: the parse-time desugaring of destructured/rest parameters into synthetic %pattern{N}%/%rest% params plus body-prologue var PatternDecls inverts spec-required left-to-right parameter initialization — apply_function_param_defaults (src/runtime/function/parameters.rs:220) evaluates all defaults before the prologue unpacks any pattern, so `function f({a}, b = a){}` mis-evaluates (ReferenceError/undefined instead of the destructured value). Also confirmed: the synthesized super(...%superargs%) in default derived constructors (class.rs:141-153) routes through get_iterator and observably honors a patched Array.prototype[Symbol.iterator], which the spec's default constructor must not. Two sub-points corrected: (1) the %-sentinel names do NOT collide with the user binding namespace — % is unlexable in identifiers, per the documented design at function.rs:10-13; (2) the var-vs-parameter-environment classification is only observable in narrow corner cases (closures created in defaults vs body var copies). Severity high stands on the evaluation-order bug and the derived-constructor iterator observability alone.

**Suggestion:** Represent parameter patterns directly in FunctionParam (target: BindingPattern, default) and let the compiler emit initialization in parameter order; give the default derived constructor a dedicated compiler/runtime path instead of synthesized spread AST.

### [MEDIUM] Template tokens keep only cooked text: no raw component, substitution-free templates collapse into String tokens, escaped directives miscompare

- **Kind:** `hack` · **Location:** `src/lexer/scanner/mod.rs:427` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** partially-confirmed (corrected severity: low)

**Evidence:** end_template_part: "A template without substitutions stays a plain string token" (426-429); TokenKind::TemplateHead/Middle/Tail carry only cooked Vec<u16> (token.rs:22-24). strict.rs:141-143 detects "use strict" by comparing the cooked StaticString value.

**Why it matters:** Raw text is unrecoverable, so tagged templates and String.raw are unimplementable on this token model (there is no TaggedTemplate AST node either), and invalid escapes — which the spec defers to cooked=undefined inside tagged templates — are hard lex errors. Separately, because directives are compared post-cooking, `"use strict"` (spec: NOT a directive) enables strict mode, a silent semantic switch. The information loss is a one-way door taken at the lowest layer.

**Verifier notes:** The three code citations are accurate: scanner/mod.rs:426-429 collapses substitution-free templates into TokenKind::String, token.rs:22-24 keeps only cooked Vec<u16> with no raw text, and strict.rs:141-143 matches directives on the cooked value with no escape guard (Token.identifier_escaped is never set for strings, and Parser holds tokens only — no source text). The concrete spec deviations are real: "use\x20strict" and even `use strict` (a template!) enable strict mode, and collapsed templates are accepted as object property names (literal.rs:346) and module specifiers (module.rs:365-379). However, the "silent one-way door / unimplementable" framing is refuted: docs/project-plan.md explicitly documents tagged templates as a "known deferred gap" and the single-string token path for plain templates as a deliberate decision in a staged subset engine (~10.9k/102.6k Test262), tokens retain SourceSpans so raw text is recoverable via a normal refactor, and hard lex errors on invalid escapes are spec-correct for the untagged templates the engine actually supports.

**Verifier correction:** Cooked-only template/string tokens cause real but narrow spec deviations: escaped directives ("use\x20strict") and substitution-free templates (`use strict`) incorrectly enable strict mode, and templates leak into StringLiteral-only positions (object property names, module specifiers). Tagged templates/String.raw are a documented deferred gap (docs/project-plan.md), not a hidden architectural trap; adding raw capture to the token model is a routine extension since tokens already carry SourceSpans.

**Suggestion:** Store (cooked: Option<Vec<u16>>, raw: Vec<u16>) on template parts, keep a real TemplateHead token even without substitutions (or record raw on String tokens), and mark string tokens containing escapes/continuations so directive detection can require an escape-free literal.

### [MEDIUM] Escaped reserved words still act as keywords; only `async` and a few contextual words are guarded

- **Kind:** `risk` · **Location:** `src/lexer/classification.rs:23` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** confirmed

**Evidence:** identifier_kind special-cases `escaped && text == "async"` only; every other escaped spelling (`let`, `if`, `this`) still yields the keyword TokenKind with identifier_escaped=true. The parser consults identifier_escaped only at contextual sites (contextual_let expression.rs:475, module `default`/`of`/`using`/`as`/`from`), never in statement_inner's match_kind(&TokenKind::If) etc.

**Why it matters:** Spec: an escaped ReservedWord may never be treated as a keyword — `if (x) {}` is a SyntaxError but parses as `if` here. The guard exists (the documented identifier_escaped bit) but its enforcement is piecemeal and asymmetric: one keyword fixed in the lexer, a handful in the parser, the rest unguarded. Incomplete guards on a security/correctness-relevant fast path are precisely the fragile-cache shape, and each new contextual keyword must remember to re-check the bit.

**Verifier notes:** identifier_kind (src/lexer/classification.rs:23-26) demaps only escaped "async" to Identifier; all other escaped reserved-word spellings become keyword TokenKinds with identifier_escaped=true (scanner/names.rs:15-16), and the parser's check/match_kind (parser/mod.rs:479-490) compare kind only, so statement_inner (statement.rs:33 etc.) parses if (x) {} as an if statement where ECMA-262 requires a SyntaxError. The identifier_escaped bit is consulted only at the nine-ish contextual sites the auditor cited (contextual let, module default/of/using, for-of), with no global guard and no early-error pass, and existing tests (for_await_of_smoke.rs:167, module_compile_smoke.rs:95) show rejecting escaped keywords is an intended invariant that is enforced asymmetrically. One caveat: the unguarded sites can only over-accept spec-invalid programs (escaped keyword behaves identically to the unescaped keyword), never misparse valid programs — the ambiguous contextual cases are exactly the guarded ones — so the impact is conformance plus parser-differential/maintenance risk, which supports medium but not higher.

**Verifier correction:** Confirmed as described: escaped reserved words (e.g. if, return, class) lex to keyword TokenKinds and parse as keywords because match_kind/check never consult identifier_escaped; only async (lexer) and a handful of contextual sites (let, default, of, using, for-of) are guarded. Impact is over-acceptance of spec-invalid programs (test262 conformance, parser-differential risk) rather than misparsing of valid programs, since the escaped keyword executes with the same semantics as its unescaped spelling; the guarded contextual sites are exactly where valid programs could be misparsed. A single centralized check (e.g. rejecting identifier_escaped in check()/match_kind() for non-contextual keyword kinds, or demapping all escaped reserved words to Identifier in identifier_kind and handling the resulting early error) would close the gap and remove the per-site burden.

**Suggestion:** Enforce once at the boundary: when identifier_kind classifies an escaped spelling as any keyword, either error immediately (spec-conforming for reserved words) or emit Identifier and let contextual sites opt in, deleting the per-site checks.

### [MEDIUM] Two duplicated ad-hoc bracket-matching token scanners for arrow and destructuring disambiguation, both O(n^2) on nesting

- **Kind:** `inelegant` · **Location:** `src/parser/expression.rs:686` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** partially-confirmed (corrected severity: low)

**Evidence:** parenthesized_arrow_end (686-715) scans from every `(` in assignment position to its matching closer, treating LParen/LBracket/LBrace as one depth counter and any closer as a decrement; assignment.rs outer_literal_closing_offset (103-137) is a second scanner that does check delimiter kinds. Every assignment() beginning at `(`/`[`/`{` runs a scan to the outer closer (assignment.rs:85-101), and each nested element re-runs it.

**Why it matters:** For `((((...x...))))` or `[[[[a]]]]=b` each nesting level rescans the remaining literal, giving quadratic parse cost on adversarial (or merely deeply nested generated) input — a DoS vector in an engine with explicit resource limits everywhere else. Having two scanners with different matching strictness for the same concept invites divergence: the arrow scanner accepts mismatched bracket kinds that the pattern scanner rejects, so the two disambiguators disagree on which prefixes are 'balanced'.

**Verifier notes:** The structural facts check out: parenthesized_arrow_end (expression.rs:686-715) is a kind-blind depth counter (only the final depth-0 token is checked to be RParen), outer_literal_closing_offset (assignment.rs:103-137) is a second, kind-strict scanner, and each nesting level does rerun a scan with no memoization (assignment.rs:17, expression.rs:651, pattern.rs:38). However, the O(n^2)/DoS reasoning fails against guards the auditor missed: check_source_len (compiled_script/mod.rs:213) caps input at 65,536 bytes and with_expression_depth/with_pattern_depth (sequence.rs:34-53, pattern.rs:249-264) abort parsing at depth 256 before deeper scans run, bounding worst-case work to O(depth_limit x tokens) ~ 8M cheap peeks — milliseconds, and asymptotically O(D*n), not unbounded quadratic. The scanner-strictness divergence is real in code but only observable on syntactically invalid input (valid token streams always nest delimiters properly by kind), so it affects error paths, not parse results of valid programs.

**Verifier correction:** Valid as an inelegance finding: two near-duplicate ad-hoc bracket scanners with different matching strictness, and per-nesting-level rescanning that is O(depth x extent). But the DoS claim is neutralized by the engine's own enforced limits — max_source_len (64 KiB, checked before parse) and max_expression_depth (256, checked before each deeper scan) cap worst-case scanner work at a few milliseconds; true quadratic blowup requires depth proportional to input, which the depth limit forbids. The strictness divergence between the two scanners only changes error-path behavior on already-invalid input, never the parse of a valid program. Worth unifying on one strict matching helper for maintainability, but not a security or correctness issue at defaults; only an embedder who deliberately raises both limits could see elevated (still linear-in-source) cost.

**Suggestion:** Either implement the spec cover grammar (parse once as CoverParenthesizedExpressionAndArrowParameterList / cover the literal as an expression and reinterpret on `=`), or share one delimiter-matching scanner and memoize matching-close offsets computed in a single O(n) pre-pass over the token vector.

### [MEDIUM] Sloppy-mode `let` disambiguation is an enumerated special-case list that misparses `let\nx = 1`

- **Kind:** `hack` · **Location:** `src/parser/statement.rs:123` · **Status in project docs:** UNDOCUMENTED
- **Verification verdict:** confirmed

**Evidence:** let_starts_expression_statement treats `let` as an expression when the next token has a line terminator before it or is `=` (126-127), excluding let/[/await/yield. For `let\nx = 1` the line-terminator branch fires, let_expression_statement (136-158) emits Identifier(let), and ASI accepts the split.

**Why it matters:** Spec: a line terminator between `let` and its binding is insignificant; `let\nx = 1` is one LexicalDeclaration, but this parser executes `let; x = 1;` — a silent semantic change (x becomes a global/var write, no lexical binding, no TDZ). The under-approximation also rejects legal sloppy code: `let;`, `let(0)`, `let.a = 1` all error because they fall through to var_decl. The spec rule is a single lookahead class (declaration iff next token is `[`, `{`, or an identifier); the enumerated conditions are strictly less correct.

**Verifier notes:** The code is exactly as claimed: `let_starts_expression_statement` (src/parser/statement.rs:123-134) fires in sloppy mode when the token after `let` is preceded by a line terminator or is `=`, minus the enumerated exclusions (let, `[`, await, yield). For `let\nx = 1` the line-terminator branch fires, `let_expression_statement` (136-158) consumes `let`, finds no `=` next, emits `Expr::Identifier(let)`, and `consume_statement_terminator` (mod.rs:458-462) accepts via the line-terminator ASI rule — so `x = 1` becomes a separate sloppy assignment (global write, no lexical binding, no TDZ), whereas ECMA-262 has no [no LineTerminator here] restriction in LexicalDeclaration and ASI cannot apply when the declaration parse succeeds (the exact principle test262's let-let-declaration-split-across-two-lines.js documents). The rejection half also holds: every other statement-initial `let` is intercepted at statement.rs:108 and routed to `var_decl`, whose `consume_binding_identifier` errors on `;`/`(`/`.`, so legal sloppy `let;`, `let(0)`, `let.a = 1` are parse errors; the expression-level `contextual_let` (expression.rs:519) is unreachable at statement start, and I found no guard, comment, test, or doc marking this as an intentional upstream-parity deviation. Note the exclusion list precisely covers test262's negative let-newline tests (let/await/yield/`[`) while missing the general rule, supporting the "enumerated special-case" characterization; the same defect also mis-splits `let\n(0)` and rejects multi-line `let\n{a} = obj` destructuring. Medium severity is fair: silent semantic change in an engine is serious, but the trigger (sloppy mode with a newline or exotic token immediately after `let`) is rare in practice.

**Suggestion:** Replace the condition list with the spec lookahead: after `let`, treat as declaration iff the next token (ignoring line terminators) can begin a binding (`{`, `[`, identifier/contextual identifier); otherwise reparse `let` as an IdentifierReference expression statement.

## Additional low-severity findings (not separately verified)

### [LOW] `**` left-operand early error checks the AST for Expr::Unary only, missing Await

- **Kind:** `risk` · **Location:** `src/parser/binary.rs:177` · **Status in project docs:** UNDOCUMENTED

**Evidence:** power(): `if matches!(left.kind(), Expr::Unary { .. })` rejects `-x ** 2` etc., but Await is a separate Expr variant (Expr::Await), so `await x ** 2` parses as `(await x) ** 2`.

**Why it matters:** Spec grammar makes ExponentiationExpression take only UpdateExpression on the left; `await x ** 2` is a SyntaxError in every conforming engine. The bug is structural: encoding a grammar restriction as an AST-shape check means every operator that is 'unary-like but its own variant' silently escapes — the same trap will recur if new prefix forms are added.

**Suggestion:** Reject based on how the operand was parsed (a flag returned by unary() when it consumed a prefix operator, await, or delete) rather than pattern-matching the produced node.

### [LOW] Dynamic/constructor `import` modeled as an ordinary identifier binding named "import" plus a post-hoc AST walk

- **Kind:** `hack` · **Location:** `src/parser/expression.rs:217` · **Status in project docs:** UNDOCUMENTED

**Evidence:** import_constructor_seed interns a real StaticBinding named "import" and emits Expr::Identifier for it; constructor_starts_with_import (224-233) then walks Member/ComputedMember/Parenthesized chains to reject `new import(...)`.

**Why it matters:** `import` is not an identifier and cannot be bound; encoding it as one leaks a fake binding into the binding table (visible to binding_layout) and relies on a shape-walk to un-confuse `new`. Any other consumer that treats Identifier("import") as a normal variable reference (scope resolution, name inference, diagnostics) is now wrong by default. The docs' AS-09ah residual list explicitly leaves dynamic import unimplemented, so this seed exists only to produce an error — a placeholder encoded in the semantic namespace.

**Suggestion:** Add an Expr::ImportCall (and ImportMeta) variant produced directly by the grammar; reject `new import` at the grammar site where `import` follows `new`, deleting the walk and the fake binding.

### [LOW] `arguments` usage detected by one global monotonic counter, over-marking outer functions

- **Kind:** `inelegant` · **Location:** `src/parser/mod.rs:114` · **Status in project docs:** UNDOCUMENTED

**Evidence:** arguments_reference_count increments on every identifier `arguments` anywhere (note_arguments_reference, 316-325); each function takes a snapshot before its parameters and checks arguments_referenced_since after its body (e.g. statement.rs:619/648), so references inside nested non-arrow functions — which bind their own arguments — still mark every enclosing function.

**Why it matters:** A lexically-scoped question answered by a global counter: `function outer(){ function inner(){ return arguments } }` gives outer an arguments_binding it never needs, forcing arguments-object materialization (one of the most expensive per-call allocations an engine makes) on functions that never touch it. It is also rollback-coupled state: for_statement.rs must remember to snapshot/restore it during speculation. Correct for arrows only by accident of the same over-approximation.

**Suggestion:** Track a per-function-context 'uses arguments' flag on a stack pushed by function entry (arrows inherit the parent frame, ordinary functions push a fresh one); this is exact, O(1), and removes the counter from the speculation snapshot set.

### [LOW] `debugger` statement silently parsed as Stmt::Empty

- **Kind:** `hack` · **Location:** `src/parser/statement.rs:63` · **Status in project docs:** UNDOCUMENTED

**Evidence:** `if self.match_kind(&TokenKind::Debugger) { self.consume_optional_semicolon(); return Ok(Stmt::Empty); }` — no AST representation, no diagnostic, and consume_optional_semicolon also skips terminator validation (`debugger 1` parses).

**Why it matters:** Dropping the construct at parse time erases it from spans/diagnostics and forecloses any future debugger/inspector support without a parser change; using Empty also lies to early-error walks (a labeled `debugger` is indistinguishable from a labeled empty statement). Small, but it is semantics deleted by the frontend rather than ignored by the runtime, which inverts the project's own layering rule that the parser preserves and later layers decide.

**Suggestion:** Add Stmt::Debugger (compile to a no-op or a host hook) and use consume_statement_terminator.

## Optimization opportunities (post-cleanup)

### [MEDIUM] Intern tables are sorted Vec + binary_search + Vec::insert — O(n^2) interning with mirrored index/value arrays

- **Kind:** `perf-opportunity` · **Location:** `src/parser/static_tables.rs:147` · **Status in project docs:** UNDOCUMENTED

**Evidence:** remember_name does self.index.insert(position, ...) into a sorted Vec (147, and 41-42 for strings), shifting O(n) entries per new symbol after a binary search; names and index both store cloned StaticName values, with defensive 'static name id is not defined' errors (158) covering potential desync between the mirrors.

**Why it matters:** Interning is on the hot path for every identifier and string literal; sorted-Vec insertion makes whole-program parsing quadratic in distinct-symbol count, and the duplicated names/index Vecs are redundant state whose only consistency guarantee is runtime error checks. This is also the structure cloned wholesale by the for-head speculation snapshot, compounding the cost.

**Suggestion:** Once the foundation is settled: single Vec<StaticName> for id->name plus a HashMap<Rc<str>, StaticNameId> (or a u16-keyed variant for strings) for lookup — O(1) amortized interning, one owner, and snapshot/rollback becomes a stored length. This also removes the six defensive error paths.

### [LOW] Whole-source Vec<(usize, char)> materialization and clone-on-advance token consumption

- **Kind:** `perf-opportunity` · **Location:** `src/lexer/scanner/mod.rs:44` · **Status in project docs:** UNDOCUMENTED

**Evidence:** Lexer::new collects source.char_indices() into Vec<(usize, char)> (16 bytes per char on 64-bit, ~4-16x source size) before scanning; parser advance() (parser/mod.rs:492-498) does peek()?.clone() on every consumed token, deep-cloning Identifier(String)/String(Vec<u16>)/BigInt payloads that mostly get interned and dropped immediately.

**Why it matters:** Two avoidable O(source) memory/allocation costs on the front of every compile (including every dynamic eval, which the runtime routes through this path): the char vector duplicates the source at 4-16x expansion, and clone-on-advance doubles every heap-carrying token. Neither buys correctness; both shape peak memory for large inputs and interact badly with the eager 'lex the whole file' model that also causes the regex heuristic.

**Suggestion:** After the regex fix moves lexing under parser control: scan over &str with a byte cursor + chars() peeking (no side vector), and have the parser take tokens by value (std::mem::replace with a cheap Eof placeholder, or index-based moves) so payloads move instead of cloning.
