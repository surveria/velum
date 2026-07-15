#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod app;
mod session;

pub use app::run;
pub use session::{ShellSession, Submission};
