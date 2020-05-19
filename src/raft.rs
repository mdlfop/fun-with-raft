use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Instant, Duration};

use super::rpc;
use rpc::{RequestVoteArgs, RequestVoteReply, AppendEntryArgs, AppendEntryReply};
use rpc::rpc_client::RpcClient;

use rand::{thread_rng, Rng};
use tokio::sync::Mutex;

const DEBUG_MODE: bool = true;
const FORCE_MORE_ELECTIONS: bool = true;

#[derive(Debug, Clone, Default)]
pub struct LogEntry {
    command: String,
    term: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ServerState {
    Dead,
    Follower,
    Candidate,
    Leader,
}

impl fmt::Display for ServerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerState::Dead => write!(f, "[Dead]"),
            ServerState::Follower => write!(f, "[Follower]"),
            ServerState::Candidate => write!(f, "[Candidate]"),
            ServerState::Leader => write!(f, "[Leader]"),
        }
    }
}
#[derive(Debug)]
pub struct RaftServer {
    id: u64,
    pub peer_ids: Vec<u64>,
    pub peer_connections: HashMap<u64, http::Uri>,

    current_term: u64,
    voted_for: Option<u64>,
    log: Vec<LogEntry>,

    commit_index: u64,
    last_applied: u64,

    state: ServerState,
    election_reset_event: Instant,

    next_index: HashMap<u64, u64>,
    match_index: HashMap<u64, u64>,
}

impl RaftServer {
    fn new(id: u64, peer_ids: Vec<u64>, peer_connections: HashMap<u64, http::Uri>) -> Self {
        Self {
            id,
            peer_ids,
            peer_connections,
            
            current_term: 0,
            voted_for: None,
            log: vec![],

            commit_index: 0,
            last_applied: 0,

            state: ServerState::Follower,
            election_reset_event: Instant::now(),

            next_index: Default::default(),
            match_index: Default::default(),
        }
    }

    fn debug(&self, format_string: String) {
        if DEBUG_MODE {
            println!("[{}] [{}] {}", chrono::Local::now().format("%H:%M:%S"), self.id, format_string);
        }
    }

    fn become_follower(&mut self, term: u64, enclosed: RaftServerEnclosed) {
        self.state = ServerState::Follower;
        self.current_term = term;
        self.voted_for = None;
        self.election_reset_event = Instant::now();
        self.debug(format!("becomes follower with term = {}, log = {:?}", self.current_term, self.log));
        tokio::spawn(async move {
            enclosed.start_election_timer().await;
        });
    }

    fn become_candidate(&mut self, enclosed: RaftServerEnclosed) {
        self.state = ServerState::Candidate;
        self.current_term += 1;
        self.election_reset_event = Instant::now();
        self.voted_for = Some(self.id);
        self.debug(format!("becomes Candidate, term = {}, log = {:?}", self.current_term, self.log));
        tokio::spawn(async move {
            enclosed.start_election_timer().await;
        });
    }

    fn become_leader(&mut self, enclosed: RaftServerEnclosed) {
        self.state = ServerState::Leader;
        self.debug(format!("becomes Leader, term = {}, log = {:?}", self.current_term, self.log));
        let a = async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            loop {
                enclosed.leader_send_heartbeats().await;
                interval.tick().await;
                let rs = enclosed.0.lock().await;
                if rs.state != ServerState::Leader {
                    return;
                }
            }
        };
        tokio::spawn(a);
    }

    fn request_vote(&self, peer_id: u64, saved_current_term: u64, votes_received: Arc<Mutex<Box<u64>>>, enclosed: RaftServerEnclosed) {
        let mut args = RequestVoteArgs::default();
        args.term = saved_current_term;
        args.candidate_id = self.id;
        
        let a = async move {
            let mut rs = enclosed.0.lock().await;
            rs.debug(format!("sending RequestVote [{:?}]to {}", args, peer_id));
            let reply = { 
                let mut client = RpcClient::connect(rs.peer_connections.get(&peer_id).unwrap().clone()).await.unwrap();
                client.request_vote(args).await.unwrap().into_inner()
            };
            rs.debug(format!("received RequestVote reply: [{:?}]", reply));
            if rs.state != ServerState::Candidate {
                rs.debug(format!("while waiting for the reply, state = [{}]", rs.state));
                return;
            }
            if reply.term > saved_current_term {
                rs.debug(format!("term out of date in RequestVoteReply"));
                rs.become_follower(reply.term, enclosed.clone());
                return;
            }
            if reply.term == saved_current_term {
                if reply.vote_granted {
                    let mut votes_received = votes_received.lock().await;
                    **votes_received += 1;
                    if **votes_received * 2 > (rs.peer_ids.len() + 1)  as u64 {
                        rs.debug(format!("wins the election with {} votes", **votes_received));
                        rs.become_leader(enclosed.clone());
                        return;
                    }
                }
            }
        };
        tokio::spawn(a);
    }

}
#[derive(Debug, Clone)]
pub struct RaftServerEnclosed(pub Arc<Mutex<RaftServer>>);

