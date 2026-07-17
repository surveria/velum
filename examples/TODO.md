# Embedding Examples And Capability Roadmap

This document defines the target embedding examples for Velum and the engine
capabilities required to implement them as real application-facing APIs.

The examples that can already be written with the current public API are
intentionally deferred. The immediate priority is the missing embedding
surface needed by the bidirectional async, callback, host-class, and
Rust-driven object examples. Once that surface is complete, the full example
suite should be implemented against the same stable API.

## Hard Requirements

The examples are product acceptance scenarios, not isolated demonstrations.
Their implementation must follow these rules:

- Calls, construction, property access, callbacks, and async completion must
  enter the existing VM semantic and bytecode-owned execution paths.
- Rust-to-JavaScript operations must not be implemented by generating source
  strings or routing through `eval`.
- There must be no second interpreter, host-only object semantics, or parallel
  call path that disagrees with ordinary JavaScript execution.
- Public handles must be VM-identity and generation checked. A value, object,
  callback, Promise, realm, or constructor from one VM must not enter another
  VM accidentally.
- Durable JavaScript values must be explicit roots with deterministic release
  behavior. Raw `Value` identifiers must not become public durable handles.
- A Rust future must never hold a mutable VM borrow across an await point.
- The VM owns JavaScript jobs and Promise state. The embedding application owns
  the outer Rust executor, I/O policy, clocks, timers, and network services.
- Every new owner must participate in resource limits, garbage collection,
  cancellation, snapshots, deterministic teardown, and VM isolation.
- The root engine crate must not depend on a particular async runtime.
- The implementation must remain safe Rust and preserve the repository's
  no-panic and checked-access rules.

## Example Catalog

No example is implemented yet. `Deferred` means the current public API is
sufficient or nearly sufficient, `Ready to implement` means its blocking
capability tranche has landed, and `Capability work` means a first-class
engine API must still land before the example can be written honestly.

| ID | Planned directory | Status | Purpose |
| --- | --- | --- | --- |
| 00 | `00_hello_world` | Deferred | Create an `Engine` and `Vm`, evaluate `print("Hello, world!")`, and collect the output. |
| 01 | `01_basic_eval` | Deferred | Evaluate named source, return an `OwnedValue`, capture output, and classify errors. |
| 02 | `02_typed_host_functions` | Deferred | Register synchronous typed Rust functions with checked arguments, return conversion, captured application state, and contextual errors. |
| 03 | `03_async_rust_javascript_roundtrip` | Ready to implement | Demonstrate an awaited Rust-to-JavaScript-to-Rust-to-JavaScript round trip without reentrant mutable VM access. |
| 04 | `04_callbacks_and_intervals` | Capability work | Pass a Rust callable into JavaScript, retain it on a JavaScript object, and invoke it ten times through an embedder-provided interval scheduler. |
| 05 | `05_rust_backed_websocket` | Capability work | Expose a mock JavaScript `WebSocket` class whose instances own typed Rust state and methods. |
| 06 | `06_javascript_class_from_rust` | Capability work | Construct a JavaScript class from Rust, call methods, await a method result, inspect and mutate fields, add a property, and replace a method. |
| 07 | `07_values_and_handles` | Deferred | Explain portable `OwnedValue`, VM-bound `RetainedValue`, callback-local values, explicit release, GC rooting, and cross-VM rejection. |
| 08 | `08_compile_once_run_many` | Deferred | Compile once, execute repeatedly in one VM, and share immutable compiled code across isolated VMs. |
| 09 | `09_sandboxed_execution` | Deferred | Configure source, stack, runtime-step, storage, object, string, and buffer limits with a deterministic clock and structured failures. |
| 10 | `10_custom_module_loader` | Deferred | Implement application-owned module resolution, canonical identities, import policy, attributes, cycles, dynamic import, and `import.meta`. |
| 11 | `11_promises_and_jobs` | Deferred | Integrate JavaScript Promise jobs with an application-controlled outer loop, including draining and cancellation. |
| 12 | `12_realms_and_plugins` | Deferred | Compare realms and independent VMs, showing isolated globals and intrinsics versus shared VM ownership. |
| 13 | `13_shared_memory_between_vms` | Deferred | Transfer a `SharedArrayBufferHandle` between independent VMs and coordinate through `Atomics`. |
| 14 | `14_observability_and_teardown` | Deferred | Inspect usage, storage, roots, heap reachability, optimization counters, explicit GC, build identity, and deterministic teardown. |

## Blocked Example Specifications

### 03: Async Rust And JavaScript Round Trip

The example should execute this awaited sequence:

