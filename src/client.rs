use crate::rpc::RequestVoteFut;
use crate::rpc::RpcError;
use crate::rpc::RpcResult;
use crate::util::spawn_compat;
use crate::util::RaftError;
use crate::ServerId;
use crate::TermId;
use crate::{
    futures::all::*,
    rpc::{self, RequestVoteRep, RequestVoteReq},
    Result,
};
use crossbeam_channel::unbounded;
use futures::stream::StreamObj;
use futures_util::stream::FuturesUnordered;
use log::debug;
use log::{error, info};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;
use std::task::LocalWaker;
use std::task::Poll;
use tarpc_bincode_transport as bincode_transport;
use tokio::prelude::Async;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct Client {
    addr: String,
    connection: Option<rpc::gen::Client>,
}

pub fn new(addr: String) -> Client {
    Client {
        addr,
        connection: None,
    }
}

impl Client {
    /// If we have an established connection, clone and return it. Otherwise,
    /// establish the connection, clone and return it,
    ///
    /// I'm pretty sure cloning the underlying client is right. Could lock it
    /// instead but it's already synchronized internally so no point.
    async fn connect(&mut self) -> Result<rpc::gen::Client> {
        match self.connection {
            Some(ref conn) => Ok(conn.clone()),
            None => {
                let addr = self.addr.parse()?;
                let transport = await!(bincode_transport::connect(&addr))?;
                let client = await!(crate::rpc::gen::new_stub(
                    tarpc::client::Config::default(),
                    transport
                ))?;
                self.connection.replace(client.clone());
                Ok(client)
            }
        }
    }

    fn connection(&self) -> Result<rpc::gen::Client> {
        if let Some(ref conn) = self.connection {
            return Ok(conn.clone());
        }
        Err("no connection".into())
    }
    pub fn request_vote(&self, tx: mpsc::Sender<RequestVoteRep>) -> Result<()> {
        info!("in async fn request vote");

        Ok(())
    }
}

struct QuorumResult {}

impl OldFuture for QuorumResult {
    type Item = ();
    type Error = RaftError;

    fn poll(&mut self) -> Result<Async<Self::Item>> {
        unimplemented!()
    }
}
enum Quorum {
    NoSession,
    InSession {
        stream: mpsc::Receiver<RequestVoteRep>,
        results: Vec<RequestVoteRep>,
    },
}

pub struct Peers {
    clients: Vec<Client>,
    quorum: Quorum,
}

impl Peers {
    pub fn new(clients: Vec<Client>) -> Peers {
        Peers {
            clients,
            quorum: Quorum::NoSession,
        }
    }

    fn replace_quorum(&mut self, mut q: Quorum) {
        use std::mem;
        mem::replace(&mut self.quorum, q);
    }

    pub fn check_election_results(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn request_vote(&mut self, term: TermId, candidate_id: ServerId) -> Result<()> {
        let (tx, rx) = mpsc::channel(self.clients.len());

        let quorum = self.clients.len() as u64 / 2;

        let futs = self
            .clients
            .clone()
            .into_iter()
            .map(|c| c.request_vote(tx.clone()));

        let mut q = Quorum::InSession {
            stream: rx,
            results: vec![],
        };
        self.replace_quorum(q);
        Ok(())
    }
}
