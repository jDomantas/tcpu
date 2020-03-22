use parity_wasm::elements as wasm;

fn function_size(f: &wasm::FuncBody) -> usize {
    parity_wasm::serialize(f.clone()).unwrap().len()
}

pub fn function_sizes(module: wasm::Module) -> Vec<(String, usize)> {
    let module = module.parse_names().unwrap();

    let names = module
        .names_section()
        .expect("no name section")
        .functions()
        .expect("no function names")
        .names();
    let code = module.code_section().unwrap();

    let imported_functions = module
        .import_section()
        .unwrap()
        .entries()
        .iter()
        .filter(|e| if let wasm::External::Function(_) = e.external() { true } else { false })
        .count();

    let mut functions = Vec::new();

    for (index, f) in code.bodies().iter().enumerate() {
        let index = (index + imported_functions) as u32;
        let size = function_size(f);
        let name = names
            .get(index as u32)
            .unwrap()
            .to_string();
        functions.push((name, size));
    }
    
    functions
}
