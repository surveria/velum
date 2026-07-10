#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_repo_root="$(cd "${script_dir}/.." && pwd)"
repo_root="${RSQJS_ARCHITECTURE_ROOT:-${default_repo_root}}"
script_name="check-architecture-boundaries"

fail() {
  printf '%s: %s\n' "${script_name}" "$*" >&2
  printf '%s: review docs/semantic-architecture-inventory.md before changing this allowlist\n' \
    "${script_name}" >&2
  exit 1
}

require_file() {
  local path="$1"
  if [[ ! -f "${repo_root}/${path}" ]]; then
    fail "required architecture input '${path}' is missing"
  fi
}

require_dir() {
  local path="$1"
  if [[ ! -d "${repo_root}/${path}" ]]; then
    fail "required architecture input directory '${path}' is missing"
  fi
}

compare_set() {
  local label="$1"
  local actual="$2"
  local expected="$3"
  local actual_sorted
  local expected_sorted
  actual_sorted="$(printf '%s\n' "${actual}" | sed '/^[[:space:]]*$/d' | sort)"
  expected_sorted="$(printf '%s\n' "${expected}" | sed '/^[[:space:]]*$/d' | sort)"
  if [[ "${actual_sorted}" == "${expected_sorted}" ]]; then
    return
  fi
  printf '%s: %s changed\n' "${script_name}" "${label}" >&2
  printf '%s\n' '--- expected' >&2
  printf '%s\n' "${expected_sorted}" >&2
  printf '%s\n' '--- actual' >&2
  printf '%s\n' "${actual_sorted}" >&2
  fail "${label} must not grow or move without its assigned AS migration"
}

function_owners() {
  local pattern="$1"
  (
    cd "${repo_root}"
    grep -R -H -E -o --include='*.rs' "${pattern}" src/runtime || true
  ) | sed -E 's/:fn[[:space:]]+/:/'
}

check_value_representation() {
  local actual
  local expected
  actual="$(
    awk '
      /^pub enum Value \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/value/kind.rs" \
      | sed '/^[[:space:]]*\/\//d' \
      | tr -d '[:space:]'
  )"
  expected='Undefined,Null,Bool(bool),Number(f64),String(String),HeapString(JsString),Symbol(JsSymbol),Function(FunctionId),NativeFunction(NativeFunctionId),HostFunction(HostFunctionId),Object(ObjectId),'
  if [[ "${actual}" != "${expected}" ]]; then
    fail "Value representation changed; AS-02 owns object-like representation changes"
  fi
}

check_runtime_frontend_boundary() {
  local imports
  imports="$(
    cd "${repo_root}"
    grep -R -n -E --include='*.rs' \
      'crate::(ast|parser|lexer)(::|[},])|crate::\{[[:space:]]*(ast|parser|lexer)(::|[},])' \
      src/runtime src/bytecode || true
  )"
  if [[ -n "${imports}" ]]; then
    printf '%s\n' "${imports}" >&2
    fail "runtime/frontend boundary changed; runtime and bytecode must remain parser-AST-free"
  fi
}

check_source_metadata_boundary() {
  local source_type_owners
  local expected_source_type_owners
  local compiled_fields
  local expected_compiled_fields
  local frontend_diagnostics

  source_type_owners="$(
    cd "${repo_root}"
    grep -R -H -E -o --include='*.rs' \
      'pub struct (SourceId|SourceSpan)' src || true
  )"
  expected_source_type_owners='src/source.rs:pub struct SourceId
src/source.rs:pub struct SourceSpan'
  compare_set "source metadata type owner allowlist" \
    "${source_type_owners}" "${expected_source_type_owners}"

  compiled_fields="$(
    awk '
      /^pub struct CompiledScript \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/compiled_script/mod.rs" \
      | sed '/^[[:space:]]*\/\//d' \
      | tr -d '[:space:]'
  )"
  expected_compiled_fields='bytecode:BytecodeProgram,binding_layout:BindingLayout,usage:CompiledScriptUsage,source_id:SourceId,source_name:Option<String>,'
  if [[ "${compiled_fields}" != "${expected_compiled_fields}" ]]; then
    fail "CompiledScript source metadata boundary changed; AS-04b2 owns source retention"
  fi

  frontend_diagnostics="$(
    grep -E '^[[:space:]]*(Lex|Parse)[[:space:]]*\{' \
      "${repo_root}/src/error.rs" \
      | tr -d '[:space:]'
  )"
  if [[ "${frontend_diagnostics}" != 'Lex{message:String,span:SourceSpan},Parse{message:String,span:SourceSpan},' ]]; then
    fail "frontend source diagnostic boundary changed; lexer and parser errors require SourceSpan"
  fi
}