impl RaftServerEnclosed {
    pub async fn new(id: u64, peer_ids: Vec<u64>, peer_connections: HashMap<u64, http::Uri>) -> Self {
        let a = Self(Arc::new(Mutex::new(RaftServer::new(id, peer_ids, peer_connections))));
        let b = a.clone();
        // add a barrier here @TODO
        let mut rs = b.0.lock().await;
        rs.become_follower(0, a.clone());
        a
    }

    fn election_timeout() -> Duration {
        let mut rng = thread_rng();
        if FORCE_MORE_ELECTIONS && rng.gen_range(0, 3) == 0{
            Duration::from_millis(150)
        } else {
            Duration::from_millis(rng.gen_range(150, 300))
        }
    }

    async fn start_election_timer(&self) {
        let timeout_duration = Self::election_timeout();
        
        let rs = self.0.lock().await;
        let term_started = rs.current_term;

        rs.debug(format!("election timer started for {:?}, term = {}", timeout_duration, term_started));
        drop(rs);

        let mut interval = tokio::time::interval(Duration::from_millis(10));
        loop {
            interval.tick().await;
            let mut rs = self.0.lock().await;
            if rs.state != ServerState::Follower && rs.state != ServerState::Candidate {
                rs.debug(format!("running election timer in state => {}, so bailing out", rs.state));
                return;
            }
            if term_started != rs.current_term {
                rs.debug(format!("in election timer, term changed from {} to {}, bailing out", term_started, rs.current_term));
                return;
            }
            if rs.election_reset_event.elapsed() > timeout_duration {
                rs.become_candidate(self.clone());
                let saved_current_term = rs.current_term;
                let votes_received = Arc::new(Mutex::new(Box::new(1 as u64)));
                for &peer_id in &rs.peer_ids {
                    rs.request_vote(peer_id, saved_current_term, votes_received.clone(), self.clone());
                }
                return;
            }
        }
    }

    pub async fn leader_send_heartbeats(&self) {
        let rs = self.0.lock().await;
        let saved_current_term = rs.current_term;
        for &peer_id in &rs.peer_ids {
            
            let mut args = AppendEntryArgs::default();
            args.term = saved_current_term;
            args.leader_id = rs.id;

            let enclosed = self.clone();
            let a = async move {
                let mut rs = enclosed.0.lock().await;
                rs.debug(format!("sending AppendEntries to {}, args: [{:?}]", peer_id, args));
                let reply = { 
                    let mut client = RpcClient::connect(rs.peer_connections.get(&peer_id).unwrap().clone()).await.unwrap();
                    client.append_entry(args).await.unwrap().into_inner()
                };
               
                if reply.term > saved_current_term {
                    rs.debug(format!("term out of date in heartbeat reply"));
                    rs.become_follower(reply.term, enclosed.clone());
                    return;
                }
            };
            tokio::spawn(a);
        }
    }

    pub async fn handle_request_vote(&self, args: RequestVoteArgs) -> RequestVoteReply {
        let mut reply = RequestVoteReply::default();
        let mut rs = self.0.lock().await;
        if rs.state == ServerState::Dead {
            return reply;
        }
        rs.debug(format!("RequestVote rpc received [args: {:?}], sending [current term: {}  voted for: {:?}]", args, rs.current_term, rs.voted_for));
        if args.term > rs.current_term {
            rs.debug(format!("... term out of date in RequestVote"));
            rs.become_follower(args.term, self.clone());
        }
        if args.term == rs.current_term {
            if rs.voted_for.is_none() || rs.voted_for == Some(args.candidate_id) {
                reply.vote_granted = true;
                rs.voted_for = Some(args.candidate_id);
                rs.election_reset_event = Instant::now();
            }
        }
        reply.term = rs.current_term;
        rs.debug(format!("... RequestVote reply: [{:?}]", reply));
        reply
    }

    pub async fn handle_append_entry(&self, args: AppendEntryArgs) -> AppendEntryReply {
        let mut reply = AppendEntryReply::default();
        let mut rs = self.0.lock().await;
        if rs.state == ServerState::Dead {
            return reply;
        }
        rs.debug(format!("AppendEntry rpc received, args: [{:?}]", args));
        if args.term > rs.current_term {
            rs.debug(format!("... term out of date in AppendEntry rpc handler"));
            rs.become_follower(args.term, self.clone());
        }
        if args.term == rs.current_term {
            if rs.state != ServerState::Follower {
                rs.become_follower(args.term, self.clone());
            }
            rs.election_reset_event = Instant::now();
            reply.success = true;
        }
        reply.term = rs.current_term;
        rs.debug(format!("AppendEntry reply: [{:?}]", reply));
        reply
    }
}