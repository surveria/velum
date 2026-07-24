# Embedding Examples

These examples form a progressive, directly runnable guide to embedding Velum
in a Rust application. Every program uses the public `Engine` and `Vm` facade.
JavaScript source is evaluated or compiled normally; Rust-to-JavaScript calls,
construction, properties, callbacks, async completion, and host classes use
typed APIs rather than generated source or an eval bridge.

Run the smallest example from the repository root:

```sh
cargo run --example 00_hello_world
```

## Learning Path

| Example | Main concept |
| --- | --- |
| [`00_hello_world`](00_hello_world/main.rs) | Create a VM, execute JavaScript, and collect `print` output. |
| [`01_basic_eval`](01_basic_eval/main.rs) | Copy a primitive into `OwnedValue`, preserve source names, and inspect structured JavaScript errors. |
| [`02_typed_host_functions`](02_typed_host_functions/main.rs) | Register synchronous Rust functions with checked arguments, typed returns, and captured application state. |
| [`03_async_rust_javascript_roundtrip`](03_async_rust_javascript_roundtrip/main.rs) | Drive a Rust-to-JavaScript-to-Rust-to-JavaScript Promise round trip without borrowing the VM across `await`. |
| [`04_callbacks_and_intervals`](04_callbacks_and_intervals/main.rs) | Pass a first-class Rust callable into a JavaScript class and invoke it ten times with an application-owned virtual interval. |
| [`05_rust_backed_websocket`](05_rust_backed_websocket/main.rs) | Register a constructable mock `WebSocket` backed by bounded Rust state, ordinary descriptors, and explicit shared wrappers. |
| [`06_javascript_class_from_rust`](06_javascript_class_from_rust/main.rs) | Construct a JavaScript class, call sync and async methods, mutate and define fields, and replace a method with Rust. |
| [`07_values_and_handles`](07_values_and_handles/main.rs) | Distinguish portable, callback-local, and explicitly retained values; observe cross-VM rejection and release. |
| [`08_compile_once_run_many`](08_compile_once_run_many/main.rs) | Reuse one immutable `CompiledScript` within and across isolated VMs. |
| [`09_sandboxed_execution`](09_sandboxed_execution/main.rs) | Supply a deterministic clock and enforce runtime, stack, string, buffer, object, property, output, and storage limits. |
| [`10_custom_module_loader`](10_custom_module_loader/main.rs) | Own canonical module identities, policy, attributes, cycles, dynamic import, and `import.meta`. |
| [`11_promises_and_jobs`](11_promises_and_jobs/main.rs) | Poll Rust futures separately from draining or cancelling VM-owned Promise jobs. |
| [`12_realms_and_plugins`](12_realms_and_plugins/main.rs) | Isolate plugin globals and intrinsics in realms while rejecting foreign realm identities across VMs. |
| [`13_shared_memory_between_vms`](13_shared_memory_between_vms/main.rs) | Install one `SharedArrayBufferHandle` in independent VMs and coordinate through `Atomics`. |
| [`14_observability_and_teardown`](14_observability_and_teardown/main.rs) | Inspect build identity, roots, storage, reachability, optimizations, explicit GC, and deterministic teardown. |

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
  14_observability_and_teardown
do
  cargo run --example "${example}"
done
```

## Tokio-Owned VMs And Host-Class Macros

The optional `velum-tokio` workspace crate adds a runtime adapter without
adding Tokio to the engine itself. Its companion example defines an
asynchronous Rust-backed `WebSocket` with `#[velum::host_class]` and
`#[velum::host_methods]`, keeps its transport token and message storage hidden
from JavaScript, and runs the VM on a permanent single-owner worker:

```sh
cargo run -p velum-tokio --example tokio_host_class
```

The adapter distributes independent VMs across a configurable number of Tokio
current-thread workers. Commands for one VM stay serialized on its owner,
while host futures, queued Rust-to-JavaScript commands, and Promise jobs are
driven automatically. Results crossing the worker boundary must be `Send`;
VM-local handles remain inside the command closure.

## Ownership And Scheduling Model

`OwnedValue` contains VM-independent primitives. `RetainedValue` is an
explicit VM-bound root and rejects use by another VM. Values borrowed through
`HostCall` exist only for that callback unless they are copied or retained.

The VM owns JavaScript Promise state and its job queue. The application owns
the outer executor and explicitly interleaves `poll_host_futures`,
`run_host_commands`, and `run_jobs`. The interval, module loader, clock, and
mock transport in this directory are application policies, not unconditional
browser or Node.js globals added to the engine.

Applications using `velum-tokio` delegate that same explicit pump to the
adapter. The underlying engine contract and bytecode-owned execution path do
not change.

The design and acceptance requirements behind the suite remain documented in
[`TODO.md`](TODO.md).
