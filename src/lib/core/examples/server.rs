use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct RpcRequest {
    method: String,
    params: Vec<i32>,
}

#[derive(Serialize, Deserialize)]
struct RpcResponse {
    result: i32,
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await.unwrap();

    while let Ok((mut stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let mut buf = [0; 1024];
            let n = stream.read(&mut buf).await.unwrap();
            let request: RpcRequest = serde_json::from_slice(&buf[..n]).unwrap();

            // 处理远程调用（示例：加法）
            let result = match request.method.as_str() {
                "add" => request.params[0] + request.params[1],
                _ => 0,
            };

            let response = RpcResponse { result };
            let response_json = serde_json::to_vec(&response).unwrap();
            stream.write_all(&response_json).await.unwrap();
        });
    }
}