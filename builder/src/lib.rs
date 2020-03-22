pub mod analyze_wasm;
mod publish;
pub mod web;

use std::error::Error;
use std::fmt;
pub use crate::publish::publish;

#[derive(Debug)]
pub struct CompileError {
    stderr: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to compile:\n{}", self.stderr)
    }
}

impl Error for CompileError {}
