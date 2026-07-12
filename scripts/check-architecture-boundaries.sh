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
  expected='Undefined,Null,Bool(bool),Number(f64),BigInt(JsBigInt),String(JsString),Symbol(JsSymbol),Function(FunctionId),NativeFunction(NativeFunctionId),HostFunction(HostFunctionId),Object(ObjectId),'
  if [[ "${actual}" != "${expected}" ]]; then
    fail "Value representation changed; AS-02 owns object-like representation changes"
  fi
}

check_owned_value_boundary() {
  local actual
  actual="$(
    awk '
      /^pub enum OwnedValue \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/api/owned_value.rs" \
      | sed '/^[[:space:]]*\/\//d' \
      | tr -d '[:space:]'
  )"
  if [[ "${actual}" != 'Undefined,Null,Bool(bool),Number(f64),BigInt(JsBigInt),String(String),' ]]; then
    fail "OwnedValue boundary changed; portable values must not retain VM-local ids or identity"
  fi
  if ! grep -F -q 'pub fn to_owned_value(self) -> Result<OwnedValue>' \
      "${repo_root}/src/api/host.rs" \
    || ! grep -F -q 'pub fn eval_owned(&mut self, source: &str) -> Result<OwnedValue>' \
      "${repo_root}/src/api/owned_value.rs" \
    || ! grep -F -q 'impl IntoJsValue for OwnedValue' \
      "${repo_root}/src/api/host.rs"; then
    fail "OwnedValue boundary changed; local copy, evaluation, and host return conversions are required"
  fi
}

check_retained_value_boundary() {
  local handle_fields
  local registry_fields
  handle_fields="$({
    awk '
      /^pub struct RetainedValue \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/retained_values.rs"
  } | sed '/^[[:space:]]*\/\//d' | tr -d '[:space:]')"
  if [[ "${handle_fields}" != 'identity:VmIdentity,registry:Weak<Mutex<RetainedValueState>>,slot:RetainedSlot,slot_generation:RetainedSlotGeneration,active:bool,' ]]; then
    fail "retained value boundary changed; handle identity, private slot generation, and release state are required"
  fi
  registry_fields="$({
    awk '
      /^pub struct RetainedValueRegistry \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/retained_values.rs"
  } | tr -d '[:space:]')"
  if [[ "${registry_fields}" != 'identity:VmIdentity,state:Rc<Mutex<RetainedValueState>>,' ]]; then
    fail "retained value boundary changed; the registry requires one authoritative VM identity"
  fi

  if grep -F -q 'impl Clone for RetainedValue' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q 'match self.0.checked_add(1)' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q 'pub fn release(mut self) -> Result<()>' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q 'impl Drop for RetainedValue' \
      "${repo_root}/src/runtime/retained_values.rs"; then
    fail "retained value boundary changed; handles must be non-cloneable, generation-checked, and releasable"
  fi

  if ! grep -F -q 'pub fn eval_retained(&mut self, source: &str) -> Result<RetainedValue>' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'pub fn get_global_retained(&self, name: &str) -> Result<Option<RetainedValue>>' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'pub fn retain(self) -> Result<RetainedValue>' \
      "${repo_root}/src/api/host.rs" \
    || ! grep -F -q 'if identity != &self.identity' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q '|| &handle.identity != identity' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q '|| !handle.registry.ptr_eq' \
      "${repo_root}/src/runtime/retained_values.rs"; then
    fail "retained value boundary changed; source-proven evaluation, global, callback, and owner validation paths are required"
  fi
}

check_storage_accounting_boundary() {
  local storage_kinds
  local snapshot_fields
  local source
  storage_kinds="$({
    awk '
      /^pub enum VmStorageKind \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/accounting.rs"
  } | sed '/^[[:space:]]*\/\//d' | tr -d '[:space:]')"
  if [[ "${storage_kinds}" != 'Atom,HeapString,Symbol,Binding,JavaScriptFunction,NativeFunction,BoundFunction,HostCallback,Object,ObjectProperty,ByteBuffer,Collection,CollectionEntry,CollectionIterator,IteratorItem,Promise,PromiseReaction,PromiseJob,RetainedHandle,TransientRoot,ExecutionFrame,OutputEntry,CacheEntry,Association,Module,SourceRecord,' ]]; then
    fail "storage accounting boundary changed; VmStorageKind categories require an assigned AS migration"
  fi

  snapshot_fields="$({
    awk '
      /^pub struct VmStorageSnapshot \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/accounting.rs"
  } | tr -d '[:space:]')"
  if [[ "${snapshot_fields}" != 'counts:[usize;STORAGE_KIND_COUNT],payload_bytes:[usize;STORAGE_KIND_COUNT],total:usize,total_payload_bytes:usize,' ]]; then
    fail "storage accounting boundary changed; snapshot requires checked category and total counts and payload bytes"
  fi

  for source in \
    'counter.record(VmStorageKind::Atom, self.atoms.len())?;' \
    'counter.record(VmStorageKind::HeapString, self.strings.len())?;' \
    'counter.record(VmStorageKind::Symbol, self.symbols.len())?;' \
    'for realm in self.realm_states() {' \
    'counter.record(VmStorageKind::Binding, realm.binding_count()?)?;' \
    'for scope in &self.locals {' \
    'for frame in &self.activation_frames {' \
    'for function in &self.functions {' \
    'for function in &self.native_functions {' \
    'counter.record(VmStorageKind::BoundFunction, self.bound_functions.len())?;' \
    'counter.record(VmStorageKind::HostCallback, self.host_functions.len())' \
    'let object_counts = self.objects.storage_counts()?;' \
    'counter.record(VmStorageKind::Collection, self.collections.len())?;' \
    'self.collection_storage_entry_count()?,' \
    'self.collection_iterators.len(),' \
    'self.collection_iterator_item_count()?,' \
    'counter.record(VmStorageKind::Promise, self.promises.len())?;' \
    'self.promise_reaction_count()?,' \
    'counter.record(VmStorageKind::PromiseJob, self.promise_jobs.len())?;' \
    'self.retained_values.active_count(),' \
    'self.transient_roots.active_count(),' \
    'self.activation_frames' \
    'counter.record(VmStorageKind::OutputEntry, self.output.len())' \
    'self.well_known_properties.entry_count(),' \
    'self.atoms.index_entry_count()' \
    'self.strings.index_entry_count()' \
    'counter.record(VmStorageKind::CacheEntry, realm.cache_entry_count()?)?;' \
    'if let Some(keys) = self.descriptor_property_keys {' \
    'for cache in &self.static_name_atom_caches {' \
    'for cache in &self.static_binding_caches {' \
    'for layout in &self.static_binding_layouts {' \
    'self.inactive_realms.len().saturating_sub(1),' \
    'self.collection_object_slots.iter().flatten().count(),' \
    'self.symbols.registry_entry_count(),' \
    'self.well_known_symbols.len())?;' \
    'self.promise_object_slots.iter().flatten().count(),' \
    'counter.record(VmStorageKind::Association, realm.association_count())?;' \
    'usize::from(self.iterator_symbol.is_some()),' \
    'counter.record(VmStorageKind::Module, self.modules.len())'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/accounting.rs"; then
      fail "storage accounting boundary changed; required owner source '${source}' is missing"
    fi
  done

  if ! grep -F -q 'self.bytes = updated_bytes;' \
      "${repo_root}/src/storage/atom.rs" \
    || ! grep -F -q 'self.name.len()' \
      "${repo_root}/src/api/host.rs" \
    || ! grep -F -q '.pattern()' \
      "${repo_root}/src/runtime/object/accounting.rs" \
    || ! grep -F -q '.flags().len()' \
      "${repo_root}/src/runtime/object/accounting.rs" \
    || ! grep -F -q 'super::ByteBuffer::byte_length' \
      "${repo_root}/src/runtime/object/accounting.rs"; then
    fail "storage accounting boundary changed; payload producers must maintain their logical byte sources"
  fi

  for source in \
    'context.record_storage_payload_bytes(&mut counter)?;' \
    'counter.record_payload_bytes(VmStorageKind::Atom, self.atoms.bytes())?;' \
    'counter.record_payload_bytes(VmStorageKind::HeapString, self.strings.bytes())?;' \
    'self.host_callback_name_bytes()?,' \
    'object_counts.object_payload_bytes())?;' \
    'object_counts.byte_buffer_payload_bytes(),' \
    'counter.record_payload_bytes(VmStorageKind::OutputEntry, self.output_payload_bytes())?;' \
    'counter.record_payload_bytes(VmStorageKind::SourceRecord, self.source_record_bytes()?)'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/accounting.rs"; then
      fail "storage accounting boundary changed; required payload source '${source}' is missing"
    fi
  done

  if ! grep -F -q 'pub fn storage_snapshot(&self) -> Result<VmStorageSnapshot>' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'pub fn finish(self) -> Result<VmTeardownReport>' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'storage: self.storage_snapshot()?,' \
      "${repo_root}/src/api/embedding.rs"; then
    fail "storage accounting boundary changed; Vm snapshot and consuming teardown reconciliation are required"
  fi
}

