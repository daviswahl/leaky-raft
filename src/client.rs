use crate::{
    futures::all::*,
    rpc::{self, RequestVoteRep, RequestVoteReq},
    Result,
};

use tarpc_bincode_transport as bincode_transport;

use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Client {
    addr: String,
    connection: Option<rpc::gen::Client>,
}

impl Client {
    pub fn new(addr: String) -> Client {
        Client {
            addr,
            connection: None,
        }
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
    pub fn request_vote(&self, _tx: mpsc::Sender<RequestVoteRep>) -> Result<()> {
        log::debug!("in async fn request vote");

        Ok(())
    }
}
