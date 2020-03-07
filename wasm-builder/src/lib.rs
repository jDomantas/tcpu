use std::error::Error as StdError;
use std::fmt;
use std::process::Command;

#[derive(Debug)]
pub struct Error {
    inner: Box<dyn StdError>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<E: StdError + 'static> From<E> for Error {
    fn from(error: E) -> Self {
        Error {
            inner: error.into(),
        }
    }
}

#[derive(Debug)]
struct CompileError {
    stderr: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "compilation failed:\n{}", self.stderr)
    }
}

impl StdError for CompileError {}

#[derive(Debug)]
struct CustomError {
    message: String,
    source: Error,
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        let mut err: &dyn StdError = self;
        while let Some(e) = err.source() {
            write!(f, "\n  {}", e)?;
            err = e;
        }
        Ok(())
    }
}

impl StdError for CustomError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.source.inner)
    }
}

trait ResultExt<T> {
    fn with_context(self, message: impl Into<String>) -> Result<T, Error>;
}

impl<T, E: StdError + 'static> ResultExt<T> for Result<T, E> {
    fn with_context(self, message: impl Into<String>) -> Result<T, Error> {
        self.map_err(|source| CustomError { message: message.into(), source: source.into() }.into())
    }
}

impl<T> ResultExt<T> for Result<T, Error> {
    fn with_context(self, message: impl Into<String>) -> Result<T, Error> {
        self.map_err(|source| CustomError { message: message.into(), source }.into())
    }
}

pub fn compile_emulator() -> Result<Vec<u8>, Error> {
    let output = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("tcpu-wasm")
        .arg("--release")
        .arg("--target=wasm32-unknown-unknown")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(CompileError { stderr }.into());
    }

    let wasm = std::fs::read("./target/wasm32-unknown-unknown/release/tcpu_wasm.wasm")
        .with_context("failed to read compiled wasm")?;
    let wasm = wasm_gc::garbage_collect_slice(&wasm)
        .with_context("failed to gc wasm module")?;

    Ok(wasm)
}