check_storage_limit_boundary() {
  local object_push_count
  local source

  for source in \
    'pub storage: VmStorageLimits,' \
    'pub const fn unlimited() -> Self {' \
    'pub fn with_max_count(self, kind: VmStorageKind, limit: usize) -> Self {' \
    'pub fn with_max_payload_bytes(self, kind: VmStorageKind, limit: usize) -> Self {' \
    'VmStorageLimitPolicy::Unlimited => usize::MAX,'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/limits.rs"; then
      fail "storage limit boundary changed; public immutable per-owner policy is required"
    fi
  done

  for source in \
    'Atom record count exceeded {}' \
    'Atom payload bytes exceeded {}'; do
    if ! grep -F -q "${source}" "${repo_root}/src/storage/atom.rs"; then
      fail "storage limit boundary changed; Atom count and payload checks are required"
    fi
  done

  for source in \
    'HeapString record count exceeded {}' \
    'HeapString payload bytes exceeded {}'; do
    if ! grep -F -q "${source}" "${repo_root}/src/storage/string_heap.rs"; then
      fail "storage limit boundary changed; HeapString count and payload checks are required"
    fi
  done

  if ! grep -F -q 'Symbol record count exceeded {}' \
      "${repo_root}/src/storage/symbol.rs" \
    || ! grep -F -q 'VmStorageKind::HostCallback' \
      "${repo_root}/src/api/host.rs"; then
    fail "storage limit boundary changed; Symbol and HostCallback checks are required"
  fi

  object_push_count="$(
    grep -R -F -h 'self.objects.insert_at_next(id.index(), object)?;' \
      "${repo_root}/src/runtime/object" | wc -l
  )"
  if [[ "${object_push_count}" -ne 1 ]] \
    || ! grep -F -q 'self.storage_limits.max_count(VmStorageKind::Object)' \
      "${repo_root}/src/runtime/object/heap.rs" \
    || ! grep -F -q 'self.storage_limits.max_count(VmStorageKind::ByteBuffer)' \
      "${repo_root}/src/runtime/object/heap.rs" \
    || ! grep -F -q 'self.object_payload_bytes = projected_object_bytes;' \
      "${repo_root}/src/runtime/object/heap.rs" \
    || ! grep -F -q 'self.byte_buffer_payload_bytes = projected_buffer_bytes;' \
      "${repo_root}/src/runtime/object/heap.rs"; then
    fail "storage limit boundary changed; Object and ByteBuffer growth require one checked insertion path"
  fi

  if ! grep -F -q 'VmStorageKind::OutputEntry' \
      "${repo_root}/src/runtime/mod.rs" \
    || ! grep -F -q 'self.output_payload_bytes = 0;' \
      "${repo_root}/src/runtime/globals.rs" \
    || ! grep -F -q 'crate::runtime::VmStorageKind::SourceRecord' \
      "${repo_root}/src/runtime/function/mod.rs"; then
    fail "storage limit boundary changed; OutputEntry and SourceRecord checks and release accounting are required"
  fi

  if ! grep -F -q 'state: Rc<VmStorageLedgerState>,' \
      "${repo_root}/src/runtime/storage_ledger.rs" \
    || ! grep -F -q 'counts: [Cell<usize>; STORAGE_KIND_COUNT],' \
      "${repo_root}/src/runtime/storage_ledger.rs" \
    || ! grep -F -q 'pub(in crate::runtime) fn reserve_count(' \
      "${repo_root}/src/runtime/storage_ledger.rs" \
    || ! grep -F -q 'pub(in crate::runtime) fn release_count(' \
      "${repo_root}/src/runtime/storage_ledger.rs" \
    || ! grep -F -q 'storage reservation became stale' \
      "${repo_root}/src/runtime/storage_ledger.rs"; then
    fail "storage limit boundary changed; AS-05b2c2 requires one VM-local checked O(1) ledger"
  fi

  for source in \
    'VmStorageKind::Binding,' \
    'VmStorageKind::JavaScriptFunction,' \
    'VmStorageKind::NativeFunction,' \
    'VmStorageKind::BoundFunction,' \
    'VmStorageKind::ObjectProperty,' \
    'VmStorageKind::CacheEntry,' \
    'context.ensure_durable_storage_ledger_matches(&snapshot)?;'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/accounting.rs"; then
      fail "storage limit boundary changed; AS-05b2c2 ledger categories require independent snapshot reconciliation"
    fi
  done

  if ! grep -F -q 'storage_ledger.reserve_count(VmStorageKind::Binding, 1)?;' \
      "${repo_root}/src/runtime/binding/scope.rs" \
    || ! grep -F -q 'scope.deactivate_storage()?;' \
      "${repo_root}/src/runtime/execution_storage.rs" \
    || ! grep -F -q 'reserve_count(VmStorageKind::JavaScriptFunction, 1)?;' \
      "${repo_root}/src/runtime/function/storage.rs" \
    || ! grep -F -q 'VmStorageKind::NativeFunction, 1' \
      "${repo_root}/src/runtime/native/core.rs" \
    || ! grep -F -q 'VmStorageKind::BoundFunction, 1' \
      "${repo_root}/src/runtime/call/bound.rs"; then
    fail "storage limit boundary changed; Binding and callable growth/release seams are required"
  fi

  if ! grep -F -q 'crate::runtime::VmStorageKind::ObjectProperty,' \
      "${repo_root}/src/runtime/object/mod.rs" \
    || ! grep -F -q 'self.release_property()?;' \
      "${repo_root}/src/runtime/object/property/slot.rs" \
    || ! grep -F -q 'VmStorageKind::ObjectProperty, property_count' \
      "${repo_root}/src/runtime/function/properties.rs" \
    || ! grep -F -q 'reserve_count(VmStorageKind::CacheEntry, cache_entries)?;' \
      "${repo_root}/src/runtime/object/shape.rs" \
    || ! grep -F -q 'cache.storage_entry_count()?,' \
      "${repo_root}/src/runtime/property/static_names/mod.rs" \
    || ! grep -F -q 'DESCRIPTOR_CACHE_ENTRY_COUNT' \
      "${repo_root}/src/runtime/native/builtins/object.rs"; then
    fail "storage limit boundary changed; ObjectProperty and CacheEntry growth/release seams are required"
  fi

  for source in \
    'VmStorageKind::Collection,' \
    'VmStorageKind::CollectionEntry,' \
    'VmStorageKind::CollectionIterator,' \
    'VmStorageKind::IteratorItem,' \
    'VmStorageKind::Promise,' \
    'VmStorageKind::PromiseReaction,' \
    'VmStorageKind::PromiseJob,' \
    'VmStorageKind::RetainedHandle,' \
    'VmStorageKind::TransientRoot,' \
    'VmStorageKind::ExecutionFrame,' \
    'VmStorageKind::Association,' \
    'VmStorageKind::Module,' \
    'context.ensure_storage_snapshot_within_limits(&snapshot)?;'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/accounting.rs"; then
      fail "storage limit boundary changed; AS-05b2c3 owners require ledger and full-limit reconciliation"
    fi
  done

  if ! grep -F -q 'grow_count(VmStorageKind::CollectionEntry, 1)?;' \
      "${repo_root}/src/runtime/collections.rs" \
    || ! grep -F -q 'release_count(VmStorageKind::CollectionEntry, released)?;' \
      "${repo_root}/src/runtime/collections.rs" \
    || ! grep -F -q 'grow_count(VmStorageKind::PromiseJob, reaction_count)?;' \
      "${repo_root}/src/runtime/promise/mod.rs" \
    || ! grep -F -q 'release_count(VmStorageKind::PromiseReaction, reaction_count)?;' \
      "${repo_root}/src/runtime/promise/mod.rs"; then
    fail "storage limit boundary changed; collection and Promise owner growth/release seams are required"
  fi

  if ! grep -F -q 'grow_count(VmStorageKind::RetainedHandle, 1)?;' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q 'release_count_on_drop(VmStorageKind::RetainedHandle, 1);' \
      "${repo_root}/src/runtime/retained_values.rs" \
    || ! grep -F -q 'grow_count(VmStorageKind::TransientRoot, 1)?;' \
      "${repo_root}/src/runtime/transient_roots.rs" \
    || ! grep -F -q 'release_count_on_drop(VmStorageKind::TransientRoot, released);' \
      "${repo_root}/src/runtime/transient_roots.rs" \
    || ! grep -F -q 'self.activation_frames.push(ActivationFrame::call(' \
      "${repo_root}/src/runtime/execution_storage.rs" \
    || ! grep -F -q '.push(ActivationFrame::bytecode(continuation, with_environments));' \
      "${repo_root}/src/runtime/bytecode/continuation.rs" \
    || ! grep -F -q 'release_count(VmStorageKind::ExecutionFrame, 1)?;' \
      "${repo_root}/src/runtime/bytecode/continuation.rs"; then
    fail "storage limit boundary changed; retained, transient, and execution owners require scoped release"
  fi

  if ! grep -F -q 'VmStorageKind::Association, 1' \
      "${repo_root}/src/runtime/globals.rs" \
    || ! grep -F -q 'counter.record(VmStorageKind::Module, self.modules.len())' \
      "${repo_root}/src/runtime/accounting.rs" \
    || ! grep -F -q 'reserve_count(VmStorageKind::Module, graph.len())?' \
      "${repo_root}/src/runtime/module.rs"; then
    fail "storage limit boundary changed; VM associations and persistent Module records require checked owners"
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
  expected_compiled_fields='bytecode:BytecodeProgram,binding_layout:BindingLayout,usage:CompiledScriptUsage,source_id:SourceId,source_name:Option<String>,strict:bool,'
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
  expected_token_fields='pubkind:TokenKind,pubspan:SourceSpan,publine_terminator_before:bool,pubidentifier_escaped:bool,'
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
      "${repo_root}/src/parser/sequence.rs" \
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
  expected_comparisons=''
  compare_set "compiler source-name allowlist" "${compiler_comparisons}" "${expected_comparisons}"

  harness_paths="$(
    cd "${repo_root}"
    grep -R -l -E --include='*.rs' \
      'BytecodeInstruction::(Print|AssertThrows)|Self::(Print|AssertThrows)[[:space:]]*\{' \
      src/bytecode src/compiler src/runtime || true
  )"
  expected_harness_paths=''
  compare_set "harness opcode use-site allowlist" "${harness_paths}" "${expected_harness_paths}"
  if grep -E -q '^[[:space:]]+(Print|AssertThrows)[[:space:]]*\{' \
      "${repo_root}/src/bytecode/types.rs"; then
    fail "harness opcode boundary changed; harness-only bytecode variants are forbidden"
  fi

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
  local dynamic_compilation_owners
  local dynamic_compilation_users

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

  dynamic_compilation_owners="$(
    function_owners 'fn[[:space:]]+dynamic_compilation_error[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "dynamic compilation error owner allowlist" \
    "${dynamic_compilation_owners}" \
    'src/runtime/native/builtins/mod.rs:dynamic_compilation_error'
  dynamic_compilation_users="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' '.map_err(dynamic_compilation_error)' \
      src/runtime/native/builtins | sort
  )"
  compare_set "dynamic compilation error user allowlist" \
    "${dynamic_compilation_users}" \
    'src/runtime/native/builtins/eval.rs
