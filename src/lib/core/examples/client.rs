use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::json;

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let request = json!({
        "method": "add",
        "params": [5, 3]
    });
    let request_bytes = serde_json::to_vec(&request).unwrap();

    stream.write_all(&request_bytes).await.unwrap();
    let mut response_buf = [0; 1024];
    let n = stream.read(&mut response_buf).await.unwrap();
    let response: serde_json::Value = serde_json::from_slice(&response_buf[..n]).unwrap();
    println!("Result: {}", response["result"]); // 输出: 8
}