```text
Rust calls jsEntry()
  -> jsEntry() awaits rustRoundTrip(jsLog, "hello")
    -> rustRoundTrip performs asynchronous Rust work
    -> Rust schedules and awaits jsLog(result)
      -> jsLog prints from JavaScript and returns a value
    <- the JavaScript callback result resumes the Rust future
  <- the Rust result resolves the Promise awaited by jsEntry()
<- Rust awaits the final JavaScript result
```

The Rust host future must not call into a mutably borrowed VM directly. The
JavaScript callback invocation must be represented as VM-owned queued work,
and progress must interleave polling the application executor with draining VM
jobs. Fulfilment, rejection, cancellation, and teardown must settle or release
every owner exactly once.

Required capabilities:

- async typed host-function registration;
- a durable callable handle accepted as a host-function argument;
- Rust-to-JavaScript async call and result conversion;
- a Promise capability owned by the VM for each pending host future;
- a safe command or wakeup path for a Rust future to schedule a JavaScript
  call without borrowing the VM across `.await`;
- rejection propagation in both directions;
- explicit cancellation and pending-work accounting.

### 04: Stored Rust Callback And Intervals

Rust should create a real callable value and pass it as an argument to a
JavaScript method. JavaScript stores that value on a class instance and invokes
it ten times from an interval callback. The example should use a deterministic
virtual clock so it has no sleeps or timing flakes.

`setInterval` and `clearInterval` are host capabilities in this example, not
ECMAScript built-ins and not permanent browser APIs in the engine. The timer
scheduler retains scheduled callbacks explicitly, enqueues due calls into the
VM, and releases them on cancellation or teardown.

Required capabilities:

- create a VM-local JavaScript-callable wrapper around a Rust callback without
  installing it only as a global binding;
- pass owned primitives and retained VM values together in one call argument
  list;
- retain a callback through ordinary JavaScript object properties and GC;
- enqueue a call from an external application event;
- preserve `this`, argument order, exception propagation, and FIFO job order;
- cancel timers and release callback roots deterministically.

### 05: Rust-Backed Mock WebSocket Class

The example should register a JavaScript `WebSocket` constructor backed by a
clearly documented mock Rust transport. It performs no real network I/O.

Each JavaScript wrapper owns a typed handle to Rust state containing the URL,
ready state, buffered amount, and sent-message history. `send` and `close`
operate on that state. A non-standard `cloneHandle` method creates a distinct
JavaScript wrapper that explicitly shares the same synchronized Rust resource.

Required capabilities:

- register an embedder-defined constructable class;
- attach a typed opaque Rust payload to each instance;
- define prototype methods and instance accessors through ordinary property
  descriptors;
- obtain the checked receiver payload in a host method;
- create a second wrapper around explicitly shared Rust state;
- trace JavaScript values retained by a host instance, when such retention is
  requested;
- run Rust payload destruction exactly once when the last owner is released;
- account host instances, payloads, callbacks, and retained edges in limits,
  snapshots, GC, and teardown.

Host payloads must not add a generic unaccounted escape hatch. Thread-safe
sharing must be explicit in the payload type; VM-local state must remain local.

### 06: JavaScript Class Controlled From Rust

JavaScript should declare a `Device` class with fields, a synchronous method,
and an async method. Rust then retrieves the constructor, creates an instance,
calls both methods with the correct receiver, awaits the async result, reads
and changes a field, defines a new field, and replaces one method with a real
callable value.

Required capabilities:

- retain a global class constructor;
- call and construct arbitrary callable values through the shared semantic
  call and construct owners;
- call a named method while preserving observable `this` behavior;
- get, set, define, and delete properties through the same Proxy-, accessor-,
  descriptor-, prototype-, and strictness-aware operations used by bytecode;
- select raw, owned, or retained result boundaries explicitly;
- await a returned Promise without inventing a second async execution path;
- expose the call receiver to Rust callbacks when a Rust callable replaces a
  JavaScript method.

No operation in this example may synthesize JavaScript source such as
`eval("device.count = 5")` or serialize an object name into a script.

## Capability Work Plan

The capability work should land as reviewable tranches. Exact public type names
remain design decisions, but the ownership and semantic boundaries below are
required.

### Tranche 1: Public Invocation Values And Arguments

Status: implemented by `JsValueRef`, `PropertyKeyRef`, and the existing
`OwnedValue`/`RetainedValue` ownership boundaries. Every retained input is
resolved against the target VM before JavaScript dispatch.

Define a public argument representation that can borrow either a portable
primitive or a retained VM value. Define explicit owned and retained result
variants rather than returning raw durable `Value` identifiers.