src/runtime/native/builtins/function_constructor.rs
src/runtime/native/builtins/shadow_realm.rs'
  if grep -R -q -F --include='*.rs' 'generated_function_syntax_error' \
      "${repo_root}/src/runtime/native/builtins"; then
    fail "dynamic compilation error boundary changed; constructors must use the shared owner"
  fi

  local local_value_fields
  local host_call_fields
  local_value_fields="$({
    awk '
      /^pub struct LocalValue/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/api/host.rs"
  } | tr -d '[:space:]')"
  if [[ "${local_value_fields}" != "identity:&'valueVmIdentity,retained_values:&'valueRetainedValueRegistry,value:&'valueValue," ]]; then
    fail "host local-value boundary changed; LocalValue requires borrowed owner, retained registry, and Value"
  fi
  host_call_fields="$({
    awk '
      /^pub struct HostCall/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/api/host.rs"
  } | tr -d '[:space:]')"
  if [[ "${host_call_fields}" != "function_name:&'callstr,identity:&'callVmIdentity,retained_values:&'callRetainedValueRegistry,roots:VmRootSnapshot,args:&'call[Value]," ]]; then
    fail "host local-value boundary changed; HostCall requires the active VM identity and retained registry"
  fi
  if ! grep -F -q 'Error::javascript_local(self.identity.clone(), self.value.clone())' \
      "${repo_root}/src/api/host.rs" \
    || ! grep -F -q 'identity != context.identity()' \
      "${repo_root}/src/runtime/control/assertions.rs"; then
    fail "host local-value boundary changed; JavaScript host errors require owner validation"
  fi
}

check_function_accessor_boundary() {
  local definition_owners
  definition_owners="$(
    function_owners 'fn[[:space:]]+define_function_property_key[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "function accessor owner allowlist" \
    "${definition_owners}" \
    'src/runtime/function/property_dispatch.rs:define_function_property_key'

  if grep -R -q -F --include='*.rs' \
      'class static accessors are not supported yet' \
      "${repo_root}/src"; then
    fail "function accessor boundary changed; class accessors must not regain a split rejection"
  fi
  if grep -R -q -F --include='*.rs' \
      'accessor properties are not supported on function objects' \
      "${repo_root}/src"; then
    fail "function accessor boundary changed; JavaScript functions require shared accessor descriptors"
  fi

  for source in \
    'property: ObjectProperty,' \
    'ObjectProperty::from_update(update)' \
    'self.get_function_property_lookup(*id, receiver, property)?' \
    'self.function_inheritance_prototype_value(*id)?' \
    'write_function_property_with_receiver('; do
    if ! grep -R -q -F --include='*.rs' "${source}" "${repo_root}/src/runtime"; then
      fail "function accessor boundary changed; required shared source '${source}' is missing"
    fi
  done
}

check_sequence_expression_boundary() {
  local parser_owners
  local compiler_owners
  local compiler_body
  local bytecode_owners

  parser_owners="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' \
      'self.expression_node(start, Expr::Sequence(expressions))' src/parser || true
  )"
  compare_set "sequence expression parser owner allowlist" \
    "${parser_owners}" \
    'src/parser/sequence.rs'

  compiler_owners="$(
    cd "${repo_root}"
    grep -R -l -E --include='*.rs' \
      'fn[[:space:]]+compile_sequence_expr[[:space:]]*\(' src/compiler || true
  )"
  compare_set "sequence expression compiler owner allowlist" \
    "${compiler_owners}" \
    'src/compiler/expression.rs'
  compiler_body="$(
    awk '
      /fn compile_sequence_expr/ { inside = 1 }
      inside { print }
      inside && /^    }/ { exit }
    ' "${repo_root}/src/compiler/expression.rs"
  )"

  bytecode_owners="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' 'Sequence' src/bytecode || true
  )"
  compare_set "sequence expression bytecode owner allowlist" \
    "${bytecode_owners}" \
    ''

  if ! grep -F -q 'Sequence(Vec<Expression>),' \
      "${repo_root}/src/ast/expression.rs" \
    || ! grep -F -q 'pub(super) fn assignment_expression(&mut self)' \
      "${repo_root}/src/parser/sequence.rs" \
    || ! grep -F -q 'self.emit(BytecodeInstruction::Pop);' <<<"${compiler_body}" \
    || ! grep -F -q 'Expr::Sequence(expressions)' \
      "${repo_root}/src/binding_layout/builder.rs" \
    || ! grep -F -q 'Expr::Sequence(expressions)' \
      "${repo_root}/src/compiler/function.rs" \
    || ! grep -F -q 'ForHeadKind::Of => self.assignment_expression(),' \
      "${repo_root}/src/parser/statement/for_statement.rs" \
    || ! grep -F -q 'enum AwaitExpressionContext {' \
      "${repo_root}/src/parser/await_context.rs" \
    || ! grep -F -q 'pub(super) is_simple: bool,' \
      "${repo_root}/src/parser/function.rs" \
    || ! grep -F -q 'YIELD_IDENTIFIER_NAME' \
      "${repo_root}/src/parser/strict.rs"; then
    fail "sequence expression boundary changed; one AST node must preserve assignment delimiters, early errors, Pop bytecode, and shared binding analysis"
  fi
}

check_named_function_binding_boundary() {
  local runtime_owners
  runtime_owners="$(
    function_owners 'fn[[:space:]]+named_function_self_scope[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "named function self-binding owner allowlist" \
    "${runtime_owners}" \
    'src/runtime/function/storage.rs:named_function_self_scope'

  for source in \
    'name: Option<StaticBinding>,' \
    'self.declare(self_scope, self_binding)?;' \
    'self_binding: Option<StaticBinding>,' \
    'BindingCell::named_function' \
    'if init.bytecode.self_binding().is_some() {' \
    'usize::from(function.self_binding.is_some()).saturating_mul(2)' \
    'self.set_generated_function_name(id, GENERATED_FUNCTION_NAME)?;' \
    'function_source(params, body, kind, None)' \
    'pub(crate) fn compile_eval(' \
    'strict_write: bool,'; do
    if ! grep -R -q -F --include='*.rs' "${source}" "${repo_root}/src"; then
      fail "named function binding boundary changed; required typed source '${source}' is missing"
    fi
  done

  if grep -R -q -E --include='*.rs' \
      'self_binding[^;]*(as_str|==[[:space:]]*"|matches!.*")|named_function[^;]*as_str' \
      "${repo_root}/src/runtime"; then
    fail "named function binding boundary changed; runtime self bindings must use compiled slots rather than source-name comparisons"
  fi
}

check_function_name_inference_boundary() {
  local runtime_owners
  local compiler_owners
  local generated_name_users

  runtime_owners="$(
    function_owners 'fn[[:space:]]+set_function_name[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "function name runtime owner allowlist" \
    "${runtime_owners}" \
    'src/runtime/function/names.rs:set_function_name'

  compiler_owners="$(
    cd "${repo_root}"
    grep -R -H -E -o --include='*.rs' \
      'fn[[:space:]]+compile_expr_with_inferred_name[[:space:]]*\(' \
      src/compiler \
      | sed -E 's/:fn[[:space:]]+/:/; s/[[:space:]]*\($//'
  )"
  compare_set "function name compiler owner allowlist" \
    "${compiler_owners}" \
    'src/compiler/inferred_name.rs:compile_expr_with_inferred_name'

  generated_name_users="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' 'set_generated_function_name(' src/runtime | sort
  )"
  compare_set "generated function name user allowlist" \
    "${generated_name_users}" \
    'src/runtime/function/mod.rs
src/runtime/function/names.rs
src/runtime/native/builtins/function_constructor.rs'

  for source in \
    'infer_name: bool,' \
    'ComputedInferredName,' \
    'compile_expression_with_inferred_name(' \
    'set_function_name_from_property(' \
    'function_name_from_property(' \
    'PropertyKey::symbol_id' \
    'self.set_function_name(&function, &function_name, prefix)?;' \
    'self.set_function_name_from_property(&value, &property, accessor)?;'; do
    if ! grep -R -q -F --include='*.rs' "${source}" "${repo_root}/src"; then
      fail "function name inference boundary changed; required shared source '${source}' is missing"
    fi
  done

  if grep -R -q -F --include='*.rs' \
      'set_computed_method_name' "${repo_root}/src"; then
    fail "function name inference boundary changed; computed methods must use the shared SetFunctionName owner"
  fi
}

check_destructuring_assignment_boundary() {
  local runtime_owners
  local reference_owners
  local compiler_users

  runtime_owners="$(
    function_owners 'fn[[:space:]]+eval_resumable_destructure[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "destructuring runtime owner allowlist" \
    "${runtime_owners}" \
    'src/runtime/bytecode/destructure.rs:eval_resumable_destructure'

  reference_owners="$(
    function_owners 'fn[[:space:]]+eval_bytecode_assignment_reference[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "assignment reference owner allowlist" \
    "${reference_owners}" \
    'src/runtime/bytecode/ops/assignment.rs:eval_bytecode_assignment_reference'

  compiler_users="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' \
      'BytecodeInstruction::DestructurePattern {' src/compiler || true
  )"
  compare_set "destructuring compiler user allowlist" \
    "${compiler_users}" \
    'src/compiler/expression.rs
src/compiler/mod.rs'

  for source in \
    'pub enum AssignmentPattern {' \
    'DestructuringAssignment {' \
    'Assignment(BytecodeAssignmentTarget),' \
    'pub enum BytecodeDestructureMode {' \
    'compile_assignment_pattern(' \
    'assignment_reference_for_pattern(' \
    'assign_bytecode_or_create_sloppy_global(' \
    'completion = self.iterator_close(source, completion)?;'; do
    if ! grep -R -q -F --include='*.rs' "${source}" "${repo_root}/src"; then
      fail "destructuring assignment boundary changed; required shared source '${source}' is missing"
    fi
  done

  if grep -R -q -F --include='*.rs' 'BytecodeAssignmentPattern' "${repo_root}/src"; then
    fail "destructuring assignment boundary changed; assignment and binding patterns must share one bytecode walker"
  fi
}

