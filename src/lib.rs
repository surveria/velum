#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod ast;
mod error;
mod lexer;
mod parser;
mod runtime;
mod runtime_assertions;
mod runtime_completion;
mod runtime_engine;
mod runtime_limits;
mod runtime_numeric;
mod runtime_object;
mod runtime_property;
mod runtime_scope;
mod value;

pub use crate::error::{Error, Result};
pub use crate::runtime::Context;
pub use crate::runtime_engine::Runtime;
pub use crate::runtime_limits::RuntimeLimits;
pub use crate::value::Value;
