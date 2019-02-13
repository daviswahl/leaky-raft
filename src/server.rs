use crate::client;
use crate::rpc;
use crate::util;
use crate::Result;
use futures::compat::*;
use futures::TryStreamExt;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc::Receiver;

pub struct Config {
    pub election_interval: (usize, usize),
    pub runloop_interval: Duration,
}

pub struct RaftServer {
    _receiver: Receiver<rpc::RequestCarrier>,
    _clients: Vec<client::Client>,
    config: Config,
    _timeout: Option<Instant>,
    pub cycles: usize,
}

pub fn new(rx: Receiver<rpc::RequestCarrier>, _clients: Vec<u32>) -> RaftServer {
    RaftServer {
        _clients: vec![],
        config: Config {
            election_interval: (300, 500),
            runloop_interval: Duration::from_millis(100),
        },
        _timeout: None,
        _receiver: rx,
        cycles: 0,
    }
}

impl RaftServer {
    pub async fn update(mut self, _t: Instant) -> Result<Self> {
        self.cycles += 1;

        if self.cycles > 10 {
            Err(util::RaftError::ServerError("shutdown"))
        } else {
            Ok(self)
        }
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
