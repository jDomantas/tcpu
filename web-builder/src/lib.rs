use std::error::Error;
use std::fmt;
use std::process::Command;

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

pub fn compile_emulator() -> Result<Vec<u8>, Box<dyn Error>> {
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

    let wasm = std::fs::read("./target/wasm32-unknown-unknown/release/tcpu_wasm.wasm")?;
    let wasm = wasm_gc::garbage_collect_slice(&wasm)?;

    Ok(wasm)
}

pub fn compile_js() -> Result<Vec<u8>, Box<dyn Error>> {
    let output = Command::new("node")
        .arg("./node_modules/typescript/bin/tsc")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(CompileError { stderr }.into());
    }

    let js = std::fs::read("./target/web/index.js")?;

    Ok(js)
}