check_update_numeric_coercion_boundary() {
  local runtime_owners

  runtime_owners="$(
    function_owners 'fn[[:space:]]+bytecode_update_values[[:space:]]*\(' \
      | sed -E 's/[[:space:]]*\($//'
  )"
  compare_set "update numeric coercion owner allowlist" \
    "${runtime_owners}" \
    'src/runtime/bytecode/ops/assignment.rs:bytecode_update_values'

  for source in \
    'match self.to_numeric(value)? {' \
    'context.bytecode_update_values(value, op)' \
    'context.bytecode_update_values(old_value, op)' \
    'let (old_value, new_value) = self.bytecode_update_values(&old_value, op)?;' \
    'let (current, updated) = self.bytecode_update_values(&current, op)?;'; do
    if ! grep -R -q -F --include='*.rs' "${source}" "${repo_root}/src/runtime/bytecode"; then
      fail "update numeric coercion boundary changed; required shared source '${source}' is missing"
    fi
  done
}

check_direct_root_boundary() {
  local root_kinds
  local source
  root_kinds="$({
    awk '
      /^pub enum VmRootKind \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/roots.rs"
  } | sed '/^[[:space:]]*\/\//d' | tr -d '[:space:]')"
  if [[ "${root_kinds}" != 'GlobalBinding,BuiltinBinding,LocalBinding,ModuleBinding,CapturedBinding,ActiveThis,ActiveNewTarget,ActiveSuper,BytecodeFrame,QueuedJob,RuntimeAnchor,RetainedHandle,TransientOperand,TransientCall,TransientTemporary,' ]]; then
    fail "direct root boundary changed; VmRootKind categories require an assigned AS migration"
  fi

  for source in \
    'for realm in self.realm_states() {' \
    'visit_scope(&realm.globals, VmRootKind::GlobalBinding, visitor)?;' \
    'visit_scope(&realm.builtin_globals, VmRootKind::BuiltinBinding, visitor)?;' \
    'for scope in &self.locals {' \
    'for module in &self.modules {' \
    'visit_scope(module.scope(), VmRootKind::ModuleBinding, visitor)?;' \
    'visitor.visit_value(VmRootKind::ModuleBinding, module.namespace())?;' \
    'for frame in &self.activation_frames {' \
    'if let Some(upvalues) = frame.upvalues() {' \
    'if let Some(value) = frame.this_value() {' \
    'if let Some(value) = frame.new_target() {' \
    'if let Some(super_binding) = frame.super_binding() {' \
    'if let Some(continuation) = frame.continuation() {' \
    'if let Some(function) = continuation.function_id() {' \
    'visitor.visit_value(VmRootKind::BytecodeFrame, &Value::Function(function))?;' \
    'visitor.visit_value(VmRootKind::BytecodeFrame, value)?;' \
    'for id in realm.anchor_objects() {' \
    'if let Some(symbol) = self.iterator_symbol {' \
    'for key in self.well_known_properties.keys() {' \
    'for (_, id) in &self.well_known_symbols {' \
    'if let Some(keys) = self.descriptor_property_keys {' \
    'for id in realm.native_function_ids() {' \
    'self.objects.visit_direct_roots(visitor)?;' \
    'for job in &self.promise_jobs {' \
    'self.retained_values.visit(visitor)?;' \
    'self.transient_roots.visit(visitor)?;'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/roots.rs"; then
      fail "direct root boundary changed; required Context root source '${source}' is missing"
    fi
  done

  if ! grep -F -q 'pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>' \
      "${repo_root}/src/runtime/promise/job.rs" \
    || ! grep -F -q 'visitor.visit_promise(VmRootKind::QueuedJob, *result)?;' \
      "${repo_root}/src/runtime/promise/job.rs" \
    || ! grep -F -q 'Self::Await { continuation } => continuation.visit_direct_roots(visitor),' \
      "${repo_root}/src/runtime/promise/job.rs" \
    || ! grep -F -q 'pub fn root_snapshot(&self) -> Result<VmRootSnapshot>' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'pub const fn root_snapshot(self) -> VmRootSnapshot' \
      "${repo_root}/src/api/host.rs"; then
    fail "direct root boundary changed; jobs, Vm, and active host callbacks require one executable root contract"
  fi

  for source in \
    'pub(in crate::runtime) struct TransientRootRegistry {' \
    '#[must_use = "transient roots must stay alive across the allocation point"]' \
    'impl Drop for TransientRootScope {' \
    '.retain(|root| root.scope != self.scope);'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/transient_roots.rs"; then
      fail "direct root boundary changed; transient registry source '${source}' is missing"
    fi
  done

  if ! grep -F -q 'VmRootKind::TransientOperand,' \
      "${repo_root}/src/runtime/bytecode/execution.rs" \
    || [[ "$(grep -F -c 'VmRootKind::TransientCall,' \
      "${repo_root}/src/runtime/semantic_object/invocation.rs")" != "2" ]] \
    || ! grep -F -q 'crate::runtime::VmRootKind::TransientCall,' \
      "${repo_root}/src/api/host.rs" \
    || ! grep -F -q 'let _root_scope = self.iterator_root_scope(source)?;' \
      "${repo_root}/src/runtime/abstract_operations/iterator.rs"; then
    fail "direct root boundary changed; operand, call, host, and iterator safepoints require transient scopes"
  fi

  if ! grep -F -q 'let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;' \
      "${repo_root}/src/runtime/native/builtins/object.rs" \
    || ! grep -F -q 'roots.add_values(get.iter())?;' \
      "${repo_root}/src/runtime/native/builtins/object.rs" \
    || ! grep -F -q 'roots.add_values(' \
      "${repo_root}/src/runtime/native/builtins/object_static.rs" \
    || [[ "$(grep -F -c 'VmRootKind::TransientTemporary,' \
      "${repo_root}/src/runtime/native/builtins/proxy.rs")" != "3" ]] \
    || ! grep -F -q 'pub(in crate::runtime) const fn trace_values(&self)' \
      "${repo_root}/src/runtime/object/property/descriptor.rs"; then
    fail "direct root boundary changed; descriptor and Proxy temporary safepoints require scoped values"
  fi
}

check_activation_frame_boundary() {
  local legacy_fields
  for source in \
    'pub(in crate::runtime) enum ActivationFrame {' \
    'Call {' \
    'local_base: usize,' \
    'upvalues: FunctionUpvalues,' \
    'this_value: Value,' \
    'new_target: Value,' \
    'super_binding: Option<Rc<FunctionSuperBinding>>,' \
    'continuation: Some(BytecodeContinuationFrame::function(function)),' \
    'TemporaryThis {' \
    'EvalBoundary {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/activation.rs"; then
      fail "activation frame boundary changed; AS-06a1 call, temporary-this, and evaluation state must stay explicit"
    fi
  done

  legacy_fields="$({
    grep -E '^[[:space:]]+(local_frame_bases|upvalue_frames|this_values|new_target_values|super_frames):' \
      "${repo_root}/src/runtime/mod.rs" || true
  })"
  if [[ -n "${legacy_fields}" ]] \
    || ! grep -F -q 'activation_frames: Vec<activation::ActivationFrame>,' \
      "${repo_root}/src/runtime/mod.rs"; then
    fail "activation frame boundary changed; Context must own one activation stack instead of parallel call-state vectors"
  fi

  for source in \
    'self.push_call_activation(' \
    'self.pop_call_activation(local_base)'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/function/mod.rs"; then
      fail "activation frame boundary changed; JavaScript calls must enter and leave one explicit activation"
    fi
  done

  for source in \
    'let boundary = self.push_eval_activation_boundary()?;' \
    'let boundary_result = self.pop_eval_activation_boundary(boundary);'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/native/builtins/function_constructor.rs"; then
      fail "activation frame boundary changed; generated functions require a rooted evaluation boundary"
    fi
  done
}

check_bytecode_continuation_boundary() {
  for source in \
    'pub(in crate::runtime) struct BytecodeContinuationFrame {' \
    'program: BytecodeContinuationProgram,' \
    'parked_state: Option<Box<BytecodeState>>,' \
    'enum BytecodeContinuationProgram {' \
    'Function(FunctionId),' \
    'Block { block: BytecodeBlock },' \
    'pub(in crate::runtime) const fn function(function: FunctionId) -> Self {' \
    'pub(in crate::runtime) const fn block(block: BytecodeBlock) -> Self {' \
    'pub(super) fn ensure_running_function_continuation(' \
    'self.activation_frames' \
    '.push(ActivationFrame::bytecode(continuation, with_environments));' \
    'pub(super) fn pop_bytecode_continuation('; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/continuation.rs"; then
      fail "bytecode continuation boundary changed; AS-06a2a requires one VM-owned block and state lifecycle"
    fi
  done

  local block_clone_count
  block_clone_count="$(grep -F -c 'block.clone()' \
    "${repo_root}/src/runtime/bytecode/continuation.rs" || true)"
  if [[ "${block_clone_count}" != '2' ]]; then
    fail "bytecode continuation boundary changed; running entry requires one durable block owner and one explicit resume clone"
  fi

  for source in \
    'state.reset();' \
    'let frame = self.push_bytecode_continuation(block)?;' \
    'let outcome = match self.run_bytecode_state(block, state) {' \
    'self.pop_bytecode_continuation(frame)?;' \
    'self.ensure_running_function_continuation(function)?;' \
    'self.run_bytecode_state(block, &mut state)'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/execution.rs"; then
      fail "bytecode continuation boundary changed; synchronous execution must unwind running frames and preserve the parked-state seam"
    fi
  done
}

check_structured_control_boundary() {
  for source in \
    'control_stack: Vec<Option<BytecodeControlRecord>>,' \
    '.flat_map(BytecodeControlRecord::root_values),' \
    'pub(super) fn enter_control(' \
    'pub(super) fn checkout_control(' \
    'pub(super) fn park_control(' \
    'pub(super) fn finish_control('; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/continuation.rs"; then
      fail "structured control boundary changed; AS-06a2b requires one continuation-owned control stack"
    fi
  done

  for source in \
    'pub(super) enum BytecodeControlRecord {' \
    'Loop {' \
    'ForIn {' \
    'ForOf {' \
    'Switch {' \
    'Try {' \
    'record: &mut BytecodeControlRecord,' \
    'record.transient_root_values(),' \
    'pub(super) fn finish_bytecode_control_result'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/control_continuation.rs"; then
      fail "structured control boundary changed; running records require one in-place state owner and transient roots"
    fi
  done

  if ! grep -F -q 'counter.record(VmStorageKind::ExecutionFrame, continuation.control_count())?;' \
      "${repo_root}/src/runtime/accounting.rs"; then
    fail "structured control boundary changed; control records must remain charged as execution frames"
  fi
}

