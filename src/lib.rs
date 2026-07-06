#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod ast;
mod atom;
mod binding_layout;
mod bytecode;
mod compiled_script;
mod embedding;
mod error;
mod host;
mod lexer;
mod native_call;
mod parser;
mod runtime;
mod string_heap;
mod value;

pub use crate::compiled_script::{CompiledScript, CompiledScriptUsage};
pub use crate::embedding::{Engine, EngineConfig, Vm, VmConfig, VmResourceUsage, VmTeardownReport};
pub use crate::error::{Error, Result};
pub use crate::host::{FromJsValue, HostCall, IntoJsValue};
pub use crate::runtime::Context;
pub use crate::runtime::engine::Runtime;
pub use crate::runtime::limits::RuntimeLimits;
pub use crate::string_heap::{JsString, StringId};
pub use crate::value::Value;
