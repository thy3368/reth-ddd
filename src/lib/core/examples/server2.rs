use jsonrpc_core::{IoHandler, Params, Result, Value};
use jsonrpc_http_server::ServerBuilder;
use serde_json::json;

fn add(a: i64, b: i64) -> Result<Value> {
    Ok(json!(a + b))
}

fn main() {
    let mut io = IoHandler::new();
    io.add_method("add", |params: Params| async move {
        let (a, b): (i64, i64) = params.parse()?;
        add(a, b)
    });

    let server = ServerBuilder::new(io)
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .expect("Server startup failed");
    server.wait(); // 阻塞监听
}