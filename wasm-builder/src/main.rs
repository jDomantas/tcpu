use parity_wasm::elements as wasm;
use std::collections::{HashMap, HashSet};

fn function_size(f: &wasm::FuncBody) -> usize {
    parity_wasm::serialize(f.clone()).unwrap().len()
}

fn find_callees(f: &wasm::FuncBody) -> HashSet<u32> {
    let mut set = HashSet::new();
    for i in f.code().elements() {
        if let wasm::Instruction::Call(idx) = *i {
            set.insert(idx);
        }
    }
    set
}

fn main() {
    let wasm = match wasm_builder::compile_emulator() {
        Ok(wasm) => wasm,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    println!("module size: {} kb ({} bytes)", wasm.len() / 1024, wasm.len());

    let module: wasm::Module = parity_wasm::deserialize_buffer(&wasm).unwrap();

    let module = module.parse_names().unwrap();
    
    let names = module
        .names_section()
        .expect("no name section")
        .functions()
        .expect("no function names")
        .names();
    let code = module.code_section().unwrap();

    struct Func {
        size: usize,
        index: u32,
        can_panic: bool,
        name: String,
    }

    let imported_functions = module
        .import_section()
        .unwrap()
        .entries()
        .iter()
        .filter(|e| if let wasm::External::Function(_) = e.external() { true } else { false })
        .count();

    let mut functions = Vec::new();
    let mut callees = HashMap::new();


    for (index, f) in code.bodies().iter().enumerate() {
        let index = (index + imported_functions) as u32;
        let size = function_size(f);
        let name = names
            .get(index as u32)
            .map(ToString::to_string)
            .unwrap();
        functions.push(Func {
            can_panic: name.contains("panic"),
            size,
            name,
            index,
        });
        callees.insert(index, find_callees(f));
    }

    for _ in 0..functions.len() {
        for i in 0..functions.len() {
            let idx = functions[i].index;
            // println!("check index ")
            if callees[&idx].iter().any(|&callee| callee != 0 && functions.iter().find(|f| f.index == callee).expect("no callee").can_panic) {
                functions[i].can_panic = true;
            }
        }
    }

    functions.sort_by_key(|f| std::cmp::Reverse(f.size));

    for Func { name, size, can_panic, .. } in &functions {
        println!("{: >6} bytes | {}", size, name);
        if *can_panic {
            println!("             | --> can panic");
        }
    }
}