The design must cover:

- VM identity and generation validation before dispatch;
- checked conversion of Rust primitives and exact UTF-16 strings;
- callback-local conversion and retention;
- non-cloneable roots or another equally explicit release model;
- source names and contextual error chains for host-initiated calls;
- storage accounting for every retained argument and result owner.

### Tranche 2: Rust-To-JavaScript Object, Call, And Construct API

Status: implemented on `Vm` with raw, owned, and retained call/property result
boundaries plus call, explicit-receiver call, method call, construction,
property get/set/define/delete, descriptor inspection, and callable/constructor
checks. The public facade delegates to the existing semantic owners and does
not synthesize source or use eval.

Expose first-class public operations for:

- call with an explicit receiver;
- method lookup followed by call with the original receiver;
- construction with ordinary `newTarget` semantics;
- property get, set, define, delete, and own-descriptor inspection;
- callable and constructable checks;
- owned and retained result selection.

These APIs must be thin public boundaries over the existing semantic object,
property, call, and construct owners. They must observe Proxies, accessors,
descriptors, prototype traversal, exceptions, and allocation safepoints exactly
as equivalent JavaScript bytecode would.

This tranche unlocks the synchronous portion of example 06 and provides the
foundation for every later callback and class API.

### Tranche 3: First-Class Rust Callable Values

Status: partially implemented. `Vm::create_host_function` and
`Vm::create_host_function_typed` now create retained Rust-backed callables
without global bindings. They can be passed into JavaScript, stored in ordinary
properties, invoked later, traced through GC, and collected with their Rust
captures after the final JavaScript edge disappears. Exposing the observable
call receiver through `HostCall` remains required before this tranche is
complete.

Separate creation of a Rust-backed callable value from installation of a named
global binding. A callable must be passable as an argument, storable in an
object, retrievable as a retained handle, and callable with an observable
receiver.

The callable registry remains VM-local. Callback captures are embedder-owned,
but their registry entries, JavaScript edges, invocations, errors, limits, and
release state must be visible to VM accounting. Cross-VM calls must fail before
execution.

This tranche unlocks the stored-callback part of example 04 and method
replacement in example 06.

### Tranche 4: Async Host Functions And Promise Bridge

Status: implemented for the bidirectional host-task round trip. Global and
first-class async Rust callables return ordinary JavaScript Promises. Their
`'static` futures produce portable `OwnedValue` results, are polled with an
embedder-supplied standard `Waker`, settle through the existing Promise owner,
enqueue reactions in the ordinary FIFO job queue, and have explicit pending
counts, storage limits, roots, and cancellation. Command-aware tasks receive a
VM-bound `HostAsyncContext`, enqueue a retained JavaScript callable plus owned
arguments without borrowing the VM, and await `HostCommandRequest`. Explicit
`run_host_commands`, `run_jobs`, and `poll_host_futures` phases preserve owner
boundaries. Fulfilments return owned primitives; arbitrary JavaScript
rejections cross the Rust future boundary as rooted `HostFutureError` values
and retain exact identity.

Add runtime-agnostic async host functions whose invocation immediately creates
and returns a JavaScript Promise. The application executor polls the Rust
future; completion is posted back to the owning VM and resolves or rejects the
Promise through the normal job queue.

Remaining adjacent work is a general embedder-side Promise result handle for
awaiting a Promise returned by a direct Rust-to-JavaScript call outside an
active async host task. That is still required for the fully Rust-driven async
method portion of example 06, but it no longer blocks example 03.

The design must specify:

- how callback arguments become owned primitives or durable VM roots before
  the synchronous host-call frame ends;
- how the VM wakes the outer application and how the application wakes the VM;
- how a Rust future requests and awaits a JavaScript callback invocation;
- ordering between host completions, Promise reactions, module jobs, and other
  ready jobs;
- cancellation when the application, VM, realm, or job owner shuts down;
- behavior when a future completes after cancellation;
- error conversion without losing Rust context or JavaScript thrown identity;
- limits for pending host futures, callback calls, queued completions, runtime
  steps, allocations, output, and wall-clock policy.

This tranche unlocks example 03 and the async portion of example 06.

### Tranche 5: External Event And Timer Scheduling

Expose a controlled way for the embedding application to enqueue a call to a
retained callable. Build the example-only interval service on that primitive.

The engine should not grow browser or Node.js timers as unconditional globals.
The example installs `setInterval` and `clearInterval` explicitly, owns the
clock and schedule in Rust, advances virtual time deterministically, and posts
due callbacks into the VM queue.

