use crate::futures::all::*;
use crate::{rpc, Error, LogIndex, Result, ServerId, TermId};
use std::net::SocketAddr;
use std::sync::Arc;
use tarpc_bincode_transport as bincode_transport;

#[derive(Clone)]
pub struct Peer {
    addr: Arc<SocketAddr>,
    connection: Option<rpc::gen::Client>,
}

impl Peer {
    pub fn new<R: AsRef<str>>(addr: R) -> Result<Peer> {
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

    pub async fn request_vote(
        mut self,
        candidate: ServerId,
        term: TermId,
        last_log_index: LogIndex,
        last_log_term: TermId,
    ) -> rpc::RpcResult<rpc::Response> {
        let mut conn = await!(self.connect())?;
        await!(conn
            .request_vote(
                tarpc::context::current(),
                rpc::RequestVoteReq {
                    candidate,
                    term,
                    last_log_index,
                    last_log_term
                }
            )
            .err_into::<rpc::RpcError>()
            .map(|e| e.unwrap()))
    }
}