check_suspended_execution_boundary() {
  for source in \
    'pub(in crate::runtime) enum BytecodeOutcome {' \
    'pub(in crate::runtime) fn resume_bytecode_activation(' \
    'continuation.resume_suspension(completion)?;' \
    'self.park_bytecode_state_at(activation_index, state)?;'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/execution.rs"; then
      fail "suspended execution boundary changed; AS-06b requires one explicit park and resume outcome"
    fi
  done

  if ! grep -F -x -q '    Suspended {' \
      "${repo_root}/src/runtime/bytecode/execution.rs"; then
    fail "suspended execution boundary changed; AS-06b requires a distinct suspended outcome variant"
  fi

  for source in \
    'pub(in crate::runtime) struct SuspendedAsyncFunction {' \
    'pub(in crate::runtime) fn cancel_storage(' \
    'self.discard_execution_suffix(local_base, activation_base)?;'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/function/suspended.rs"; then
      fail "suspended execution boundary changed; detached activations must retain and release one VM-owned suffix"
    fi
  done

  for source in \
    'Await {' \
    'continuation: Box<SuspendedAsyncFunction>,' \
    'continuation.visit_direct_roots(visitor)'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/promise/job.rs"; then
      fail "suspended execution boundary changed; await reactions must own and root detached activations"
    fi
  done

  for source in \
    'Ok(Completion::Suspended(promise))' \
    'pub fn run_jobs(&mut self) -> Result<usize> {' \
    'pub fn cancel_jobs(&mut self) -> Result<usize> {' \
    'continuation.cancel_storage(&self.storage_ledger)?;'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/promise/mod.rs"; then
      fail "suspended execution boundary changed; pending await and embedder job ownership must stay explicit"
    fi
  done

  if ! grep -F -q 'self.suspended_async_execution_frame_count()?,' \
      "${repo_root}/src/runtime/accounting.rs"; then
    fail "suspended execution boundary changed; parked frames must remain in storage reconciliation"
  fi

  for source in \
    'suspend: Option<Box<BytecodeSuspendState>>,' \
    'destructure: Option<DestructureContinuation>,' \
    'pub(super) fn synchronous_root_values(&self) -> impl Iterator<Item = &Value> {'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/state.rs"; then
      fail "suspended execution boundary changed; destructuring must remain parked and rooted in bytecode state"
    fi
  done

  for source in \
    'pub(super) struct DestructureContinuation {' \
    'reference: Option<BytecodeAssignmentReference>,' \
    'Self::Array { source, .. } => source.root_values().collect(),'; do
    if ! grep -F -q "${source}" \
        "${repo_root}/src/runtime/bytecode/destructure_continuation.rs"; then
      fail "suspended execution boundary changed; destructuring tasks must preserve phase and iterator roots"
    fi
  done

  if ! grep -F -q 'state.store_destructure_continuation(continuation)?;' \
      "${repo_root}/src/runtime/bytecode/destructure.rs"; then
    fail "suspended execution boundary changed; pending pattern evaluation must not replay completed side effects"
  fi
}

check_callable_edge_boundary() {
  local edge_kinds
  local native_id_variants
  local source
  edge_kinds="$({
    awk '
      /^pub enum VmCallableEdgeKind \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/trace.rs"
  } | sed '/^[[:space:]]*\/\//d' | tr -d '[:space:]')"
  if [[ "${edge_kinds}" != 'JavaScriptFunctionUpvalue,JavaScriptFunctionProperty,JavaScriptFunctionInternal,NativeFunctionProperty,NativeFunctionInternal,BoundFunctionInternal,' ]]; then
    fail "callable edge boundary changed; categories require an assigned AS migration"
  fi

  for source in \
    'for function in &self.functions {' \
    'for function in &self.native_functions {' \
    'for function in &self.bound_functions {' \
    'for cell in self.upvalues.iter() {' \
    '.visit_strong_edges(' \
    'if let Some(binding) = &self.super_binding {' \
    'if let Some(parent) = &self.static_parent {' \
    'if let Some(fields) = &self.class_fields {' \
    'if let FunctionNewTarget::Lexical(value) = &self.new_target {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/trace.rs"; then
      fail "callable edge boundary changed; JavaScript function source '${source}' is missing"
    fi
  done

  for source in \
    'StrongEdgeReference::Value(&self.prototype)' \
    'StrongEdgeReference::Value(self.intrinsic_defaults.length.value_ref())' \
    'StrongEdgeReference::Value(self.intrinsic_defaults.name.value_ref())' \
    'if let Some(prototype) = &self.intrinsic_defaults.prototype {' \
    'if let Some(value) = self.length.stored_value() {' \
    'if let Some(value) = self.name.stored_value() {' \
    'for entry in &self.properties {' \
    'for key in &self.property_order {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/function/properties.rs"; then
      fail "callable edge boundary changed; function property source '${source}' is missing"
    fi
  done

  for source in \
    'NativeFunctionKind::BoundFunction(id)' \
    'NativeFunctionKind::CollectionIteratorNext(id)' \
    'NativeFunctionKind::PromiseCapabilityExecutor {' \
    'NativeFunctionKind::PromiseCombinatorElement { state, .. }' \
    'NativeFunctionKind::PromiseResolver { promise, .. }' \
    'NativeFunctionKind::ProxyRevoke(id)'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/native/function/mod.rs"; then
      fail "callable edge boundary changed; native payload source '${source}' is missing"
    fi
  done

  for source in \
    'StrongEdgeReference::Value(&self.target)' \
    'if let BoundFunctionBehavior::Ordinary { this_value, args } = &self.behavior {' \
    'StrongEdgeReference::Value(this_value)' \
    'for arg in args {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/call/bound.rs"; then
      fail "callable edge boundary changed; bound function source '${source}' is missing"
    fi
  done

  native_id_variants="$({
    grep -E '^[[:space:]]*[A-Za-z][A-Za-z0-9_]*\([^)]*Id\),|^[[:space:]]*[a-z_][a-z0-9_]*:[^,]*Id,' \
      "${repo_root}/src/runtime/native/function/kind.rs" || true
  } | sed 's/[[:space:]]//g')"
  compare_set "native function id-payload allowlist" "${native_id_variants}" \
    'BoundFunction(BoundFunctionId),
CollectionIteratorNext(crate::runtime::collections::CollectionIteratorId),
capability_state:ObjectId,
promise:crate::runtime::promise::PromiseId,
ProxyRevoke(ObjectId),
state:ObjectId,
state:ObjectId,'

  if ! grep -F -q 'pub fn callable_edge_snapshot(&self) -> Result<VmCallableEdgeSnapshot>' \
      "${repo_root}/src/api/embedding.rs"; then
    fail "callable edge boundary changed; Vm requires a bounded diagnostic snapshot"
  fi
}

check_object_edge_boundary() {
  local edge_kinds
  local source
  edge_kinds="$({
    awk '
      /^pub enum VmObjectEdgeKind \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/trace.rs"
  } | sed '/^[[:space:]]*\/\//d' | tr -d '[:space:]')"
  if [[ "${edge_kinds}" != 'Property,Prototype,InternalSlot,' ]]; then
    fail "object edge boundary changed; categories require an assigned AS migration"
  fi

  for source in \
    'for object in &self.objects {' \
    'for entry in &self.named_properties {' \
    'self.array_storage.visit_strong_edges(visitor)?;' \
    'if let Some(prototype) = self.prototype {' \
    'if let Some(string) = &self.string_value {' \
    'if let Some(ObjectPrimitiveValue::Symbol(symbol)) = &self.primitive_value {' \
    'if let Some(proxy) = &self.proxy_value {' \
    'if let Some(view) = &self.typed_array {' \
    'if let Some(view) = &self.data_view {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/object/trace.rs"; then
      fail "object edge boundary changed; object source '${source}' is missing"
    fi
  done

  for source in \
    'ObjectPropertyPayload::Data(descriptor)' \
    'StrongEdgeReference::Value(descriptor.value_ref())' \
    'ObjectPropertyPayload::Accessor(descriptor)' \
    'StrongEdgeReference::Value(descriptor.get_ref())' \
    'StrongEdgeReference::Value(descriptor.set_ref())'; do
    if ! grep -F -q "${source}" \
      "${repo_root}/src/runtime/object/property/descriptor.rs"; then
      fail "object edge boundary changed; descriptor source '${source}' is missing"
    fi
  done

  for source in \
    'ArrayElements::Packed(elements)' \
    'ArrayElements::Holey(elements)' \
    'for key in self.sparse_keys.values() {'; do
    if ! grep -F -q "${source}" \
      "${repo_root}/src/runtime/object/array/storage.rs"; then
      fail "object edge boundary changed; array source '${source}' is missing"
    fi
  done

  for source in \
    'StrongEdgeReference::Value(&self.target)' \
    'StrongEdgeReference::Value(&self.handler)'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/object/proxy.rs"; then
      fail "object edge boundary changed; Proxy source '${source}' is missing"
    fi
  done

  for source in \
    'if let Some(id) = self.object_prototype {' \
    'if let Some(id) = self.array_prototype {' \
    'for key in self.shapes.property_keys() {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/object/trace.rs"; then
      fail "object edge boundary changed; ObjectHeap anchor '${source}' is missing"
    fi
  done

  if ! grep -F -q 'pub fn object_edge_snapshot(&self) -> Result<VmObjectEdgeSnapshot>' \
      "${repo_root}/src/api/embedding.rs"; then
    fail "object edge boundary changed; Vm requires a bounded diagnostic snapshot"
  fi
}

