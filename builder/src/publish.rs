use std::error::Error;

pub fn publish(index: &[u8], js: &[u8], wasm: &[u8]) -> Result<(), Box<dyn Error>> {
    let repo = git2::Repository::open(".")?;
    let config = repo.config()?;
    let name = match config.get_string("user.name") {
        Ok(name) => name,
        Err(e) if e.code() == git2::ErrorCode::NotFound => whoami::user(),
        Err(e) => return Err(e.into()),
    };
    let email = config.get_string("user.email")?;
    let signature = git2::Signature::now(&name, &email)?;

    let mut tree_builder = repo.treebuilder(None)?;
    let index_blob = repo.blob(index)?;
    tree_builder.insert("index.html", index_blob, 0o100644)?;
    let js_blob = repo.blob(js)?;
    tree_builder.insert("index.js", js_blob, 0o100644)?;
    let wasm_blob = repo.blob(wasm)?;
    tree_builder.insert("tcpu.wasm", wasm_blob, 0o100644)?;

    let tree_id = tree_builder.write()?;
    let tree = repo.find_tree(tree_id)?;

    let commit_id = repo.commit(
        None,
        &signature,
        &signature,
        "publish",
        &tree,
        &[],
    )?;
    
    let commit = repo.find_commit(commit_id)?;
    repo.branch("gh-pages", &commit, true)?;

    println!("set gh-pages to commit {}", commit_id);

    Ok(())
}
