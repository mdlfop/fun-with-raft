mod raft;
mod rpc;
mod communication;

use rpc::rpc_server::{RpcServer};
use tonic::{transport::Server};
#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let mut handles = vec![];
    let mut nodes = vec![];
    for i in 1..=5 {
        nodes.push(communication::MyServer::new(i).await);
    }
    for (id, node) in nodes.into_iter().enumerate() {
        let addr = format!("[::1]:{}", 50050+id+1).parse().unwrap();
        let h = tokio::spawn(async move {
            Server::builder()
            .add_service(RpcServer::new(node))
            .serve(addr)
            .await.unwrap();
        });
        handles.push(h);
    }
    for h in handles {
        h.await.expect("join handle panicked");
    }
}
