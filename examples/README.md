# Embedding Examples

These examples form a progressive, directly runnable guide to embedding Velum
in a Rust application. Fifteen examples use Tokio. One intentionally keeps the
executor-neutral manual API visible so applications with another runtime can
see the underlying scheduling contract.

The examples live in a dedicated package so Tokio and application-oriented
dependencies do not enter the dependency-light `velum` engine crate.
JavaScript source is compiled and executed normally; Rust-to-JavaScript calls,
construction, properties, callbacks, async completion, and host classes use
typed APIs rather than generated source or an eval bridge.

Run the smallest example from the repository root:

```sh
cargo run -p velum-embedding-examples --example 00_hello_world
```

## Learning Path

| Example | Runtime style | Main concept |
| --- | --- | --- |
| [`00_hello_world`](00_hello_world/main.rs) | Tokio-owned VM | Create a VM, execute JavaScript, and collect `print` output. |
| [`01_basic_eval`](01_basic_eval/main.rs) | Tokio-owned VM | Copy a primitive into `OwnedValue`, preserve source names, and inspect structured JavaScript errors. |
| [`02_typed_host_functions`](02_typed_host_functions/main.rs) | Tokio-owned VM | Register synchronous Rust functions with checked arguments, typed returns, and captured thread-safe application state. |
| [`03_async_rust_javascript_roundtrip`](03_async_rust_javascript_roundtrip/main.rs) | Tokio-owned VM | Drive a Rust-to-JavaScript-to-Rust-to-JavaScript Promise round trip through VM-local async work. |
| [`04_callbacks_and_intervals`](04_callbacks_and_intervals/main.rs) | Tokio-owned VM | Pass a first-class Rust callable into a JavaScript class, retain it there, and invoke it ten times from a JavaScript interval policy backed by Tokio sleep. |
| [`05_rust_backed_websocket`](05_rust_backed_websocket/main.rs) | Tokio-owned VM and macros | Define an async mock `WebSocket` over hidden Rust state with `#[velum::host_class]` and `#[velum::host_methods]`. |
| [`06_javascript_class_from_rust`](06_javascript_class_from_rust/main.rs) | Tokio-owned VM | Construct a JavaScript class, call sync and async methods, mutate and define fields, and replace a method with Rust. |
| [`07_values_and_handles`](07_values_and_handles/main.rs) | Tokio current-thread | Distinguish portable, callback-local, and explicitly retained values; observe cross-VM rejection and release. |
| [`08_compile_once_run_many`](08_compile_once_run_many/main.rs) | Tokio current-thread | Reuse one immutable VM-local `CompiledScript` within and across isolated VMs on one owner thread. |
| [`09_sandboxed_execution`](09_sandboxed_execution/main.rs) | Tokio current-thread | Supply a deterministic clock and enforce runtime, stack, string, buffer, object, property, output, and storage limits. |
| [`10_custom_module_loader`](10_custom_module_loader/main.rs) | Tokio-owned VM | Own canonical module identities, policy, attributes, cycles, dynamic import, and `import.meta`. |
| [`11_promises_and_jobs`](11_promises_and_jobs/main.rs) | Manual, no Tokio | Register a host class and async function through the low-level builders, then poll Rust futures and drain or cancel Promise jobs explicitly. |
| [`12_realms_and_plugins`](12_realms_and_plugins/main.rs) | Tokio current-thread | Isolate plugin globals and intrinsics in realms while rejecting foreign realm identities across VMs. |
| [`13_shared_memory_between_vms`](13_shared_memory_between_vms/main.rs) | Two Tokio-owned VMs | Install one `SharedArrayBufferHandle` in independent VM workers and coordinate through `Atomics`. |
| [`14_observability_and_teardown`](14_observability_and_teardown/main.rs) | Tokio current-thread | Inspect build identity, roots, storage, reachability, optimizations, explicit GC, and consuming deterministic teardown. |
| [`15_host_type_multiple_vms`](15_host_type_multiple_vms/main.rs) | Three Tokio-owned VMs and macros | Register one macro-defined Rust type in three VMs and create one independent hidden-state instance in each VM. |

Run the entire suite:

```sh
for example in \
  00_hello_world \
  01_basic_eval \
  02_typed_host_functions \
  03_async_rust_javascript_roundtrip \
  04_callbacks_and_intervals \
  05_rust_backed_websocket \
  06_javascript_class_from_rust \
  07_values_and_handles \
  08_compile_once_run_many \
  09_sandboxed_execution \
  10_custom_module_loader \
  11_promises_and_jobs \
  12_realms_and_plugins \
  13_shared_memory_between_vms \
  14_observability_and_teardown \
  15_host_type_multiple_vms
do
  cargo run -p velum-embedding-examples --example "${example}"
done
```

## Tokio And VM Ownership

`velum-tokio` assigns each VM permanently to one Tokio current-thread worker.
Commands for one VM stay serialized on its owner, while VM-local host futures,
queued Rust-to-JavaScript commands, and Promise jobs are advanced
automatically. Independent VMs can be distributed over several such workers
and make progress in parallel.

Examples 07, 08, 09, 12, and 14 deliberately create VMs directly inside a
Tokio current-thread application. They teach APIs whose values remain local to
that thread, including `RetainedValue`, `CompiledScript`, custom clock state,
`RealmId`, and the consuming `Vm::finish` operation.

Example 11 is the sole no-Tokio reference. It exposes the executor-neutral
contract below the adapter: the application polls host futures, runs host
commands, and drains or cancels JavaScript jobs itself. This is useful when an
embedder supplies a different executor; it is not the default learning path.

The engine itself does not depend on Tokio. Choosing worker count is a runtime
configuration decision and never changes a macro-defined host type.

## Host Types And Hidden State

Examples 05 and 15 mark only the Rust fields and methods intended for
JavaScript. Unannotated fields remain private Rust implementation state and are
still available to exported methods. Natural Rust `async fn` methods become
JavaScript Promise-returning methods through the same VM-owned host-future
path.

Example 15 registers the same Rust type definition in three independent VMs.
Each JavaScript constructor call creates a separate Rust payload; nothing is
shared implicitly. Explicit application resources can still be shared by
putting a synchronization-aware handle such as `Arc<Mutex<_>>` inside the
payload, but that is an embedder ownership choice rather than a macro mode.

## Values And Scheduling Boundaries

`OwnedValue` represents VM-independent primitives, but its full enum is not a
cross-thread transport type. Convert a result to the exact `Send` application
type needed by the caller while still inside `VmHandle::run` or
`VmHandle::run_local`. `RetainedValue` is an explicit VM-bound root and never
crosses the worker command boundary.

The interval, module loader, clock, shared memory, and mock transport in this
directory are application policies, not unconditional browser or Node.js
globals added to the engine. The underlying JavaScript work still follows the
ordinary compiler, bytecode, call, property, Promise, and job paths.

The design and acceptance requirements behind the suite remain documented in
[`TODO.md`](TODO.md).
