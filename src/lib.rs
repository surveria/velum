#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod api;
mod ast;
mod binding_layout;
mod binding_metadata;
mod build_info;
mod bytecode;
mod compiled_module;
mod compiled_script;
mod compiler;
mod error;
mod lexer;
mod ownership;
mod parser;
mod regexp_syntax;
mod runtime;
mod source;
mod storage;
mod syntax;
mod value;

pub use crate::api::embedding::{
    Engine, EngineConfig, Vm, VmConfig, VmResourceUsage, VmTeardownReport,
};
pub use crate::api::host::{
    FromJsValue, HostCall, HostFuture, HostFutureError, HostOperation, HostTaskResult, IntoJsValue,
    IntoOwnedJsValue, LocalValue,
};
pub use crate::api::invocation::{
    AccessorPropertyDefinition, DataPropertyDefinition, JsValueRef, PropertyDefinition,
    PropertyDescriptor, PropertyKeyRef,
};
pub use crate::api::owned_value::OwnedValue;
pub use crate::api::shared_array_buffer::SharedArrayBufferHandle;
pub use crate::build_info::{BuildInfo, engine_build_info};
pub use crate::compiled_module::{
    CompiledModule, DynamicModuleRequest, ModuleExport, ModuleImport, ModuleImportName,
    ModuleLoader, ModuleRequest, ModuleSource,
};
pub use crate::compiled_script::{CompiledScript, CompiledScriptUsage};
pub use crate::error::{Error, JavaScriptErrorMetadata, JavaScriptException, Result};
pub use crate::ownership::{VmGeneration, VmIdentity};
pub use crate::runtime::Context;
pub use crate::runtime::engine::Runtime;
pub use crate::runtime::limits::{RuntimeLimits, VmStorageLimits};
pub use crate::runtime::{
    HostAsyncContext, HostCommandRequest, HostFuturePoll, OptimizationMode, QueuedCallRequest,
    QueuedCallResult, RealmId, RetainedValue, VmAsyncEdgeKind, VmAsyncEdgeSnapshot,
    VmAsyncEdgeStrength, VmCallableEdgeKind, VmCallableEdgeSnapshot, VmGarbageCollectionReport,
    VmGcKind, VmHeapReachabilitySnapshot, VmObjectEdgeKind, VmObjectEdgeSnapshot,
    VmOptimizationSnapshot, VmRootKind, VmRootSnapshot, VmStorageKind, VmStorageSnapshot,
};
pub use crate::source::{SourceId, SourceSpan};
pub use crate::storage::string_heap::{JsString, StringId};
pub use crate::storage::symbol::{JsSymbol, SymbolId};
pub use crate::syntax::ImportPhase;
pub use crate::value::{JsBigInt, Value};
