use crate::rpc::Request;
use crate::rpc::RequestCarrier;
use crate::rpc::RequestVoteRep;
use crate::rpc::Response;
use crate::{
    client::{self, Peers},
    collect_await,
    futures::all::*,
    rpc,
    rpc::RequestVoteReq,
    storage::Storage,
    util::*,
    Result, ServerId, TermId,
};
use log::{debug, error, info};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;
use tarpc::server::Handler;
use tarpc_bincode_transport as bincode_transport;
use tokio::prelude::Async;
use tokio::sync::mpsc::*;

pub struct Config {
    pub election_interval: (u64, u64),
    pub runloop_interval: Duration,
}

#[derive(Copy, Clone)]
enum Mode {
    Leader,
    Follower,
    Candidate,
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        let m = match self {
            Mode::Leader => "Leader",
            Mode::Follower => "Follower",
            Mode::Candidate => "Candidate",
        };
        write!(f, "Mode({})", m)
    }
}

struct VolatileState {
    commit_index: u64,
    last_applied: u64,
}

impl VolatileState {
    fn new() -> Self {
        VolatileState {
            commit_index: 0,
            last_applied: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PersistedState {
    current_term: TermId,
    voted_for: Option<ServerId>,
}

struct LeaderState {
    next_index: HashMap<ServerId, u64>,
    match_index: HashMap<ServerId, u64>,
}

impl LeaderState {
    fn new() -> LeaderState {
        LeaderState {
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }
}

pub struct RaftServer<S> {
    id: ServerId,
    receiver: Option<Receiver<rpc::RequestCarrier>>,
    peers: Peers,
    config: Config,
    timeout: Option<Instant>,
    pub cycles: u64,
    mode: Mode,
    volatile_state: VolatileState,
    persisted_state: PersistedState,
    leader_state: Option<LeaderState>,
    storage: S,
    election_results: Option<tokio::sync::oneshot::Receiver<Vec<bool>>>,
}

pub async fn new<S: Storage>(
    addr: SocketAddr,
    client_addrs: Vec<String>,
    storage: S,
) -> Result<RaftServer<S>> {
    let id = ServerId(addr);
    let clients = client_addrs.into_iter().map(client::new).collect();

    let persisted_state = match await!(storage.read_state())? {
        Some(state) => state,
        None => PersistedState {
            current_term: TermId(0),
            voted_for: None,
        },
    };

    Ok(RaftServer {
        id,
        peers: Peers::new(clients),
        config: Config {
            election_interval: (300, 500),
            runloop_interval: Duration::from_millis(10),
        },
        timeout: None,
        receiver: None,
        cycles: 0,
        mode: Mode::Follower,
        volatile_state: VolatileState::new(),
        persisted_state: persisted_state,
        leader_state: None,
        storage,
        election_results: None,
    })
}

impl<S: Storage + Unpin> RaftServer<S> {
    /// Check if we've exceeded election timeout, or set timeout if none is set.
    fn timed_out(&mut self, now: Instant) -> bool {
        if let Some(ref timeout) = self.timeout {
            *timeout <= now
        } else {
            self.update_timeout(now);
            false
        }
    }

    fn update_timeout(&mut self, now: Instant) {
        let mut rng = rand::thread_rng();
        let interval = self.config.election_interval;
        let instant = now + Duration::from_millis(rng.gen_range(interval.0, interval.1));
        self.timeout.replace(instant);
    }

    /// Process messages in channel. RPC requests get queued into this channel from the
    /// RPC services, who clone the sender end of the channel.
    async fn process_messages(&mut self) -> Result<()> {
        debug!("processing messages");

        let mut rx = self.receiver.take().ok_or_else(|| "receiver went away")?;

        let result = await!(self.process_messages_helper(&mut rx));
        self.receiver.replace(rx);

        let count = result?;
        if count > 0 {
            info!("{} processed: {} messages", self.logline(), count);
            self.update_timeout(Instant::now());
        }
        Ok(())
    }

    async fn process_messages_helper<'a>(
        &'a mut self,
        rx: &'a mut Receiver<RequestCarrier>,
    ) -> Result<u64> {
        let mut count = 0;
        while let Ok(Async::Ready(opt)) = rx.poll() {
            if let Some(msg) = opt {
                count += 1;
                await!(self.handle_message(msg))?;
            } else {
                panic!("receiver went away!!!");
            }
        }
        Ok(count)
    }

    /// Main entrypoint for the runloop.
    /// Returning Err<_> will stop the server.
    async fn tick(mut self, t: Instant) -> Result<RaftServer<S>> {
        self.check_election_results()?;
        await!(self.process_messages())?;

        if self.timed_out(t) {
            debug!("timed out");
            await!(self.become_candidate())?;
        } else {
            debug!("{}", self.logline());
            debug!("not timed out");
        }

        Ok(self)
    }

    /// Start the runloop. It is driven by a tokio::timer::Interval.
    /// Something more sophisticated than an Interval may be needed later.
    pub async fn start(mut self) -> Result<RaftServer<S>> {
        let transport = bincode_transport::listen(&self.id.0)?;

        let (tx, rx) = tokio::sync::mpsc::channel(1_000);

        self.receiver.replace(rx);
        // TODO: Need to be able to shut this down.
        let server = tarpc::server::new(tarpc::server::Config::default())
            .incoming(transport)
            .respond_with(rpc::gen::serve(rpc::new_server(tx)));

        spawn_compat(server);
        await! {
            tokio::timer::Interval::new_interval(self.config.runloop_interval)
            .compat()
            .map_err(|_| RaftError::ServerError("timer error"))
            .try_fold(self, Self::tick)
        }
    }

    fn check_election_results(&mut self) -> Result<()> {
        if let Mode::Candidate = self.mode {
            if let Some(ref mut rx) = self.election_results {
                match rx.poll() {
                    Ok(Async::Ready(v)) => {
                        info!("got result: {:?}", v);
                        self.election_results.take();
                    }
                    Ok(Async::NotReady) => (),
                    Err(e) => {
                        error!("{}, {:?}", self.logline(), e);
                        self.election_results.take();
                    }
                }
            }
        }
        Ok(())
    }
    async fn update_state(&mut self, state: PersistedState) -> Result<()> {
        await!(self.storage.update_state(state.clone()))?;
        self.persisted_state = state;
        Ok(())
    }

    async fn become_candidate(&mut self) -> Result<()> {
        self.mode = Mode::Candidate;
        let current_term = TermId(self.persisted_state.current_term.0 + 1);
        let voted_for = Some(self.id);
        await!(self.update_state(PersistedState {
            current_term,
            voted_for
        }))?;
        info!("{} become candidate", self.logline());

        self.update_timeout(Instant::now());

        self.election_results
            .replace(self.peers.request_vote(RequestVoteReq {}));

        Ok(())
    }

    fn logline(&self) -> Logline {
        Logline {
            id: self.id,
            mode: self.mode,
            term: self.persisted_state.current_term,
        }
    }

    async fn handle_message(&self, msg: RequestCarrier) -> Result<()> {
        match msg.body() {
            Request::RequestVote(ref vote) => {
                await!(msg.respond(Response::RequestVote(RequestVoteRep {
                    term: self.persisted_state.current_term,
                    vote_granted: true,
                })))
            }
            _ => Ok(()),
        }
    }
}

struct Logline {
    id: ServerId,
    mode: Mode,
    term: TermId,
}

impl Display for Logline {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), fmt::Error> {
        write!(f, "[{}: {} {}]", self.id, self.mode, self.term)
    }
}
