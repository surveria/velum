#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod ast;
mod atom;
mod binding_layout;
mod binding_layout_types;
mod bytecode;
mod bytecode_hoist;
mod bytecode_types;
mod compiled_script;
mod embedding;
mod error;
mod host;
mod lexer;
mod lexer_support;
mod parser;
mod runtime;
mod runtime_assertions;
mod runtime_call_args;
mod runtime_completion;
mod runtime_engine;
mod runtime_limits;
mod runtime_numeric;
mod runtime_object;
mod runtime_property;
mod runtime_scope;
mod string_heap;
mod value;

pub use crate::compiled_script::{CompiledScript, CompiledScriptUsage};
pub use crate::embedding::{Engine, EngineConfig, Vm, VmConfig, VmResourceUsage, VmTeardownReport};
pub use crate::error::{Error, Result};
pub use crate::host::{FromJsValue, HostCall, IntoJsValue};
pub use crate::runtime::Context;
pub use crate::runtime_engine::Runtime;
pub use crate::runtime_limits::RuntimeLimits;
pub use crate::string_heap::{JsString, StringId};
pub use crate::value::Value;
