use crate::rpc::RpcError;
use crate::rpc::RpcResult;
use crate::util::spawn_compat;
use crate::util::RaftError;
use crate::{
    futures::all::*,
    rpc::{self, RequestVoteRep, RequestVoteReq},
    Result,
};
use crossbeam_channel::unbounded;
use futures_util::stream::FuturesUnordered;
use log::debug;
use log::{error, info};
use std::pin::Pin;
use std::sync::Arc;
use std::task::LocalWaker;
use std::task::Poll;
use tarpc_bincode_transport as bincode_transport;
use tokio::prelude::Async;
use tokio::sync::oneshot::{self, Receiver, Sender};

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

    fn connection(&mut self) -> Result<rpc::gen::Client> {
        if let Some(ref conn) = self.connection {
            return Ok(conn.clone());
        }
        Err("no connection".into())
    }
    pub async fn request_vote(mut self) -> RpcResult<rpc::RequestVoteRep> {
        info!("in async fn request vote");
        let mut conn = await!(self.connect())?;
        await!(conn
            .request_vote(tarpc::context::current(), rpc::RequestVoteReq {})
            .err_into::<RpcError>())?
    }
}

pub struct Peers {
    clients: Vec<Client>,
}

impl Peers {
    pub fn new(clients: Vec<Client>) -> Peers {
        Peers { clients }
    }

    pub fn request_vote(&mut self, vote: RequestVoteReq) -> Receiver<Vec<bool>> {
        let (tx, rx) = oneshot::channel();

        let quorum = self.clients.len() as u64 / 2;

        let futs = self
            .clients
            .clone()
            .into_iter()
            .map(|c| c.request_vote().boxed().compat());

        let stream = tokio::prelude::stream::futures_unordered(futs)
            .filter_map(
                |r: RequestVoteRep| {
                    if r.vote_granted {
                        Some(true)
                    } else {
                        None
                    }
                },
            )
            .take(quorum)
            .collect()
            .map(move |f| {
                info!("sending election result on channel");
                tx.send(f)
            })
            .map_err(|_| ())
            .map(|_| ());
        tokio_executor::spawn(stream);
        rx
    }
}