The tranche must define queue backpressure, cancellation races, callback
release, callback exceptions, VM shutdown, and per-timer resource accounting.

This tranche completes example 04 and establishes the event-source pattern for
network, filesystem, GUI, and application callbacks.

### Tranche 6: Typed Host Objects And Classes

Add a VM-local typed host-payload registry plus a class registration surface.
The JavaScript wrapper remains an ordinary object with standard descriptors,
prototype behavior, callability rules, and Proxy observability. Only the opaque
payload and its checked Rust access are host-defined.

The design must specify:

- constructor, method, getter, setter, and static-member registration;
- checked `this` extraction and subclass behavior;
- payload ownership, optional explicit synchronized sharing, and destruction;
- tracing of any JavaScript handles retained by a payload;
- prevention of VM-to-VM payload confusion;
- interaction with GC cycles, weak edges, finalization, snapshots, and forced
  VM teardown;
- host-instance and payload limits;
- behavior when a constructor or method returns an error or Promise.

This tranche unlocks example 05. The mock WebSocket should be the acceptance
case, not a special runtime object kind built only for that class.

### Tranche 7: Public Facade And Full Example Suite

Review the completed surface through `Engine` and `Vm`. Example code should not
drop into internal modules, depend on raw arena identifiers, or use `Context`
only because a useful operation was omitted from the primary facade.

Then implement examples 00 through 14, add an `examples/README.md` learning
path, and make every example directly runnable. Shared helpers should remain
small and should not hide the public API being taught.

## Verification Strategy

Every capability tranche needs direct public API tests before its example is
considered complete.

### Semantic Tests

- Calls, method calls, and construction match equivalent JavaScript execution.
- Property APIs observe accessors, inherited setters, non-writable properties,
  strict failures, Proxies, descriptors, symbols, and prototype mutation.
- Host functions receive the correct `this`, arguments, realm, and VM identity.
- JavaScript exceptions and Rust errors preserve structured context in both
  directions.

### Ownership And Isolation Tests

- Every public handle rejects foreign and stale VM identities.
- Stored callbacks and returned objects survive GC while rooted.
- Released handles, cancelled timers, and completed async calls stop retaining
  their values.
- Independent VMs can run the same compiled code without sharing globals,
  callbacks, jobs, host objects, errors, or teardown state.
- Explicit shared host payloads share only the resource selected by the
  embedder.

### Async And Cancellation Tests

- Fulfilment and rejection cross Rust and JavaScript boundaries exactly once.
- Nested Rust-to-JavaScript-to-Rust calls make progress without reentrant VM
  borrows or deadlock.
- Job ordering remains deterministic across Promise reactions, host future
  completions, module jobs, and externally enqueued callbacks.
- Cancellation works before polling, while pending, during a queued callback,
  and after completion has raced with shutdown.
- VM teardown with pending futures, timers, and callbacks releases all roots
  and records a truthful report.

### Resource And Architecture Tests

- Limits cover retained handles, host callables, pending futures, queued
  completions, external events, timers, host instances, and payload bytes.
- Architecture boundary checks prove public embedding APIs delegate to shared
  semantic property, call, construct, Promise, and job owners.
- The implementation contains no source-generation or `eval` bridge for the
  new APIs.
- Direct library benchmarks measure sync call, property access, callback,
  async completion, and host-object overhead without making benchmarks a
  separate semantic path.
- The normal fast gate and required complete correctness CI remain green.

## Delivery Order

The intended sequence is:

1. invocation values and argument ownership;
2. synchronous call, construct, method, and property APIs;
3. first-class Rust callable values;
4. async host functions and the bidirectional Promise bridge;
5. external event scheduling and example-owned timers;
6. typed host objects and class registration;
7. the complete executable example suite and user-facing guide.

Each tranche should update the relevant architecture inventory and carry its
own direct API tests. Avoid one large embedding branch: the early synchronous
boundaries establish ownership and semantic truth that the async and host-class
work can build on without later rewrites.

## Completion Criteria

This roadmap is complete when:

- all fifteen examples run through documented public APIs;
- examples 03 through 06 contain no generated-source or `eval` workaround;
- Rust and JavaScript can call, construct, inspect, mutate, retain, and release
  values through VM-checked handles;
- async calls work in both directions under an embedder-owned executor;
- stored callbacks and external events have deterministic lifetime and
  cancellation behavior;
- typed Rust-backed classes use a general host-object mechanism;
- all new owners are represented in limits, GC, snapshots, and teardown;
- direct embedding tests, architecture checks, strict linting, and complete
  correctness CI pass.