check_frontend_span_boundary() {
  local token_fields
  local expected_token_fields

  token_fields="$(
    awk '
      /^pub struct Token \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/lexer/token.rs" \
      | sed '/^[[:space:]]*\/\//d' \
      | tr -d '[:space:]'
  )"
  expected_token_fields='pubkind:TokenKind,pubspan:SourceSpan,publine_terminator_before:bool,'
  if [[ "${token_fields}" != "${expected_token_fields}" ]]; then
    fail "frontend token span boundary changed; tokens require one canonical SourceSpan"
  fi

  if ! grep -F -q 'pub type Expression = AstNode<Expr>;' \
      "${repo_root}/src/ast/expression.rs" \
    || ! grep -F -q 'pub type Statement = AstNode<Stmt>;' \
      "${repo_root}/src/ast/statement.rs"; then
    fail "frontend AST span boundary changed; expressions and statements require AstNode"
  fi
  if ! grep -F -q 'pub(super) fn expression(&mut self) -> Result<Expression>' \
      "${repo_root}/src/parser/expression.rs" \
    || ! grep -F -q 'pub(super) fn statement(&mut self) -> Result<Statement>' \
      "${repo_root}/src/parser/statement.rs"; then
    fail "parser AST span boundary changed; parser roots must return span-bearing nodes"
  fi
  if ! grep -F -q 'pub(super) fn compile_expr(&mut self, expr: &Expression)' \
      "${repo_root}/src/compiler/expression.rs" \
    || ! grep -F -q 'fn compile_statement(&mut self, statement: &Statement' \
      "${repo_root}/src/compiler/mod.rs"; then
    fail "compiler AST span boundary changed; compiler inputs must retain frontend nodes"
  fi
}

check_bytecode_span_boundary() {
  local block_fields
  local compiler_fields
  local execution_owners
  local expected_execution_owners
  local runtime_span_fields

  block_fields="$(
    awk '
      /^pub struct BytecodeBlock \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/bytecode/block.rs" \
      | sed '/^[[:space:]]*\/\//d' \
      | tr -d '[:space:]'
  )"
  if [[ "${block_fields}" != 'instructions:Rc<[BytecodeInstruction]>,spans:Rc<[SourceSpan]>,' ]]; then
    fail "bytecode source span boundary changed; BytecodeBlock requires one aligned span table"
  fi

  compiler_fields="$(
    awk '
      /^struct BytecodeCompiler/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/compiler/mod.rs" \
      | sed '/^[[:space:]]*\/\//d' \
      | tr -d '[:space:]'
  )"
  if [[ "${compiler_fields}" != "layout:&'aBindingLayout,instructions:Vec<BytecodeInstruction>,spans:Vec<SourceSpan>,current_span:SourceSpan," ]]; then
    fail "bytecode source span compiler boundary changed; emit requires one current AST span"
  fi
  if ! grep -F -q 'self.instructions.push(instruction);' \
      "${repo_root}/src/compiler/mod.rs" \
    || ! grep -F -q 'self.spans.push(self.current_span);' \
      "${repo_root}/src/compiler/mod.rs"; then
    fail "bytecode source span compiler boundary changed; emit must append instruction and span together"
  fi

  execution_owners="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' 'block.step(state.pc)?' src/runtime || true
  )"
  expected_execution_owners='src/runtime/bytecode/execution.rs
src/runtime/bytecode/linear/segment.rs'
  compare_set "bytecode source span execution owner allowlist" \
    "${execution_owners}" "${expected_execution_owners}"

  runtime_span_fields="$(
    grep -E '^[[:space:]]+span: Option<(Box<)?SourceSpan' "${repo_root}/src/error.rs" \
      | wc -l \
      | tr -d '[:space:]'
  )"
  if [[ "${runtime_span_fields}" != "4" ]]; then
    fail "runtime source diagnostic boundary changed; Runtime, JavaScript, and resource errors require optional spans"
  fi
}

check_harness_boundaries() {
  local compiler_comparisons
  local expected_comparisons
  local harness_paths
  local expected_harness_paths
  local test262_paths
  local test262_count

  compiler_comparisons="$(
    cd "${repo_root}"
    grep -R -H -E -o --include='*.rs' \
      '\.as_str\(\)[[:space:]]*(==|!=)[[:space:]]*"[^"]+"' \
      src/compiler || true
  )"
  compiler_comparisons="$(
    printf '%s\n' "${compiler_comparisons}" \
      | sed -E 's/[[:space:]]*(==|!=)[[:space:]]*/ \1 /'
  )"
  expected_comparisons='src/compiler/call.rs:.as_str() == "print"
src/compiler/call.rs:.as_str() == "assert"
src/compiler/call.rs:.as_str() != "throws"'
  compare_set "compiler source-name allowlist" "${compiler_comparisons}" "${expected_comparisons}"

  harness_paths="$(
    cd "${repo_root}"
    grep -R -l -E --include='*.rs' \
      '(^|[^A-Za-z0-9_])(Print|AssertThrows)([^A-Za-z0-9_]|$)' \
      src/bytecode src/compiler src/runtime || true
  )"
  expected_harness_paths='src/bytecode/metrics/mod.rs
src/bytecode/types.rs
src/compiler/call.rs
src/compiler/mod.rs
src/runtime/bytecode/call.rs
src/runtime/bytecode/mod.rs'
  compare_set "harness opcode use-site allowlist" "${harness_paths}" "${expected_harness_paths}"

  test262_paths="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' 'TEST262_ERROR_NAME' src/runtime src/compiler || true
  )"
  compare_set "Test262 source-name allowlist" "${test262_paths}" 'src/runtime/mod.rs'
  test262_count="$(
    cd "${repo_root}"
    { grep -o -F 'TEST262_ERROR_NAME' src/runtime/mod.rs || true; } \
      | wc -l \
      | tr -d '[:space:]'
  )"
  if [[ "${test262_count}" != "2" ]]; then
    fail "Test262 source-name allowlist changed; expected two named fallback references"
  fi
}

