use crate::quorum::Quorum;
use crate::rpc::Request;
use crate::rpc::RequestCarrier;
use crate::rpc::RequestVoteRep;
use crate::rpc::Response;
use crate::{
    client, collect_await, futures::all::*, rpc, rpc::RequestVoteReq, storage::Storage, util::*,
    Result, ServerId, TermId,
};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;

use crate::rpc::Server;
use futures::task::Waker;
use std::future::get_task_waker;
use std::future::poll_with_tls_waker;
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

struct ServerReceiver(Receiver<rpc::RequestCarrier>);

impl Clone for ServerReceiver {
    fn clone(&self) -> Self {
        unimplemented!()
    }
}

impl StdStream for ServerReceiver {
    type Item = RequestCarrier;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &std::task::Waker,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.0.poll() {
            Ok(Async::Ready(t)) => Poll::Ready(t),
            Ok(Async::NotReady) => Poll::Pending,
            Err(e) => {
                log::error!("ServerReceiver {:?}", e);
                Poll::Ready(None)
            }
        }
    }
}

impl Drop for ServerReceiver {
    fn drop(&mut self) {
        panic!("dropping server receiver");
        self.0.close();
    }
}

#[macro_use]
macro_rules! tick_err {
    ($self:ident, $e:expr) => {{
        if let Err(e) = $e {
            log::error!("error in tick: {}", e);
        }
    }};
}

pub struct RaftServer<S> {
    id: ServerId,
    receiver: Option<ServerReceiver>,
    config: Config,
    timeout: Option<Instant>,
    pub cycles: u64,
    mode: Mode,
    volatile_state: VolatileState,
    persisted_state: PersistedState,
    leader_state: Option<LeaderState>,
    storage: S,
    quorum: Quorum,
}

pub async fn new<S: Storage>(
    addr: SocketAddr,
    client_addrs: Vec<String>,
    storage: S,
) -> Result<RaftServer<S>> {
    let id = ServerId(addr);
    let quorum = Quorum::new(client_addrs);

    let persisted_state = match await!(storage.read_state())? {
        Some(state) => state,
        None => PersistedState {
            current_term: TermId(0),
            voted_for: None,
        },
    };

    Ok(RaftServer {
        id,
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
        quorum,
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
        log::debug!("processing messages");

        let mut rx = self.receiver.take().ok_or_else(|| "receiver went away")?;

        let result = await!(self.process_messages_helper(&mut rx));
        self.receiver.replace(rx);

        let count = result?;
        if count > 0 {
            log::info!("{} processed: {} messages", self.logline(), count);
            self.update_timeout(Instant::now());
        }
        Ok(())
    }

    async fn process_messages_helper<'a>(&'a mut self, rx: &'a mut ServerReceiver) -> Result<u64> {
        let mut messages = vec![];
        get_task_waker(|wk| {
            while let Poll::Ready(Some(msg)) = rx.poll_next_unpin(wk) {
                messages.push(msg);
            }
        });
        await!(self.handle_messages(messages))
    }

    /// Main entrypoint for the runloop.
    /// Returning Err<_> will stop the server.
    async fn tick(&mut self, t: Instant) -> Result<&mut Self> {
        log::debug!("tick: {}", self.logline());

        tick_err!(self, self.quorum.poll_response());
        tick_err!(self, await!(self.process_messages()));

        if self.timed_out(t) {
            log::debug!("timed out");
            tick_err!(self, await!(self.become_candidate()));
        }

        Ok(self)
    }

    /// Start the runloop. It is driven by a tokio::timer::Interval.
    /// Something more sophisticated than an Interval may be needed later.
    pub async fn start(mut self) -> Result<()> {
        let transport = bincode_transport::listen(&self.id.0)?;

        let (tx, rx) = tokio::sync::mpsc::channel(1_000);

        self.receiver.replace(ServerReceiver(rx));
        // TODO: Need to be able to shut this down.
        let server = tarpc::server::new(tarpc::server::Config::default())
            .incoming(transport)
            .respond_with(rpc::gen::serve(rpc::new_server(tx)));

        spawn_compat(server);

        while let Err(e) = await!(self.run()) {
            log::error!("{}", e);
        }
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        let result = await!(
            tokio::timer::Interval::new_interval(self.config.runloop_interval)
                .compat()
                .map_err(|_| crate::error::RaftErrorKind::ServerError("timer error").into())
                .try_fold(self, Self::tick)
        );

        if let Err(e) = result {
            log::error!("run error: {}", e);
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
        log::info!("{} become candidate", self.logline());

        self.update_timeout(Instant::now());
        self.quorum
            .request_vote(self.id, self.persisted_state.current_term)?;

        Ok(())
    }

    fn logline(&self) -> Logline {
        Logline {
            id: self.id,
            mode: self.mode,
            term: self.persisted_state.current_term,
        }
    }

    async fn handle_messages(&self, messages: Vec<RequestCarrier>) -> Result<u64> {
        let mut processed = 0;
        for msg in messages.into_iter() {
            processed += 1;
            match msg.body() {
                Request::RequestVote(ref _vote) => {
                    await!(msg.respond(Response::RequestVote(RequestVoteRep {
                        term: self.persisted_state.current_term,
                        vote_granted: true,
                    })))?
                }
                _ => (),
            }
        }
        Ok(processed)
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
