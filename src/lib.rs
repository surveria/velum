#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod ast;
mod error;
mod lexer;
mod parser;
mod runtime;
mod runtime_assertions;
mod runtime_completion;
mod runtime_numeric;
mod runtime_object;
mod runtime_scope;
mod value;

pub use crate::error::{Error, Result};
pub use crate::runtime::{Context, Runtime, RuntimeLimits};
pub use crate::value::Value;