check_semantic_duplicate_allowlists() {
  local equality
  local expected_equality
  local conversion
  local expected_conversion
  local invocation
  local expected_invocation
  local semantic_object
  local expected_semantic_object
  local indexing
  local expected_indexing
  local property_call
  local expected_property_call
  local legacy_property_call
  local iterator_operations
  local expected_iterator_operations
  local legacy_iterator_operations

  equality="$(
    function_owners \
      'fn[[:space:]]+[a-z_]*(same_value|same_number_value|strict_equal|abstract_equality|numbers_equal|switch_number_equal)[a-z_]*'
  )"
  expected_equality='src/runtime/abstract_operations/equality.rs:abstract_equality
src/runtime/abstract_operations/equality.rs:number_same_value
src/runtime/abstract_operations/equality.rs:number_same_value_zero
src/runtime/abstract_operations/equality.rs:number_strict_equality
src/runtime/abstract_operations/equality.rs:same_value
src/runtime/abstract_operations/equality.rs:same_value_zero
src/runtime/abstract_operations/equality.rs:strict_equality'
  compare_set "equality operation allowlist" "${equality}" "${expected_equality}"

  conversion="$(
    function_owners \
      'fn[[:space:]]+(display_for_concat|get_to_primitive_method|is_primitive|is_truthy|ordinary_to_primitive|prefixed_integer_to_number|property_key[[:space:]]*\(|string_to_number|to_boolean|to_number_primitive|to_number|to_primitive|to_property_key[[:space:]]*\(|to_string_primitive|to_string[[:space:]]*\()' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  expected_conversion='src/runtime/abstract_operations/conversion.rs:get_to_primitive_method
src/runtime/abstract_operations/conversion.rs:is_primitive
src/runtime/abstract_operations/conversion.rs:ordinary_to_primitive
src/runtime/abstract_operations/conversion.rs:prefixed_integer_to_number
src/runtime/abstract_operations/conversion.rs:string_to_number
src/runtime/abstract_operations/conversion.rs:to_boolean
src/runtime/abstract_operations/conversion.rs:to_number
src/runtime/abstract_operations/conversion.rs:to_number_primitive
src/runtime/abstract_operations/conversion.rs:to_primitive
src/runtime/abstract_operations/conversion.rs:to_property_key
src/runtime/abstract_operations/conversion.rs:to_string
src/runtime/abstract_operations/conversion.rs:to_string_primitive'
  compare_set "primitive conversion operation allowlist" "${conversion}" "${expected_conversion}"

  invocation="$(
    function_owners 'fn[[:space:]]+[a-z_]*(is_callable|is_constructor)[a-z_]*'
  )"
  expected_invocation='src/runtime/semantic_object/invocation.rs:semantic_is_callable
src/runtime/semantic_object/invocation.rs:semantic_is_constructor'
  compare_set "callable/constructor predicate allowlist" "${invocation}" "${expected_invocation}"

  semantic_object="$(
    function_owners \
      'fn[[:space:]]+[a-z_]*(semantic_object_ref|semantic_type_name|semantic_call|semantic_construct|semantic_property_read|semantic_property_presence|semantic_property_write|semantic_property_delete|semantic_reflect_property_write|semantic_define_own_property|semantic_own_enumerable_string_keys|semantic_own_property|semantic_get_prototype|semantic_try_set_prototype|semantic_is_extensible|semantic_prevent_extensions|semantic_set_integrity_level|semantic_test_integrity_level|finish_semantic_property_read|finish_semantic_property_presence|finish_semantic_property_write|finish_semantic_property_delete|delete_property_value_with_lookup|is_object_like|constructor_return_is_object)[a-z_]*'
  )"
  expected_semantic_object='src/runtime/semantic_object.rs:finish_semantic_property_presence
src/runtime/semantic_object.rs:finish_semantic_property_read
src/runtime/semantic_object.rs:semantic_object_ref
src/runtime/semantic_object.rs:semantic_property_presence
src/runtime/semantic_object.rs:semantic_property_read
src/runtime/semantic_object.rs:semantic_property_read_with_receiver
src/runtime/semantic_object/descriptor.rs:semantic_define_own_property_from_value
src/runtime/semantic_object/descriptor.rs:semantic_define_own_property_update
src/runtime/semantic_object/descriptor.rs:semantic_define_own_property_update_with_descriptor
src/runtime/semantic_object/descriptor.rs:semantic_own_property_descriptor
src/runtime/semantic_object/invocation.rs:semantic_call
src/runtime/semantic_object/invocation.rs:semantic_construct
src/runtime/semantic_object/invocation.rs:semantic_type_name
src/runtime/semantic_object/keys.rs:semantic_own_enumerable_string_keys
src/runtime/semantic_object/keys.rs:semantic_own_property_keys
src/runtime/semantic_object/keys.rs:semantic_own_property_names
src/runtime/semantic_object/keys.rs:semantic_own_property_symbols
src/runtime/semantic_object/mutation.rs:delete_property_value_with_lookup
src/runtime/semantic_object/mutation.rs:finish_semantic_property_delete
src/runtime/semantic_object/mutation.rs:finish_semantic_property_write
src/runtime/semantic_object/mutation.rs:semantic_property_delete
src/runtime/semantic_object/mutation.rs:semantic_property_write
src/runtime/semantic_object/mutation.rs:semantic_reflect_property_write
src/runtime/semantic_object/prototype_integrity.rs:semantic_get_prototype
src/runtime/semantic_object/prototype_integrity.rs:semantic_is_extensible
src/runtime/semantic_object/prototype_integrity.rs:semantic_prevent_extensions
src/runtime/semantic_object/prototype_integrity.rs:semantic_set_integrity_level
src/runtime/semantic_object/prototype_integrity.rs:semantic_test_integrity_level
src/runtime/semantic_object/prototype_integrity.rs:semantic_try_set_prototype'
  compare_set "semantic object facade allowlist" "${semantic_object}" "${expected_semantic_object}"

  indexing="$(
    function_owners \
      'fn[[:space:]]+[a-z_]*(array_like_length|to_length|to_integer|to_index)[a-z_]*'
  )"
  expected_indexing='src/runtime/abstract_operations/conversion.rs:to_index
