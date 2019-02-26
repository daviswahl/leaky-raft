use crate::{client, futures::*, rpc, storage::Storage, util::*, Result, ServerId, TermId};
use log::debug;
use log::info;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
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

enum Mode {
    Leader,
    Follower,
    Candidate,
}

struct VolatileState {}
#[derive(Serialize, Deserialize, Debug)]
pub struct PersistedState {
    current_term: TermId,
    voted_for: Option<ServerId>,
}

struct LeaderState {}

pub struct RaftServer<S> {
    id: ServerId,
    receiver: Option<Receiver<rpc::RequestCarrier>>,
    clients: Vec<client::Client>,
    config: Config,
    timeout: Option<Instant>,
    pub cycles: usize,
    mode: Mode,
    volatile_state: VolatileState,
    persisted_state: PersistedState,
    leader_state: Option<LeaderState>,
    storage: S,
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
        clients,
        config: Config {
            election_interval: (300, 500),
            runloop_interval: Duration::from_millis(10),
        },
        timeout: None,
        receiver: None,
        cycles: 0,
        mode: Mode::Follower,
        volatile_state: VolatileState {},
        persisted_state: persisted_state,
        leader_state: None,
        storage,
    })
}

impl<S: Storage> RaftServer<S> {
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
        while let Ok(Async::Ready(msg)) = rx.poll() {
            debug!("{:?}", msg);
        }
        debug!("no more messages");
        self.receiver.replace(rx);
        Ok(())
    }

    /// Main entrypoint for the runloop.
    /// Returning Err<_> will stop the server.
    async fn tick(mut self, t: Instant) -> Result<Self> {
        await!(self.process_messages())?;

        if self.timed_out(t) {
            debug!("timed out");
        } else {
            debug!("{:?}", self.persisted_state);
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
}
