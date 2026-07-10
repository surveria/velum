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
  expected='Undefined,Null,Bool(bool),Number(f64),String(String),HeapString(JsString),Symbol(JsSymbol),Function(FunctionId),NativeFunction(NativeFunctionId),HostFunction(HostFunctionId),Object(ObjectId),Error(ErrorObject),'
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
  expected_context_fields='limits
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

  if [[ "$(clone_derive_count "${repo_root}/src/runtime/mod.rs" 'pub struct Context {')" != "1" ]]; then
    fail "Context clone-debt marker changed; AS-05a owns removal or redesign"
  fi
  if [[ "$(clone_derive_count "${repo_root}/src/api/embedding.rs" 'pub struct Vm {')" != "1" ]]; then
    fail "Vm clone-debt marker changed; AS-05a owns removal or redesign"
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
  require_file src/runtime/object/mod.rs
  require_file src/api/embedding.rs
  require_dir src/compiler
  require_dir src/bytecode
  require_dir src/runtime/bytecode/control
  require_dir src/runtime/bytecode/linear

  check_value_representation
  check_runtime_frontend_boundary
  check_harness_boundaries
  check_semantic_duplicate_allowlists
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
  sed -i '/    Error(ErrorObject),/i\    FutureObject(ObjectId),' \
    "${fixture_root}/src/value/kind.rs"
}

mutate_runtime_frontend_import() {
  local fixture_root="$1"
  printf '\nuse crate::ast::Expr;\n' >>"${fixture_root}/src/runtime/mod.rs"
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
  sed -i '0,/^#\[derive(Debug, Clone)\]$/{s/^#\[derive(Debug, Clone)\]$/#[derive(Debug)]/}' \
    "${fixture_root}/src/runtime/mod.rs"
}

mutate_vm_clone_marker() {
  local fixture_root="$1"
  sed -i '0,/^#\[derive(Debug, Clone)\]$/{s/^#\[derive(Debug, Clone)\]$/#[derive(Debug)]/}' \
    "${fixture_root}/src/api/embedding.rs"
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
  expect_guard_failure "${temp_dir}" context-store \
    'Context state-owner field allowlist changed' mutate_context_store
  expect_guard_failure "${temp_dir}" object-payload \
    'Object payload field allowlist changed' mutate_object_payload
  expect_guard_failure "${temp_dir}" context-owner \
    'public Context owner allowlist changed' mutate_context_owner
  expect_guard_failure "${temp_dir}" context-clone-marker \
    'Context clone-debt marker changed' mutate_context_clone_marker
  expect_guard_failure "${temp_dir}" vm-clone-marker \
    'Vm clone-debt marker changed' mutate_vm_clone_marker
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