src/runtime/abstract_operations/conversion.rs:to_integer_or_infinity
src/runtime/abstract_operations/conversion.rs:to_length
src/runtime/call/bound.rs:array_like_length_from_value
src/runtime/native/builtins/array/callbacks.rs:array_like_length_for_callback
src/runtime/native/builtins/array/generic.rs:array_like_length
src/runtime/native/builtins/array/generic.rs:array_like_length_value
src/runtime/native/builtins/array/generic.rs:checked_array_like_length
src/runtime/native/builtins/array/generic.rs:max_array_like_length
src/runtime/native/builtins/array/generic.rs:set_array_like_length'
  compare_set "length/integer operation allowlist" "${indexing}" "${expected_indexing}"

  property_call="$(
    (
      cd "${repo_root}"
      grep -R -H -E -o --include='*.rs' \
        'fn[[:space:]]+(get|get_named|set|call|call_value|get_method|get_named_method)[[:space:]]*\(' \
        src/runtime/abstract_operations || true
    ) | sed -E 's/:fn[[:space:]]+/:/; s/[[:space:]]*\($//'
  )"
  expected_property_call='src/runtime/abstract_operations/property_call.rs:call
src/runtime/abstract_operations/property_call.rs:call_value
src/runtime/abstract_operations/property_call.rs:get
src/runtime/abstract_operations/property_call.rs:get_method
src/runtime/abstract_operations/property_call.rs:get_named
src/runtime/abstract_operations/property_call.rs:get_named_method
src/runtime/abstract_operations/property_call.rs:set'
  compare_set \
    "property/method/call abstract-operation allowlist" \
    "${property_call}" \
    "${expected_property_call}"

  legacy_property_call="$(
    function_owners \
      'fn[[:space:]]+(eval_call_completion|eval_call_value|get_property_value|get_property_value_with_lookup|proxy_trap)[[:space:]]*\('
  )"
  compare_set "legacy property/method/call facade allowlist" "${legacy_property_call}" ''

  iterator_operations="$(
    function_owners \
      'fn[[:space:]]+(get_iterator|get_iterator_from_method|iterator_method|iterator_step|iterator_close|iterator_close_on_error)[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  expected_iterator_operations='src/runtime/abstract_operations/iterator.rs:get_iterator
src/runtime/abstract_operations/iterator.rs:get_iterator_from_method
src/runtime/abstract_operations/iterator.rs:iterator_close
src/runtime/abstract_operations/iterator.rs:iterator_close_on_error
src/runtime/abstract_operations/iterator.rs:iterator_method
src/runtime/abstract_operations/iterator.rs:iterator_step'
  compare_set \
    "iterator abstract-operation allowlist" \
    "${iterator_operations}" \
    "${expected_iterator_operations}"

  legacy_iterator_operations="$(
    function_owners \
      'fn[[:space:]]+(close_for_of_source|for_of_source|for_of_step|protocol_source|set_call)[[:space:]]*\('
  )"
  compare_set "legacy iterator facade allowlist" "${legacy_iterator_operations}" ''
}

check_completion_error_boundary() {
  local legacy_conversions
  local typed_variant
  local exception_fields
  local owners
  local expected_owners

  legacy_conversions="$(
    cd "${repo_root}"
    grep -R -n -E --include='*.rs' \
      'uncaught throw:|ReferenceError:|REFERENCE_ERROR_PREFIX|Error::Exception|Value::Error|ErrorObject' \
      src || true
  )"
  if [[ -n "${legacy_conversions}" ]]; then
    printf '%s\n' "${legacy_conversions}" >&2
    fail "legacy completion/error boundary must not format throws or classify exceptions by text"
  fi

  typed_variant="$(
    grep -E '^    JavaScript ' "${repo_root}/src/error.rs" \
      | tr -d '[:space:]'
  )"
  if [[ "${typed_variant}" != 'JavaScript{exception:Box<JavaScriptException>},' ]]; then
    fail "typed JavaScript error boundary changed; expected one identity-bound Value variant with structured metadata and source span"
  fi
  exception_fields="$({
    awk '
      /^pub struct JavaScriptException/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/error.rs"
  } | tr -d '[:space:]')"
  if [[ "${exception_fields}" != 'identity:Option<VmIdentity>,value:Value,metadata:Option<Box<JavaScriptErrorMetadata>>,display:Box<str>,span:Option<Box<SourceSpan>>,' ]]; then
    fail "typed JavaScript error boundary changed; JavaScriptException fields must stay private and identity-bound"
  fi

  owners="$(
    function_owners \
      'fn[[:space:]]+(runtime_exception_value|reference_error_undefined|reference_error_uninitialized)[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  expected_owners='src/runtime/control/assertions.rs:reference_error_undefined
