use parity_wasm::elements::Module;
use std::error::Error;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("usage: {} <command>", args[0]);
        eprintln!("    wasm-size       Print wasm function sizes");
        eprintln!("    publish         Build web into github-pages branch");
        std::process::exit(1);
    }

    let result = match args[1].as_str() {
        "wasm-size" => wasm_size(),
        "publish" => publish(),
        other => {
            eprintln!("unknown command: {}", other);
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(2);
    }
}

fn wasm_size() -> Result<(), Box<dyn Error>> {
    let binary = builder::web::compile_emulator()?;    
    let module: Module = parity_wasm::deserialize_buffer(&binary).unwrap();
    let mut function_sizes = builder::analyze_wasm::function_sizes(module);
    function_sizes.sort_by_key(|&(_, size)| std::cmp::Reverse(size));

    println!("module size: {} kb ({} bytes)", binary.len() / 1024, binary.len());
    for (name, size) in &function_sizes {
        println!("{: >6} bytes | {}", size, name);
    }

    Ok(())
}

fn publish() -> Result<(), Box<dyn Error>> {
    let index = std::fs::read("./web/index.html")?;
    let js = builder::web::compile_js()?;
    let wasm = builder::web::compile_emulator()?;
    builder::publish(&index, &js, &wasm)?;
    Ok(())
}
