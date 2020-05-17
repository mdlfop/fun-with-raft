use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};

use rand::{thread_rng, Rng};

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

struct RaftServer {
    id: u64,
    peer_ids: Vec<u64>,
    peer_connections: HashMap<u64, >,

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
    fn new(id: u64, peer_ids: Vec<u64>, peer_connections: HashMap<u64, >) -> Self {
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

            next_index: 0,
            match_index: 0,
        }
    }

    fn debug(&self, format_string: String) {
        if DEBUG_MODE {
            println!("[{}] [{}] {}", chrono::Local::now().format("%H:%M:%S"), self.id, format_string);
        }
    }

    fn become_follower(&mut self, term: u64) {
        self.state = ServerState::Follower;
        self.current_term = term;
        self.voted_for = None;
        self.election_reset_event = Instant::now();
        self.debug(format!("becomes follower with term = {}, log = {:?}", self.current_term, self.log));
    }

    fn become_candidate(&mut self) {
        self.state = ServerState::Candidate;
        self.current_term += 1;
        self.election_reset_event = Instant::now();
        self.voted_for = Some(self.id);
        self.debug(format!("becomes Candidate, term = {}, log = {:?}", self.current_term, self.log));
    }

    fn become_leader(&mut self) {
        self.state = ServerState::Leader;
        self.debug(format!("becomes Leader, term = {}, log = {:?}", self.current_term, self.log));
    }

    fn request_vote(&self, votes_received: Arc<Mutex<Box<u64>>>, enclosed: RaftServerEnclosed) {
        let a = async move {
            // let mut args = 
        };
        tokio::spawn(a);
    }

}
#[derive(Debug, Clone)]
struct RaftServerEnclosed(Arc<Mutex<RaftServer>>);

impl RaftServerEnclosed {
    pub fn new(id: u64, peer_ids: Vec<u64>, peer_connections: HashMap<u64, >) -> Self {
        let a = Self(Arc::new(Mutex::new(RaftServer::new(id, peer_ids, peer_connections))));
        let b = a.clone();
        // add a barrier here @TODO
        let mut rs = b.0.lock().unwrap();
        rs.become_follower(0);
        tokio::spawn(async move {
            b.start_election_timer().await;
        });
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
        
        let rs = self.0.lock().unwrap();
        let term_started = rs.current_term;

        rs.debug(format!("election timer started for {:?}, term = {}", timeout_duration, term_started));
        drop(rs);

        let interval = tokio::time::interval(Duration::from_millis(10));
        loop {
            interval.tick().await;
            let rs = self.0.lock().unwrap();
            if rs.state != ServerState::Follower && rs.state != ServerState::Candidate {
                rs.debug(format!("running election timer in state => {}, so bailing out", rs.state));
                return;
            }
            if term_started != rs.current_term {
                rs.debug(format!("in election timer, term changed from {} to {}, bailing out", term_started, rs.current_term));
                return;
            }
            if rs.election_reset_event.elapsed() > timeout_duration {
                rs.become_candidate();
                let votes_received = Arc::new(Mutex::new(Box::new(1 as u64)));
                for peer_id in rs.peer_ids {
                    let votes_received_clone = votes_received.clone();
                    let a = self.clone();

                }
                self.start_election().await;
                return;
            }
        }
    }

    async fn start_election(&self) {
        let votes_received = Arc::new(Mutex::new(Box::new(1 as u64)));
        for peer_id in peer_ids 
    }
}