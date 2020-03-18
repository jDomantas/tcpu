use warp::Filter;

fn wasm() -> Box<dyn warp::Reply> {
    match wasm_builder::compile_emulator() {
        Ok(bytes) => Box::new(
            warp::reply::with_header(
                bytes,
                "content-type",
                "application/wasm",
            ),
        ),
        Err(e) => Box::new(
            warp::reply::with_status(
                warp::reply::html(format!("failed to build: {}", e)),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ),
    }
}

fn js() -> Box<dyn warp::Reply> {
    match wasm_builder::compile_js() {
        Ok(js) => Box::new(
            warp::reply::with_header(
                js,
                "content-type",
                "text/javascript",
            ),
        ),
        Err(e) => Box::new(
            warp::reply::with_status(
                warp::reply::html(format!("failed to build: {}", e)),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ),
    }
}

#[tokio::main]
async fn main() {
    let html = warp::get()
        .and(warp::path::end())
        .and(warp::fs::file("./web/index.html"));

    let js = warp::get()
        .and(warp::path!("index.js"))
        .map(js);

    let wasm = warp::get()
        .and(warp::path!("tcpu.wasm"))
        .map(wasm);

    let app = html.or(js).or(wasm);

    warp::serve(app)
        .run(([127, 0, 0, 1], 8000))
        .await;
}