src/runtime/control/assertions.rs:reference_error_uninitialized
src/runtime/control/assertions.rs:runtime_exception_value'
  compare_set "completion/error operation allowlist" "${owners}" "${expected_owners}"

  local local_value_fields
  local host_call_fields
  local_value_fields="$({
    awk '
      /^pub struct LocalValue/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/api/host.rs"
  } | tr -d '[:space:]')"
  if [[ "${local_value_fields}" != "identity:&'valueVmIdentity,value:&'valueValue," ]]; then
    fail "host local-value boundary changed; LocalValue requires borrowed owner identity and Value"
  fi
  host_call_fields="$({
    awk '
      /^pub struct HostCall/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/api/host.rs"
  } | tr -d '[:space:]')"
  if [[ "${host_call_fields}" != "function_name:&'callstr,identity:&'callVmIdentity,args:&'call[Value]," ]]; then
    fail "host local-value boundary changed; HostCall requires the active VM identity"
  fi
  if ! grep -F -q 'Error::javascript_local(self.identity.clone(), self.value.clone())' \
      "${repo_root}/src/api/host.rs" \
    || ! grep -F -q 'identity != context.identity()' \
      "${repo_root}/src/runtime/control/assertions.rs"; then
    fail "host local-value boundary changed; JavaScript host errors require owner validation"
  fi
}

check_state_owner_allowlists() {
  local context_fields
  local expected_context_fields
  local object_fields
  local expected_object_fields
  local context_owners

  context_fields="$(
    awk '
      /^pub struct Context \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/mod.rs" \
      | sed -nE \
        's/^[[:space:]]*(pub\(crate\)[[:space:]]+)?([a-z_][a-z0-9_]*):.*/\2/p'
  )"
  expected_context_fields='identity
limits
atoms
strings
symbols
well_known_properties
iterator_symbol
descriptor_property_keys
static_name_atom_caches
static_binding_caches
static_binding_layouts
globals
builtin_globals
locals
local_frame_bases
upvalue_frames
functions
native_functions
native_function_registry
bound_functions
host_functions
objects
global_object
collections
collection_object_slots
collection_iterators
promises
promise_object_slots
promise_jobs
promise_prototype
this_values
new_target_values
super_frames
output
performance_clock
random_state
runtime_steps
bytecode_linear_segment_runs
bytecode_linear_direct_runs
call_depth
native_call_cache_hits
native_call_cache_misses
native_call_cache_slow_paths
call_value_cache_hits
call_value_cache_misses
call_value_cache_slow_paths'
  compare_set "Context state-owner field allowlist" "${context_fields}" "${expected_context_fields}"

  object_fields="$(
    awk '
      /^struct Object \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/object/mod.rs" \
      | sed -nE 's/^[[:space:]]*([a-z_][a-z0-9_]*):.*/\1/p'
  )"
  expected_object_fields='named_properties
array_storage
shape
enumerable_property_count
array_length
array_length_writable
string_value
primitive_value
error_metadata
date_value
regexp_value
proxy_value
byte_buffer
uint8_array
is_raw_json
prototype
extensibility'
  compare_set "Object payload field allowlist" "${object_fields}" "${expected_object_fields}"

  context_owners="$(
    cd "${repo_root}"
    grep -R -H -E -o --include='*.rs' \
      '^[[:space:]]*context:[[:space:]]*Context,' src || true
  )"
  context_owners="$(
    printf '%s\n' "${context_owners}" \
      | sed -E 's/:[[:space:]]*context:[[:space:]]*Context,/:context:Context,/'
  )"
  compare_set "public Context owner allowlist" "${context_owners}" \
    'src/api/embedding.rs:context:Context,'

  if [[ "$(clone_derive_count "${repo_root}/src/runtime/mod.rs" 'pub struct Context {')" != "0" ]]; then
    fail "Context clone boundary changed; VM-owned state must remain non-cloneable"
  fi
  if [[ "$(clone_derive_count "${repo_root}/src/api/embedding.rs" 'pub struct Vm {')" != "0" ]]; then
    fail "Vm clone boundary changed; VM-owned state must remain non-cloneable"
  fi

  if ! grep -F -q 'owner: Rc<VmOwnerToken>,' "${repo_root}/src/ownership.rs" \
    || ! grep -F -q 'generation: VmGeneration,' "${repo_root}/src/ownership.rs"; then
    fail "VM identity boundary changed; identity requires one owner capability and generation"
  fi

  local primitive_owner_fields
  primitive_owner_fields="$({
    grep -F '    identity: VmIdentity,' "${repo_root}/src/storage/string_heap.rs" || true
    grep -F '    identity: VmIdentity,' "${repo_root}/src/storage/symbol.rs" || true
  } | wc -l | tr -d '[:space:]')"
  if [[ "${primitive_owner_fields}" != "4" ]]; then
    fail "VM primitive owner boundary changed; JsString, StringHeap, JsSymbol, and SymbolTable require identity"
  fi
  if ! grep -F -q 'if text.identity() != self.identity()' \
      "${repo_root}/src/runtime/values.rs" \
    || ! grep -F -q 'if symbol.identity() != self.identity()' \
      "${repo_root}/src/runtime/values.rs"; then
    fail "VM primitive validation boundary changed; checked values must reject foreign strings and Symbols"
  fi
}

