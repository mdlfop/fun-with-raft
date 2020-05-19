use super::raft::RaftServerEnclosed;

use std::collections::HashMap;

use super::rpc;
use rpc::rpc_server::{Rpc, RpcServer};
use rpc::{RequestVoteArgs, RequestVoteReply, AppendEntryArgs, AppendEntryReply};
use rpc::rpc_client::RpcClient;
use tonic::{transport::Server, Request, Response, Status};

pub struct MyServer {
    pub rs: RaftServerEnclosed,
}

impl MyServer {
    pub async fn new(id: u64) -> Self {
        let peer_ids: Vec<u64> = [1, 2, 3, 4, 5].iter().filter(|i| **i != id).map(|i| *i).collect::<Vec<u64>>();
        let peer_connections: HashMap<u64, RpcClient<tonic::transport::channel::Channel>> = Default::default();
        
        let s = Self {
            rs: RaftServerEnclosed::new(
                id,
                peer_ids, 
                peer_connections
            ).await,
        };
        s
        
    }

    pub async fn configure_connections(&mut self) {
        let mut rs = self.rs.0.lock().await;
        let peer_ids = rs.peer_ids.clone();
        for i in peer_ids {
            let conn = RpcClient::connect(format!("https://[::1]:{}", 50050+i)).await.unwrap();
            rs.peer_connections.insert(i, conn);
        }
    }
}

#[tonic::async_trait]
impl Rpc for MyServer {
    async fn request_vote(
        &self,
        request: Request<RequestVoteArgs>,
    ) -> Result<Response<RequestVoteReply>, Status> {
        let reply = self.rs.handle_request_vote(request.into_inner()).await;
        Ok(Response::new(reply))
    }
    async fn append_entry(
        &self,
        request: Request<AppendEntryArgs>,
    ) -> Result<Response<AppendEntryReply>, Status> {
        let reply = self.rs.handle_append_entry(request.into_inner()).await;
        Ok(Response::new(reply))
    }
}