check_async_edge_boundary() {
  local edge_kinds
  local source
  edge_kinds="$({
    awk '
      /^pub enum VmAsyncEdgeKind \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/async_trace.rs"
  } | sed '/^[[:space:]]*\/\//d' | tr -d '[:space:]')"
  if [[ "${edge_kinds}" != 'PromiseState,PromiseReaction,PromiseObjectAssociation,CollectionObjectAssociation,CollectionEntry,IteratorItem,WeakCollectionKey,WeakCollectionEphemeron,GeneratorObjectAssociation,GeneratorState,' ]]; then
    fail "asynchronous edge boundary changed; categories require an assigned AS migration"
  fi

  for source in \
    'pub(in crate::runtime) trait WeakEdgeVisitor<Kind>' \
    'fn visit_weak(&mut self, kind: Kind' \
    'fn visit_ephemeron(' \
    'PromiseAssociation {' \
    'CollectionAssociation {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/trace.rs"; then
      fail "asynchronous edge boundary changed; typed trace source '${source}' is missing"
    fi
  done

  for source in \
    'for (index, promise) in self.promise_object_slots.iter().enumerate() {' \
    'for promise in &self.promises {' \
    'for (index, slot) in self.collection_object_slots.iter().enumerate() {' \
    'for collection in &self.collections {' \
    'for iterator in &self.collection_iterators {' \
    'for (index, generator) in self.generator_object_slots.iter().enumerate() {' \
    'for generator in &self.generators {' \
    'Self::WeakCollectionKey => VmAsyncEdgeStrength::Weak' \
    'Self::WeakCollectionEphemeron => VmAsyncEdgeStrength::Ephemeron'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/async_trace.rs"; then
      fail "asynchronous edge boundary changed; Context source '${source}' is missing"
    fi
  done

  for source in \
    'PromiseState::Pending { reactions }' \
    'PromiseState::Fulfilled(value) | PromiseState::Rejected(value)' \
    'VmAsyncEdgeKind::PromiseState'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/promise/state.rs"; then
      fail "asynchronous edge boundary changed; Promise state source '${source}' is missing"
    fi
  done

  for source in \
    'StrongEdgeReference::Promise(*result)' \
    'if let Some(handler) = on_fulfilled {' \
    'if let Some(handler) = on_rejected {' \
    'Self::Await { continuation } => continuation.visit_strong_edges(visitor)?,'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/promise/job.rs"; then
      fail "asynchronous edge boundary changed; Promise reaction source '${source}' is missing"
    fi
  done

  for source in \
    'kind: CollectionKind,' \
    'CollectionKind::Map | CollectionKind::Set' \
    'CollectionKind::WeakMap => visitor.visit_ephemeron(' \
    'CollectionKind::WeakSet => visitor.visit_weak(' \
    'for item in &self.items {'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/collections.rs"; then
      fail "asynchronous edge boundary changed; collection source '${source}' is missing"
    fi
  done

  if ! grep -F -q 'pub fn async_edge_snapshot(&self) -> Result<VmAsyncEdgeSnapshot>' \
      "${repo_root}/src/api/embedding.rs"; then
    fail "asynchronous edge boundary changed; Vm requires a strength-classified snapshot"
  fi
}

check_gc_boundary() {
  for source in \
    'pub struct SlotArena<T> {' \
    'pub(crate) fn sweep_unmarked(&mut self, marks: &[bool]) -> Result<usize> {' \
    'self.free.push(index);'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/arena.rs"; then
      fail "garbage collection boundary changed; non-moving arena source '${source}' is missing"
    fi
  done

  for source in \
    'context.visit_direct_roots(&mut marker)?;' \
    'for (key, value) in marker.ephemerons.clone() {' \
    'pub fn collect_garbage(&mut self) -> Result<VmGarbageCollectionReport> {' \
    'self.invalidate_identity_caches();' \
    'self.release_collected_storage(&before, &after)?;' \
    '.sweep_dead_weak_entries(|key| reachability.weak_key_is_reachable(key))'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/gc.rs"; then
      fail "garbage collection boundary changed; root, ephemeron, sweep, cache, or accounting source '${source}' is missing"
    fi
  done

  for source in \
    'functions: SlotArena<Function>,' \
    'native_functions: SlotArena<native::NativeFunction>,' \
    'collections: SlotArena<collections::CollectionData>,' \
    'promises: SlotArena<Promise>,'; do
    if ! grep -F -q "${source}" "${repo_root}/src/runtime/mod.rs"; then
      fail "garbage collection boundary changed; Context store '${source}' is not reclaimable"
    fi
  done

  if ! grep -F -q 'pub(in crate::runtime) fn release_collected_storage(' \
      "${repo_root}/src/runtime/accounting.rs"; then
    fail "garbage collection boundary changed; collection must reconcile the independent owner snapshot"
  fi

  for file in src/storage/string_heap.rs src/storage/symbol.rs; do
    if ! grep -F -q 'pub(crate) fn sweep_unmarked' "${repo_root}/${file}"; then
      fail "garbage collection boundary changed; ${file} must retain explicit non-moving sweep"
    fi
  done

  if ! grep -F -q 'pub fn collect_garbage(&mut self) -> Result<VmGarbageCollectionReport>' \
      "${repo_root}/src/api/embedding.rs"; then
    fail "garbage collection boundary changed; Vm requires an explicit collection surface"
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
storage_ledger
atoms
strings
symbols
well_known_symbols
well_known_properties
iterator_symbol
descriptor_property_keys
static_name_atom_caches
static_binding_caches
static_binding_layouts
active_realm
realm
inactive_realms
locals
modules
module_evaluation_depth
dynamic_module_loader
active_module_name
activation_frames
functions
native_functions
bound_functions
host_functions
objects
collections
collection_object_slots
collection_iterators
generators
generator_object_slots
promises
promise_object_slots
promise_jobs
retained_values
transient_roots
output
output_payload_bytes
performance_clock
random_state
runtime_steps
optimizer
call_depth'
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
data_view
typed_array
is_raw_json
arguments_brand
shadow_realm
prototype
extensibility
storage_ledger'
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
    fail "VM primitive owner boundary changed; heap-admitted JsString, StringHeap, JsSymbol, and SymbolTable require identity"
  fi
  if ! grep -F -q 'if text.identity() != Some(self.identity())' \
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
  expected_control_files='src/runtime/bytecode/control/for_in.rs
src/runtime/bytecode/control/structured_do_while.rs
src/runtime/bytecode/control/structured_switch.rs
src/runtime/bytecode/control/try_catch.rs'
  compare_set "structured-control optimization owner allowlist" \
    "${control_files}" "${expected_control_files}"

  local shaped_control_markers
  shaped_control_markers="$(
    grep -R -nE \
      'LoopFastPath|loop_fast_path|BytecodeCatchFastPath|BytecodeTryFinallyFastPath|body_fast_path|try_fast_path' \
      "${repo_root}/src/runtime/bytecode/control.rs" \
      "${repo_root}/src/runtime/bytecode/control" \
      "${repo_root}/src/compiler/control.rs" \
      "${repo_root}/src/bytecode/fast_path.rs" \
      "${repo_root}/src/bytecode/types.rs" || true
  )"
  if [[ -n "${shaped_control_markers}" ]]; then
    printf '%s\n' "${shaped_control_markers}" >&2
    fail "workload-shaped control recognizer boundary changed; use reusable bytecode plans"
  fi

  linear_files="$(
    cd "${repo_root}"
    find src/runtime/bytecode/linear -maxdepth 1 -type f -name '*.rs' -printf '%p\n'
  )"
  expected_linear_files='src/runtime/bytecode/linear/direct.rs
src/runtime/bytecode/linear/in_operator.rs
src/runtime/bytecode/linear/mod.rs
src/runtime/bytecode/linear/numeric_chain.rs
src/runtime/bytecode/linear/numeric_array_reduction.rs
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

check_optimizer_boundary() {
  local direct_owners
  local expected_direct_owners
  local optimizer_fields

  direct_owners="$(
    cd "${repo_root}"
    grep -R -l -F --include='*.rs' '.optimizer' src/runtime || true
  )"
  expected_direct_owners='src/runtime/mod.rs
src/runtime/optimizer.rs'
  compare_set "optimizer state owner allowlist" "${direct_owners}" "${expected_direct_owners}"

  optimizer_fields="$({
    awk '
      /^pub\(in crate::runtime\) struct Optimizer \{/ { inside = 1; next }
      inside && /^}/ { exit }
      inside { print }
    ' "${repo_root}/src/runtime/optimizer.rs"
  } | sed -nE 's/^[[:space:]]*([a-z_][a-z0-9_]*):.*/\1/p')"
  compare_set "optimizer profiling field allowlist" "${optimizer_fields}" \
    'mode
bytecode_linear_segment_runs
bytecode_linear_direct_runs
native_call_cache_hits
native_call_cache_misses
native_call_cache_slow_paths
call_value_cache_hits
call_value_cache_misses
call_value_cache_slow_paths'

  if ! grep -F -q 'pub const fn with_optimization_mode' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'pub const fn optimization_snapshot' \
      "${repo_root}/src/api/embedding.rs" \
    || ! grep -F -q 'pub(in crate::runtime) const fn optional_optimizations_enabled' \
      "${repo_root}/src/runtime/mod.rs"; then
    fail "optimizer policy boundary changed; public VM policy, snapshot, and one Context gate are required"
  fi
}