clone_derive_count() {
  local path="$1"
  local declaration="$2"
  awk -v declaration="${declaration}" '
    previous == "#[derive(Debug, Clone)]" && $0 == declaration { count += 1 }
    { previous = $0 }
    END { print count + 0 }
  ' "${path}"
}

check_optimization_owner_allowlists() {
  local control_files
  local expected_control_files
  local linear_files
  local expected_linear_files
  local fast_path_files
  local expected_fast_path_files

  control_files="$(
    cd "${repo_root}"
    find src/runtime/bytecode/control -maxdepth 1 -type f -name '*.rs' -printf '%p\n'
  )"
  expected_control_files='src/runtime/bytecode/control/array_add_loop.rs
src/runtime/bytecode/control/array_fill_loop.rs
src/runtime/bytecode/control/block_lexical_loop.rs
src/runtime/bytecode/control/compound_assignment_loop.rs
src/runtime/bytecode/control/constructor_prototype_loop.rs
src/runtime/bytecode/control/for_loop.rs
src/runtime/bytecode/control/function_apply_has_instance_loop.rs
src/runtime/bytecode/control/loop_helpers.rs
src/runtime/bytecode/control/object_literal_loop.rs
src/runtime/bytecode/control/string_concat_loop.rs
src/runtime/bytecode/control/switch_for_loop.rs
src/runtime/bytecode/control/try_catch.rs
src/runtime/bytecode/control/try_catch_loop.rs
src/runtime/bytecode/control/try_finally_loop.rs
src/runtime/bytecode/control/update_expression_loop.rs
src/runtime/bytecode/control/while_loop.rs'
  compare_set "structured-control optimization owner allowlist" \
    "${control_files}" "${expected_control_files}"

  linear_files="$(
    cd "${repo_root}"
    find src/runtime/bytecode/linear -maxdepth 1 -type f -name '*.rs' -printf '%p\n'
  )"
  expected_linear_files='src/runtime/bytecode/linear/direct.rs
src/runtime/bytecode/linear/in_operator.rs
src/runtime/bytecode/linear/mod.rs
src/runtime/bytecode/linear/numeric_chain.rs
src/runtime/bytecode/linear/property_chain.rs
src/runtime/bytecode/linear/property_numeric.rs
src/runtime/bytecode/linear/segment.rs'
  compare_set "linear optimization owner allowlist" "${linear_files}" "${expected_linear_files}"

  fast_path_files="$(
    cd "${repo_root}"
    find src -type f -name '*fast_path*.rs' -printf '%p\n'
  )"
  expected_fast_path_files='src/bytecode/fast_path.rs
src/runtime/function/callback_fast_path.rs
src/runtime/function/fast_path.rs'
  compare_set "fast-path owner allowlist" "${fast_path_files}" "${expected_fast_path_files}"
}

run_checks() {
  require_file src/value/kind.rs
  require_file src/runtime/mod.rs
  require_file src/ownership.rs
  require_file src/source.rs
  require_file src/ast/node.rs
  require_file src/bytecode/block.rs
  require_file src/runtime/object/mod.rs
  require_file src/api/embedding.rs
  require_dir src/compiler
  require_dir src/bytecode
  require_dir src/runtime/bytecode/control
  require_dir src/runtime/bytecode/linear

  check_value_representation
  check_runtime_frontend_boundary
  check_source_metadata_boundary
  check_frontend_span_boundary
  check_bytecode_span_boundary
  check_harness_boundaries
  check_semantic_duplicate_allowlists
  check_completion_error_boundary
  check_state_owner_allowlists
  check_optimization_owner_allowlists
  printf '%s: ok\n' "${script_name}"
}

prepare_fixture() {
  local fixture_root="$1"
  rm -rf "${fixture_root}"
  mkdir -p "${fixture_root}"
  cp -R "${repo_root}/src" "${fixture_root}/src"
}

mutate_value_variant() {
  local fixture_root="$1"
  sed -i '/    Object(ObjectId),/i\    FutureObject(ObjectId),' \
    "${fixture_root}/src/value/kind.rs"
}

mutate_runtime_frontend_import() {
  local fixture_root="$1"
  printf '\nuse crate::ast::Expr;\n' >>"${fixture_root}/src/runtime/mod.rs"
}

mutate_frontend_source_span() {
  local fixture_root="$1"
  sed -i 's/Lex { message: String, span: SourceSpan }/Lex { message: String, offset: usize }/' \
    "${fixture_root}/src/error.rs"
}

mutate_frontend_ast_span() {
  local fixture_root="$1"
  sed -i 's/pub type Expression = AstNode<Expr>;/pub type Expression = Expr;/' \
    "${fixture_root}/src/ast/expression.rs"
}

mutate_bytecode_source_span() {
  local fixture_root="$1"
  sed -i '/    spans: Rc<\[SourceSpan\]>,/d' \
    "${fixture_root}/src/bytecode/block.rs"
}

