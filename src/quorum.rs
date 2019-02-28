use crate::rpc::RequestVoteReq;
use crate::rpc::Response;
use crate::rpc::RpcError;
use crate::rpc::RpcResult;
use crate::util::spawn_compat;
use crate::{futures::all::*, rpc, Result, ServerId, TermId};

use std::net::SocketAddr;
use std::pin::Pin;

use std::sync::Arc;
use std::task::Poll;
use std::task::Waker;
use tarpc_bincode_transport as bincode_transport;

use tokio::prelude::Async;
use tokio::sync::oneshot;

pub struct Quorum {
    parent: ServerId,
    peers: Vec<Peer>,
    receiver: Option<oneshot::Receiver<Vec<bool>>>,
}

impl Quorum {
    pub fn new<R: AsRef<str>>(parent: ServerId, peers: Vec<R>) -> Self {
        Quorum {
            parent,
            peers: peers.iter().map(Peer::new).map(|e| e.unwrap()).collect(),
            receiver: None,
        }
    }

    pub fn request_vote(&mut self, _server: ServerId, _term: TermId) -> Result<()> {
        if let Some(mut rx) = self.receiver.take() {
            log::info!("{}, quorum: closing existing rx", self.parent);
            rx.close();
        }

        let (tx, rx) = oneshot::channel();
        self.receiver.replace(rx);

        let fut = self
            .peers
            .clone()
            .into_iter()
            .map(|peer| peer.request_vote());

        let stream = util::stream::futures_unordered(fut);
        let stream = stream
            .try_filter_map(|res| {
                async {
                    match res {
                        rpc::Response::RequestVote(rep) => {
                            if rep.vote_granted {
                                Ok(Some(true))
                            } else {
                                Ok(None)
                            }
                        }
                        _ => Ok(None),
                    }
                }
            })
            .try_collect()
            .map(|vec: rpc::RpcResult<Vec<_>>| {
                if let Ok(v) = vec {
                    tx.send(v).unwrap_or_else(|e| log::error!("error: {:?}", e))
                }
            });

        //spawn_compat(stream);
        Ok(())
    }

    pub fn poll_response(&mut self) -> Result<()> {
        if let Some(ref mut recv) = self.receiver {
            match recv.poll() {
                Ok(Async::Ready(_msg)) => {
                    self.receiver.take();
                    Ok(log::debug!("got resp"))
                }
                Ok(_) => Ok(()),
                Err(e) => {
                    self.receiver.take();
                    Err("RecvError in Quorum::poll_response".into())
                }
            }
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct Peer {
    addr: Arc<SocketAddr>,
    connection: Option<rpc::gen::Client>,
}

impl Peer {
    fn new<R: AsRef<str>>(addr: R) -> Result<Peer> {
        let addr = addr.as_ref().parse()?;
        Ok(Peer {
            addr: Arc::new(addr),
            connection: None,
        })
    }
    /// If we have an established connection, clone and return it. Otherwise,
    /// establish the connection, clone and return it,
    ///
    /// I'm pretty sure cloning the underlying client is right. Could lock it
    /// instead but it's already synchronized internally so no point.
    async fn connect(&mut self) -> Result<rpc::gen::Client> {
        match self.connection {
            Some(ref conn) => Ok(conn.clone()),
            None => {
                let transport = await!(bincode_transport::connect(&self.addr))?;
                let client = await!(crate::rpc::gen::new_stub(
                    tarpc::client::Config::default(),
                    transport
                ))?;
                self.connection.replace(client.clone());
                Ok(client)
            }
        }
    }

    pub async fn request_vote(mut self) -> RpcResult<rpc::Response> {
        let mut conn = await!(self.connect())?;
        await!(conn
            .request_vote(tarpc::context::current(), rpc::RequestVoteReq {})
            .err_into::<RpcError>())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        tokio::runtime::current_thread::run(backward(
            async {
                let mut q = Quorum::new(vec!["127.0.0.1:1234"]);
                q.request_vote(ServerId("127.0.0.1:1234".parse().unwrap()), TermId(0));
                Ok(())
            },
        ))
    }
}
