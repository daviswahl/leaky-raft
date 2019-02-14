use crate::client;
use crate::rpc;
use crate::util;
use crate::Result;
use futures::compat::*;
use futures::TryStreamExt;
use log::debug;
use rand::prelude::*;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc::Receiver;

pub struct Config {
    pub election_interval: (u64, u64),
    pub runloop_interval: Duration,
}

pub struct RaftServer {
    _receiver: Receiver<rpc::RequestCarrier>,
    clients: Vec<client::Client>,
    config: Config,
    timeout: Option<Instant>,
    pub cycles: usize,
}

pub fn new(rx: Receiver<rpc::RequestCarrier>, client_addrs: Vec<String>) -> RaftServer {
    let clients = client_addrs.into_iter().map(client::new).collect();
    RaftServer {
        clients,
        config: Config {
            election_interval: (300, 500),
            runloop_interval: Duration::from_millis(100),
        },
        timeout: None,
        _receiver: rx,
        cycles: 0,
    }
}

// for future reference
// let futs: Vec<_> = self
//     .clients
//     .iter_mut()
//     .map(|client| client.request_vote().boxed())
//     .collect();
// let result = await!(futures::future::join_all(futs));
impl RaftServer {
    fn timed_out(&mut self, now: Instant) -> bool {
        if let Some(ref timeout) = self.timeout {
            timeout <= &now
        } else {
            let mut rng = rand::thread_rng();
            let interval = self.config.election_interval;
            let instant = now + Duration::from_millis(rng.gen_range(interval.0, interval.1));
            self.timeout.replace(instant);
            false
        }
    }

    pub async fn process_messages(&mut self) -> Result<()> {
        Ok(())
    }

    pub async fn update(mut self, t: Instant) -> Result<Self> {
        await!(self.process_messages())?;

        if self.timed_out(t) {
            debug!("timed out");
        }

        Ok(self)
    }

    pub async fn start(self) -> Result<RaftServer> {
        await! {
            tokio::timer::Interval::new_interval(self.config.runloop_interval)
            .compat()
            .map_err(|_| util::RaftError::ServerError("timer error"))
            .try_fold(self, |acc, t| acc.update(t))
        }
    }
}