mutate_compiler_source_name() {
  local fixture_root="$1"
  printf '\nfn architecture_probe(name: &str) { let _ = name.as_str() == "benchmark"; }\n' \
    >>"${fixture_root}/src/compiler/call.rs"
}

mutate_equality_duplicate() {
  local fixture_root="$1"
  printf '\nfn same_value_zero_architecture_probe() {}\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_conversion_duplicate() {
  local fixture_root="$1"
  printf '\nfn is_truthy(value: &Value) -> bool { matches!(value, Value::Bool(true)) }\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_invocation_predicate() {
  local fixture_root="$1"
  printf '\nfn is_callable_architecture_probe() {}\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_semantic_object_facade() {
  local fixture_root="$1"
  printf '\nfn is_object_like_architecture_probe() {}\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_index_helper() {
  local fixture_root="$1"
  printf '\nfn to_length_architecture_probe() {}\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_property_call_owner() {
  local fixture_root="$1"
  printf '\nfn get_method() {}\n' \
    >>"${fixture_root}/src/runtime/abstract_operations/conversion.rs"
}

mutate_legacy_property_call_facade() {
  local fixture_root="$1"
  printf '\nfn eval_call_value() {}\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_iterator_owner() {
  local fixture_root="$1"
  printf '\nfn iterator_step() {}\n' \
    >>"${fixture_root}/src/runtime/abstract_operations/conversion.rs"
}

mutate_legacy_iterator_facade() {
  local fixture_root="$1"
  printf '\nfn for_of_step() {}\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_legacy_completion_conversion() {
  local fixture_root="$1"
  printf '\nconst LEGACY_COMPLETION_PROBE: &str = "uncaught throw: probe";\n' \
    >>"${fixture_root}/src/runtime/values.rs"
}

mutate_host_local_value_identity() {
  local fixture_root="$1"
  sed -i '/    identity: &.value VmIdentity,/d' \
    "${fixture_root}/src/api/host.rs"
}

mutate_javascript_exception_visibility() {
  local fixture_root="$1"
  sed -i '0,/    identity: Option<VmIdentity>,/{s/    identity: Option<VmIdentity>,/    pub identity: Option<VmIdentity>,/}' \
    "${fixture_root}/src/error.rs"
}

mutate_test262_source_name() {
  local fixture_root="$1"
  printf '\nconst ARCHITECTURE_PROBE: &str = TEST262_ERROR_NAME;\n' \
    >>"${fixture_root}/src/runtime/mod.rs"
}

mutate_context_store() {
  local fixture_root="$1"
  sed -i '/    objects: ObjectHeap,/a\    future_objects: Vec<Value>,' \
    "${fixture_root}/src/runtime/mod.rs"
}

mutate_object_payload() {
  local fixture_root="$1"
  sed -i '/    proxy_value: Option<ProxyValue>,/a\    future_value: Option<Value>,' \
    "${fixture_root}/src/runtime/object/mod.rs"
}

mutate_context_owner() {
  local fixture_root="$1"
  printf '\n#[derive(Debug, Clone)]\npub struct VmAlias {\n    context: Context,\n}\n' \
    >>"${fixture_root}/src/api/embedding.rs"
}

mutate_context_clone_marker() {
  local fixture_root="$1"
  sed -i '0,/^#\[derive(Debug)\]$/{s/^#\[derive(Debug)\]$/#[derive(Debug, Clone)]/}' \
    "${fixture_root}/src/runtime/mod.rs"
}

mutate_vm_clone_marker() {
  local fixture_root="$1"
  sed -i '0,/^#\[derive(Debug)\]$/{s/^#\[derive(Debug)\]$/#[derive(Debug, Clone)]/}' \
    "${fixture_root}/src/api/embedding.rs"
}

mutate_vm_identity_owner() {
  local fixture_root="$1"
  sed -i 's/owner: Rc<VmOwnerToken>,/owner: Rc<ForeignOwnerToken>,/' \
    "${fixture_root}/src/ownership.rs"
}

mutate_vm_primitive_owner() {
  local fixture_root="$1"
  sed -i '0,/    identity: VmIdentity,/{/    identity: VmIdentity,/d;}' \
    "${fixture_root}/src/storage/string_heap.rs"
}

mutate_control_owner() {
  local fixture_root="$1"
  printf 'pub(super) fn benchmark_loop_architecture_probe() {}\n' \
    >"${fixture_root}/src/runtime/bytecode/control/benchmark_loop.rs"
}

mutate_linear_owner() {
  local fixture_root="$1"
  printf 'pub(super) fn benchmark_linear_architecture_probe() {}\n' \
    >"${fixture_root}/src/runtime/bytecode/linear/benchmark.rs"
}

mutate_fast_path_owner() {
  local fixture_root="$1"
  printf 'pub(super) fn benchmark_fast_path_architecture_probe() {}\n' \
    >"${fixture_root}/src/runtime/benchmark_fast_path.rs"
}

mutate_harness_opcode_owner() {
  local fixture_root="$1"
  printf 'fn architecture_probe(value: BytecodeInstruction) { let _ = BytecodeInstruction::Print { arg_count: 0 }; drop(value); }\n' \
    >"${fixture_root}/src/runtime/benchmark_harness.rs"
}

