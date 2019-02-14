use crate::client;
use crate::rpc;
use crate::util;
use crate::Result;
use futures::compat::Stream01CompatExt;
use futures::TryStreamExt;
use futures_01::stream::Stream;
use log::debug;
use rand::prelude::*;
use std::time::Duration;
use std::time::Instant;
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

trait PersistedState {
    type Storage: crate::storage::Storage;
}

struct LeaderState {}

pub struct RaftServer<S> {
    receiver: Receiver<rpc::RequestCarrier>,
    clients: Vec<client::Client>,
    config: Config,
    timeout: Option<Instant>,
    pub cycles: usize,
    mode: Mode,
    volatile_state: VolatileState,
    persisted_state: S,
    leader_state: Option<LeaderState>,
}

pub fn new<S>(
    rx: Receiver<rpc::RequestCarrier>,
    client_addrs: Vec<String>,
    storage: S,
) -> RaftServer<S> {
    let clients = client_addrs.into_iter().map(client::new).collect();
    RaftServer {
        clients,
        config: Config {
            election_interval: (300, 500),
            runloop_interval: Duration::from_millis(10),
        },
        timeout: None,
        receiver: rx,
        cycles: 0,
        mode: Mode::Follower,
        volatile_state: VolatileState {},
        persisted_state: storage,
        leader_state: None,
    }
}

impl<S> RaftServer<S> {
    /// Check if we've exceeded election timeout, or set timeout if none is set.
    fn timed_out(&mut self, now: Instant) -> bool {
        if let Some(ref timeout) = self.timeout {
            *timeout <= now
        } else {
            let mut rng = rand::thread_rng();
            let interval = self.config.election_interval;
            let instant = now + Duration::from_millis(rng.gen_range(interval.0, interval.1));
            self.timeout.replace(instant);
            false
        }
    }

    /// Process messages in channel. RPC requests get queued into this channel from the
    /// RPC services, who clone the sender end of the channel.
    async fn process_messages(&mut self) -> Result<()> {
        debug!("processing messages");
        while let Ok(Async::Ready(msg)) = self.receiver.poll() {
            debug!("{:?}", msg);
        }
        debug!("no more messages");
        Ok(())
    }

    /// Theserver's main entrypoint, called each tick of the runloop.
    /// Returning Err<_> will stop the server.
    async fn update(mut self, t: Instant) -> Result<Self> {
        await!(self.process_messages())?;

        if self.timed_out(t) {
            debug!("timed out");
        } else {
            debug!("not timed out");
        }

        Ok(self)
    }

    /// Start the runloop. It is driven by a tokio::timer::Interval.
    /// Something more sophisticated than an Interval may be needed later.
    pub async fn start(self) -> Result<RaftServer<S>> {
        await! {
            tokio::timer::Interval::new_interval(self.config.runloop_interval)
            .compat()
            .map_err(|_| util::RaftError::ServerError("timer error"))
            .try_fold(self, Self::update)
        }
    }
}