run_checks() {
  require_file src/value/kind.rs
  require_file src/api/owned_value.rs
  require_file src/runtime/retained_values.rs
  require_file src/runtime/accounting.rs
  require_file src/runtime/activation.rs
  require_file src/runtime/arena.rs
  require_file src/runtime/bytecode/continuation.rs
  require_file src/runtime/bytecode/control_continuation.rs
  require_file src/runtime/bytecode/control/structured_switch.rs
  require_file src/runtime/bytecode/destructure_continuation.rs
  require_file src/runtime/function/suspended.rs
  require_file src/runtime/optimizer.rs
  require_file src/runtime/gc.rs
  require_file src/runtime/object/accounting.rs
  require_file src/runtime/mod.rs
  require_file src/runtime/roots.rs
  require_file src/runtime/trace.rs
  require_file src/runtime/async_trace.rs
  require_file src/runtime/transient_roots.rs
  require_file src/runtime/object/trace.rs
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
  check_owned_value_boundary
  check_retained_value_boundary
  check_storage_accounting_boundary
  check_storage_limit_boundary
  check_runtime_frontend_boundary
  check_source_metadata_boundary
  check_frontend_span_boundary
  check_bytecode_span_boundary
  check_harness_boundaries
  check_semantic_duplicate_allowlists
  check_completion_error_boundary
  check_function_accessor_boundary
  check_sequence_expression_boundary
  check_named_function_binding_boundary
  check_function_name_inference_boundary
  check_destructuring_assignment_boundary
  check_update_numeric_coercion_boundary
  check_direct_root_boundary
  check_activation_frame_boundary
  check_bytecode_continuation_boundary
  check_structured_control_boundary
  check_suspended_execution_boundary
  check_callable_edge_boundary
  check_object_edge_boundary
  check_async_edge_boundary
  check_gc_boundary
  check_state_owner_allowlists
  check_optimization_owner_allowlists
  check_optimizer_boundary
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

mutate_owned_value_variant() {
  local fixture_root="$1"
  sed -i '/    String(String),/a\    Symbol(SymbolId),' \
    "${fixture_root}/src/api/owned_value.rs"
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

mutate_dynamic_compilation_owner() {
  local fixture_root="$1"
  printf '\nfn dynamic_compilation_error(error: Error) -> Error { error }\n' \
    >>"${fixture_root}/src/runtime/native/builtins/eval.rs"
}

mutate_function_accessor_owner() {
  local fixture_root="$1"
  printf '\nfn define_function_property_key() {}\n' \
    >>"${fixture_root}/src/runtime/bytecode/class.rs"
}

mutate_sequence_expression_pop() {
  local fixture_root="$1"
  sed -i '/fn compile_sequence_expr/,/^    }/ { /self.emit(BytecodeInstruction::Pop);/d; }' \
    "${fixture_root}/src/compiler/expression.rs"
}

mutate_sequence_for_of_rhs() {
  local fixture_root="$1"
  sed -i 's/ForHeadKind::Of => self.assignment_expression(),/ForHeadKind::Of => self.expression(),/' \
    "${fixture_root}/src/parser/statement/for_statement.rs"
}

mutate_named_function_self_binding_owner() {
  local fixture_root="$1"
  sed -i 's/BindingCell::named_function/BindingCell::renamed_function/' \
    "${fixture_root}/src/runtime/function/storage.rs"
}

mutate_function_name_inference_owner() {
  local fixture_root="$1"
  printf '\nfn set_function_name() {}\n' \
    >>"${fixture_root}/src/runtime/bytecode/ops/object_literal.rs"
}

mutate_destructuring_assignment_owner() {
  local fixture_root="$1"
  printf '\nfn eval_resumable_destructure() {}\n' \
    >>"${fixture_root}/src/runtime/bytecode/class.rs"
}

mutate_update_numeric_coercion_owner() {
  local fixture_root="$1"
  printf '\nfn bytecode_update_values() {}\n' \
    >>"${fixture_root}/src/runtime/bytecode/class.rs"
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

mutate_direct_root_source() {
  local fixture_root="$1"
  sed -i '/if let Some(value) = frame.this_value() {/d' \
    "${fixture_root}/src/runtime/roots.rs"
}

mutate_activation_frame_upvalues() {
  local fixture_root="$1"
  sed -i '/        upvalues: FunctionUpvalues,/d' \
    "${fixture_root}/src/runtime/activation.rs"
}

mutate_bytecode_continuation_state() {
  local fixture_root="$1"
  sed -i '/    parked_state: Option<Box<BytecodeState>>,/d' \
    "${fixture_root}/src/runtime/bytecode/continuation.rs"
}

mutate_bytecode_continuation_unwind() {
  local fixture_root="$1"
  sed -i '/self.pop_bytecode_continuation(frame)?;/d' \
    "${fixture_root}/src/runtime/bytecode/execution.rs"
}

mutate_bytecode_continuation_clone_count() {
  local fixture_root="$1"
  sed -i '/let continuation = BytecodeContinuationFrame::block(block.clone());/a\        let _duplicate_block = block.clone();' \
    "${fixture_root}/src/runtime/bytecode/continuation.rs"
}

mutate_bytecode_function_program() {
  local fixture_root="$1"
  sed -i '/    Function(FunctionId),/d' \
    "${fixture_root}/src/runtime/bytecode/continuation.rs"
}

mutate_structured_control_owner() {
  local fixture_root="$1"
  sed -i '/    control_stack: Vec<Option<BytecodeControlRecord>>,/d' \
    "${fixture_root}/src/runtime/bytecode/continuation.rs"
}

mutate_structured_control_in_place() {
  local fixture_root="$1"
  sed -i 's/        record: &mut BytecodeControlRecord,/        record: BytecodeControlRecord,/g' \
    "${fixture_root}/src/runtime/bytecode/control_continuation.rs"
}

mutate_suspended_outcome() {
  local fixture_root="$1"
  sed -i '/^    Suspended {$/d' \
    "${fixture_root}/src/runtime/bytecode/execution.rs"
}

mutate_suspended_cancel_release() {
  local fixture_root="$1"
  sed -i '/continuation.cancel_storage(&self.storage_ledger)?;/d' \
    "${fixture_root}/src/runtime/promise/mod.rs"
}

mutate_suspended_destructure_owner() {
  local fixture_root="$1"
  sed -i '/    destructure: Option<DestructureContinuation>,/d' \
    "${fixture_root}/src/runtime/bytecode/state.rs"
}

mutate_bytecode_frame_root() {
  local fixture_root="$1"
  sed -i '/visitor.visit_value(VmRootKind::BytecodeFrame, value)?;/d' \
    "${fixture_root}/src/runtime/roots.rs"
}

mutate_native_registry_root() {
  local fixture_root="$1"
  sed -i '/for id in realm.native_function_ids() {/d' \
    "${fixture_root}/src/runtime/roots.rs"
}

mutate_transient_operand_root() {
  local fixture_root="$1"
  sed -i '/VmRootKind::TransientOperand,/d' \
    "${fixture_root}/src/runtime/bytecode/execution.rs"
}

mutate_iterator_temporary_root() {
  local fixture_root="$1"
  sed -i '/let _root_scope = self.iterator_root_scope(source)?;/d' \
    "${fixture_root}/src/runtime/abstract_operations/iterator.rs"
}

mutate_descriptor_temporary_root() {
  local fixture_root="$1"
  sed -i '/roots.add_values(get.iter())?;/d' \
    "${fixture_root}/src/runtime/native/builtins/object.rs"
}

mutate_retained_handle_root() {
  local fixture_root="$1"
  sed -i '/self.retained_values.visit(visitor)?;/d' \
    "${fixture_root}/src/runtime/roots.rs"
}

mutate_retained_slot_generation() {
  local fixture_root="$1"
  sed -i '/    slot_generation: RetainedSlotGeneration,/d' \
    "${fixture_root}/src/runtime/retained_values.rs"
}

mutate_storage_owner_source() {
  local fixture_root="$1"
  sed -i '/self.collection_iterator_item_count()?,/d' \
    "${fixture_root}/src/runtime/accounting.rs"
}

mutate_storage_payload_source() {
  local fixture_root="$1"
  sed -i '/object_counts.byte_buffer_payload_bytes(),/d' \
    "${fixture_root}/src/runtime/accounting.rs"
}

mutate_storage_payload_accumulator() {
  local fixture_root="$1"
  sed -i '/self.bytes = updated_bytes;/d' \
    "${fixture_root}/src/storage/atom.rs"
}

mutate_storage_limit_atom_payload() {
  local fixture_root="$1"
  sed -i '/Atom payload bytes exceeded {}/d' \
    "${fixture_root}/src/storage/atom.rs"
}

mutate_storage_limit_object_insertion() {
  local fixture_root="$1"
  sed -i '/self.storage_limits.max_count(VmStorageKind::ByteBuffer),/d' \
    "${fixture_root}/src/runtime/object/heap.rs"
}

mutate_storage_limit_output_release() {
  local fixture_root="$1"
  sed -i '/self.output_payload_bytes = 0;/d' \
    "${fixture_root}/src/runtime/globals.rs"
}

mutate_storage_limit_durable_reconciliation() {
  local fixture_root="$1"
  sed -i '/context.ensure_durable_storage_ledger_matches(&snapshot)?;/d' \
    "${fixture_root}/src/runtime/accounting.rs"
}

mutate_storage_limit_binding_release() {
  local fixture_root="$1"
  sed -i '/scope.deactivate_storage()?;/d' \
    "${fixture_root}/src/runtime/execution_storage.rs"
}

mutate_storage_limit_property_release() {
  local fixture_root="$1"
  sed -i '/self.release_property()?;/d' \
    "${fixture_root}/src/runtime/object/property/slot.rs"
}

mutate_storage_limit_shape_cache() {
  local fixture_root="$1"
  sed -i '/reserve_count(VmStorageKind::CacheEntry, cache_entries)?;/d' \
    "${fixture_root}/src/runtime/object/shape.rs"
}

mutate_storage_limit_full_reconciliation() {
  local fixture_root="$1"
  sed -i '/context.ensure_storage_snapshot_within_limits(&snapshot)?;/d' \
    "${fixture_root}/src/runtime/accounting.rs"
}

mutate_storage_limit_collection_release() {
  local fixture_root="$1"
  sed -i '/release_count(VmStorageKind::CollectionEntry, released)?;/d' \
    "${fixture_root}/src/runtime/collections.rs"
}

mutate_storage_limit_promise_job_growth() {
  local fixture_root="$1"
  sed -i '/grow_count(VmStorageKind::PromiseJob, reaction_count)?;/d' \
    "${fixture_root}/src/runtime/promise/mod.rs"
}

mutate_storage_limit_transient_release() {
  local fixture_root="$1"
  sed -i '/release_count_on_drop(VmStorageKind::TransientRoot, released);/d' \
    "${fixture_root}/src/runtime/transient_roots.rs"
}

mutate_storage_limit_execution_frame() {
  local fixture_root="$1"
  sed -i '/self.activation_frames.push(ActivationFrame::call(/d' \
    "${fixture_root}/src/runtime/execution_storage.rs"
}

mutate_storage_limit_association_anchor() {
  local fixture_root="$1"
  sed -i '/VmStorageKind::Association, 1/d' \
    "${fixture_root}/src/runtime/globals.rs"
}

mutate_teardown_storage_snapshot() {
  local fixture_root="$1"
  sed -i '/            storage: self.storage_snapshot()?,/d' \
    "${fixture_root}/src/api/embedding.rs"
}

mutate_bound_function_edge() {
  local fixture_root="$1"
  sed -i '/for arg in args {/d' \
    "${fixture_root}/src/runtime/call/bound.rs"
}

mutate_object_internal_edge() {
  local fixture_root="$1"
  sed -i '/if let Some(view) = &self.typed_array {/d' \
    "${fixture_root}/src/runtime/object/trace.rs"
}

mutate_object_shape_root() {
  local fixture_root="$1"
  sed -i '/for key in self.shapes.property_keys() {/d' \
    "${fixture_root}/src/runtime/object/trace.rs"
}

mutate_promise_reaction_edge() {
  local fixture_root="$1"
  sed -i '/StrongEdgeReference::Promise(\*result)/d' \
    "${fixture_root}/src/runtime/promise/job.rs"
}

mutate_weak_collection_edge() {
  local fixture_root="$1"
  sed -i '/CollectionKind::WeakMap => visitor.visit_ephemeron(/d' \
    "${fixture_root}/src/runtime/collections.rs"
}

mutate_gc_root_source() {
  local fixture_root="$1"
  sed -i '/context.visit_direct_roots(&mut marker)?;/d' \
    "${fixture_root}/src/runtime/gc.rs"
}

mutate_gc_cache_invalidation() {
  local fixture_root="$1"
  sed -i '/self.invalidate_identity_caches();/d' \
    "${fixture_root}/src/runtime/gc.rs"
}

mutate_gc_ledger_reconciliation() {
  local fixture_root="$1"
  sed -i '/self.release_collected_storage(&before, &after)?;/d' \
    "${fixture_root}/src/runtime/gc.rs"
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

mutate_control_recognizer() {
  local fixture_root="$1"
  printf '\nstruct BenchmarkLoopFastPath;\nfn compile_benchmark_loop_fast_path() {}\n' \
    >>"${fixture_root}/src/runtime/bytecode/control.rs"
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

mutate_optimizer_state_owner() {
  local fixture_root="$1"
  printf 'fn architecture_probe(context: &mut super::Context) { context.optimizer.record_native_call_cache_hit(); }\n' \
    >"${fixture_root}/src/runtime/optimizer_bypass.rs"
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
  expect_guard_failure "${temp_dir}" owned-value-representation \
    'OwnedValue boundary changed' mutate_owned_value_variant
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
  expect_guard_failure "${temp_dir}" dynamic-compilation-owner \
    'dynamic compilation error owner allowlist changed' mutate_dynamic_compilation_owner
  expect_guard_failure "${temp_dir}" function-accessor-owner \
    'function accessor owner allowlist changed' mutate_function_accessor_owner
  expect_guard_failure "${temp_dir}" sequence-expression-pop \
    'sequence expression boundary changed' mutate_sequence_expression_pop
  expect_guard_failure "${temp_dir}" sequence-for-of-rhs \
    'sequence expression boundary changed' mutate_sequence_for_of_rhs
  expect_guard_failure "${temp_dir}" named-function-self-binding-owner \
    'named function binding boundary changed' mutate_named_function_self_binding_owner
  expect_guard_failure "${temp_dir}" function-name-inference-owner \
    'function name runtime owner allowlist changed' mutate_function_name_inference_owner
  expect_guard_failure "${temp_dir}" destructuring-assignment-owner \
    'destructuring runtime owner allowlist changed' mutate_destructuring_assignment_owner
  expect_guard_failure "${temp_dir}" update-numeric-coercion-owner \
    'update numeric coercion owner allowlist changed' mutate_update_numeric_coercion_owner
  expect_guard_failure "${temp_dir}" host-local-value-identity \
    'host local-value boundary changed' mutate_host_local_value_identity
  expect_guard_failure "${temp_dir}" javascript-exception-visibility \
    'JavaScriptException fields must stay private' mutate_javascript_exception_visibility
  expect_guard_failure "${temp_dir}" direct-root-source \
    'direct root boundary changed' mutate_direct_root_source
  expect_guard_failure "${temp_dir}" activation-frame-upvalues \
    'activation frame boundary changed' mutate_activation_frame_upvalues
  expect_guard_failure "${temp_dir}" bytecode-continuation-state \
    'bytecode continuation boundary changed' mutate_bytecode_continuation_state
  expect_guard_failure "${temp_dir}" bytecode-continuation-unwind \
    'bytecode continuation boundary changed' mutate_bytecode_continuation_unwind
  expect_guard_failure "${temp_dir}" bytecode-continuation-clone-count \
    'bytecode continuation boundary changed' mutate_bytecode_continuation_clone_count
  expect_guard_failure "${temp_dir}" bytecode-function-program \
    'bytecode continuation boundary changed' mutate_bytecode_function_program
  expect_guard_failure "${temp_dir}" structured-control-owner \
    'structured control boundary changed' mutate_structured_control_owner
  expect_guard_failure "${temp_dir}" structured-control-in-place \
    'structured control boundary changed' mutate_structured_control_in_place
  expect_guard_failure "${temp_dir}" suspended-outcome \
    'suspended execution boundary changed' mutate_suspended_outcome
  expect_guard_failure "${temp_dir}" suspended-cancel-release \
    'suspended execution boundary changed' mutate_suspended_cancel_release
  expect_guard_failure "${temp_dir}" suspended-destructure-owner \
    'suspended execution boundary changed' mutate_suspended_destructure_owner
  expect_guard_failure "${temp_dir}" bytecode-frame-root \
    'direct root boundary changed' mutate_bytecode_frame_root
  expect_guard_failure "${temp_dir}" native-registry-root \
    'direct root boundary changed' mutate_native_registry_root
  expect_guard_failure "${temp_dir}" transient-operand-root \
    'direct root boundary changed' mutate_transient_operand_root
  expect_guard_failure "${temp_dir}" iterator-temporary-root \
    'direct root boundary changed' mutate_iterator_temporary_root
  expect_guard_failure "${temp_dir}" descriptor-temporary-root \
    'direct root boundary changed' mutate_descriptor_temporary_root
  expect_guard_failure "${temp_dir}" retained-handle-root \
    'direct root boundary changed' mutate_retained_handle_root
  expect_guard_failure "${temp_dir}" retained-slot-generation \
    'retained value boundary changed' mutate_retained_slot_generation
  expect_guard_failure "${temp_dir}" storage-owner-source \
    'storage accounting boundary changed' mutate_storage_owner_source
  expect_guard_failure "${temp_dir}" storage-payload-source \
    'storage accounting boundary changed' mutate_storage_payload_source
  expect_guard_failure "${temp_dir}" storage-payload-accumulator \
    'storage accounting boundary changed' mutate_storage_payload_accumulator
  expect_guard_failure "${temp_dir}" storage-limit-atom-payload \
    'storage limit boundary changed' mutate_storage_limit_atom_payload
  expect_guard_failure "${temp_dir}" storage-limit-object-insertion \
    'storage limit boundary changed' mutate_storage_limit_object_insertion
  expect_guard_failure "${temp_dir}" storage-limit-output-release \
    'storage limit boundary changed' mutate_storage_limit_output_release
  expect_guard_failure "${temp_dir}" storage-limit-durable-reconciliation \
    'storage limit boundary changed' mutate_storage_limit_durable_reconciliation
  expect_guard_failure "${temp_dir}" storage-limit-binding-release \
    'storage limit boundary changed' mutate_storage_limit_binding_release
  expect_guard_failure "${temp_dir}" storage-limit-property-release \
    'storage limit boundary changed' mutate_storage_limit_property_release
  expect_guard_failure "${temp_dir}" storage-limit-shape-cache \
    'storage limit boundary changed' mutate_storage_limit_shape_cache
  expect_guard_failure "${temp_dir}" storage-limit-full-reconciliation \
    'storage limit boundary changed' mutate_storage_limit_full_reconciliation
  expect_guard_failure "${temp_dir}" storage-limit-collection-release \
    'storage limit boundary changed' mutate_storage_limit_collection_release
  expect_guard_failure "${temp_dir}" storage-limit-promise-job-growth \
    'storage limit boundary changed' mutate_storage_limit_promise_job_growth
  expect_guard_failure "${temp_dir}" storage-limit-transient-release \
    'storage limit boundary changed' mutate_storage_limit_transient_release
  expect_guard_failure "${temp_dir}" storage-limit-execution-frame \
    'storage limit boundary changed' mutate_storage_limit_execution_frame
  expect_guard_failure "${temp_dir}" storage-limit-association-anchor \
    'storage limit boundary changed' mutate_storage_limit_association_anchor
  expect_guard_failure "${temp_dir}" teardown-storage-snapshot \
    'storage accounting boundary changed' mutate_teardown_storage_snapshot
  expect_guard_failure "${temp_dir}" bound-function-edge \
    'callable edge boundary changed' mutate_bound_function_edge
  expect_guard_failure "${temp_dir}" object-internal-edge \
    'object edge boundary changed' mutate_object_internal_edge
  expect_guard_failure "${temp_dir}" object-shape-root \
    'object edge boundary changed' mutate_object_shape_root
  expect_guard_failure "${temp_dir}" promise-reaction-edge \
    'asynchronous edge boundary changed' mutate_promise_reaction_edge
  expect_guard_failure "${temp_dir}" weak-collection-edge \
    'asynchronous edge boundary changed' mutate_weak_collection_edge
  expect_guard_failure "${temp_dir}" gc-root-source \
    'garbage collection boundary changed' mutate_gc_root_source
  expect_guard_failure "${temp_dir}" gc-cache-invalidation \
    'garbage collection boundary changed' mutate_gc_cache_invalidation
  expect_guard_failure "${temp_dir}" gc-ledger-reconciliation \
    'garbage collection boundary changed' mutate_gc_ledger_reconciliation
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
  expect_guard_failure "${temp_dir}" control-recognizer \
    'workload-shaped control recognizer boundary changed' mutate_control_recognizer
  expect_guard_failure "${temp_dir}" linear-owner \
    'linear optimization owner allowlist changed' mutate_linear_owner
  expect_guard_failure "${temp_dir}" fast-path-owner \
    'fast-path owner allowlist changed' mutate_fast_path_owner
  expect_guard_failure "${temp_dir}" optimizer-state-owner \
    'optimizer state owner allowlist changed' mutate_optimizer_state_owner
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