expect_guard_failure() {
  local temp_dir="$1"
  local name="$2"
  local marker="$3"
  local mutator="$4"
  local fixture_root="${temp_dir}/fixture"
  local output="${temp_dir}/guard-output"

  prepare_fixture "${fixture_root}"
  "${mutator}" "${fixture_root}"
  if RSQJS_ARCHITECTURE_ROOT="${fixture_root}" "${BASH_SOURCE[0]}" >"${output}" 2>&1; then
    fail "self-test '${name}' did not reject its mutation"
  fi
  if ! grep -F -q "${marker}" "${output}"; then
    printf '%s: self-test output for %s\n' "${script_name}" "${name}" >&2
    sed -n '1,120p' "${output}" >&2
    fail "self-test '${name}' failed for the wrong boundary"
  fi
  printf '%s: self-test ok: %s\n' "${script_name}" "${name}"
}

run_self_tests() {
  local temp_dir
  run_checks
  temp_dir="$(mktemp -d)"
  trap "rm -rf '${temp_dir}'" EXIT

  expect_guard_failure "${temp_dir}" value-representation \
    'Value representation changed' mutate_value_variant
  expect_guard_failure "${temp_dir}" runtime-frontend \
    'runtime/frontend boundary changed' mutate_runtime_frontend_import
  expect_guard_failure "${temp_dir}" frontend-source-span \
    'frontend source diagnostic boundary changed' mutate_frontend_source_span
  expect_guard_failure "${temp_dir}" frontend-ast-span \
    'frontend AST span boundary changed' mutate_frontend_ast_span
  expect_guard_failure "${temp_dir}" bytecode-source-span \
    'bytecode source span boundary changed' mutate_bytecode_source_span
  expect_guard_failure "${temp_dir}" compiler-source-name \
    'compiler source-name allowlist changed' mutate_compiler_source_name
  expect_guard_failure "${temp_dir}" test262-source-name \
    'Test262 source-name allowlist changed' mutate_test262_source_name
  expect_guard_failure "${temp_dir}" equality-duplicate \
    'equality operation allowlist changed' mutate_equality_duplicate
  expect_guard_failure "${temp_dir}" conversion-duplicate \
    'primitive conversion operation allowlist changed' mutate_conversion_duplicate
  expect_guard_failure "${temp_dir}" invocation-predicate \
    'callable/constructor predicate allowlist changed' mutate_invocation_predicate
  expect_guard_failure "${temp_dir}" semantic-object-facade \
    'semantic object facade allowlist changed' mutate_semantic_object_facade
  expect_guard_failure "${temp_dir}" index-helper \
    'length/integer operation allowlist changed' mutate_index_helper
  expect_guard_failure "${temp_dir}" property-call-owner \
    'property/method/call abstract-operation allowlist changed' mutate_property_call_owner
  expect_guard_failure "${temp_dir}" legacy-property-call-facade \
    'legacy property/method/call facade allowlist changed' mutate_legacy_property_call_facade
  expect_guard_failure "${temp_dir}" iterator-owner \
    'iterator abstract-operation allowlist changed' mutate_iterator_owner
  expect_guard_failure "${temp_dir}" legacy-iterator-facade \
    'legacy iterator facade allowlist changed' mutate_legacy_iterator_facade
  expect_guard_failure "${temp_dir}" legacy-completion-conversion \
    'legacy completion/error boundary' mutate_legacy_completion_conversion
  expect_guard_failure "${temp_dir}" host-local-value-identity \
    'host local-value boundary changed' mutate_host_local_value_identity
  expect_guard_failure "${temp_dir}" javascript-exception-visibility \
    'JavaScriptException fields must stay private' mutate_javascript_exception_visibility
  expect_guard_failure "${temp_dir}" context-store \
    'Context state-owner field allowlist changed' mutate_context_store
  expect_guard_failure "${temp_dir}" object-payload \
    'Object payload field allowlist changed' mutate_object_payload
  expect_guard_failure "${temp_dir}" context-owner \
    'public Context owner allowlist changed' mutate_context_owner
  expect_guard_failure "${temp_dir}" context-clone-marker \
    'Context clone boundary changed' mutate_context_clone_marker
  expect_guard_failure "${temp_dir}" vm-clone-marker \
    'Vm clone boundary changed' mutate_vm_clone_marker
  expect_guard_failure "${temp_dir}" vm-identity-owner \
    'VM identity boundary changed' mutate_vm_identity_owner
  expect_guard_failure "${temp_dir}" vm-primitive-owner \
    'VM primitive owner boundary changed' mutate_vm_primitive_owner
  expect_guard_failure "${temp_dir}" control-owner \
    'structured-control optimization owner allowlist changed' mutate_control_owner
  expect_guard_failure "${temp_dir}" linear-owner \
    'linear optimization owner allowlist changed' mutate_linear_owner
  expect_guard_failure "${temp_dir}" fast-path-owner \
    'fast-path owner allowlist changed' mutate_fast_path_owner
  expect_guard_failure "${temp_dir}" harness-opcode-owner \
    'harness opcode use-site allowlist changed' mutate_harness_opcode_owner

  printf '%s: self-tests passed\n' "${script_name}"
}

case "${1:-}" in
  '')
    run_checks
    ;;
  --self-test)
    run_self_tests
    ;;
  *)
    fail "unknown argument '$1'"
    ;;
esac
