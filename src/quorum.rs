use crate::rpc::RequestVoteReq;
use crate::util::spawn_compat;
use crate::{futures::all::*, rpc, Result, ServerId, TermId};
use futures::future::Shared;
use futures_core::future::UnsafeFutureObj;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::LocalWaker;
use std::task::Poll;
use tarpc_bincode_transport as bincode_transport;
use tokio::prelude::task::spawn;

struct Request<'a> {
    fut: Box<dyn StdFuture<Output = rpc::RpcResult<rpc::Response>> + 'a>,
}

impl<'a> Request<'a> {
    pub fn new(client: rpc::gen::Client, req: rpc::Request) -> Self {
        match req {
            rpc::Request::RequestVote(req) => Self::request_vote(client, req),
            _ => unimplemented!(),
        }
    }

    fn request_vote(mut client: rpc::gen::Client, req: RequestVoteReq) -> Self {
        let fut = Box::new(
            client
                .request_vote(tarpc::context::current(), req)
                .map(|e| e.unwrap()),
        );
        Request { fut: fut }
    }
}

impl<'a> StdFuture for Request<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        unimplemented!()
    }
}

enum RequestState<'a> {
    Pending(Request<'a>),
    Done(rpc::Response),
}

pub struct Requests<'a> {
    requests: Vec<RequestState<'a>>,
}

pub struct Quorum<'a> {
    peers: Vec<Peer>,
    in_flight: Option<Requests<'a>>,
}

impl<'a> Quorum<'a> {
    pub fn new() -> Self {
        Quorum {
            peers: vec![],
            in_flight: None,
        }
    }

    pub fn request_vote(&mut self, server: ServerId, term: TermId) -> Result<()> {
        Ok(())
    }

    pub fn poll_response(&self) -> Result<()> {
        Ok(())
    }
}

pub struct Peer {
    addr: SocketAddr,
    connection: Option<rpc::gen::Client>,
}

impl Peer {
    fn new(addr: &str) -> Result<Peer> {
        let addr = addr.parse()?;
        Ok(Peer {
            addr,
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

    fn connection(&self) -> Result<rpc::gen::Client> {
        if let Some(ref conn) = self.connection {
            return Ok(conn.clone());
        }
        Err("no connection".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        let mut q = Quorum::new();
        q.request_vote(ServerId("127.0.0.1:1234".parse().unwrap()), TermId(0));
    }
}
