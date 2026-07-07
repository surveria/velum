#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod api;
mod ast;
mod binding_layout;
mod bytecode;
mod compiled_script;
mod error;
mod lexer;
mod parser;
mod runtime;
mod storage;
mod syntax;
mod value;

pub use crate::api::embedding::{
    Engine, EngineConfig, Vm, VmConfig, VmResourceUsage, VmTeardownReport,
};
pub use crate::api::host::{FromJsValue, HostCall, IntoJsValue};
pub use crate::compiled_script::{CompiledScript, CompiledScriptUsage};
pub use crate::error::{Error, Result};
pub use crate::runtime::Context;
pub use crate::runtime::engine::Runtime;
pub use crate::runtime::limits::RuntimeLimits;
pub use crate::storage::string_heap::{JsString, StringId};
pub use crate::storage::symbol::{JsSymbol, SymbolId};
pub use crate::value::Value;
