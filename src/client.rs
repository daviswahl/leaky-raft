use crate::{futures::*, rpc, Error, Result};
use tarpc_bincode_transport as bincode_transport;

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

    pub async fn request_vote(&mut self) -> Result<rpc::RequestVoteRep> {
        let mut conn = await!(self.connect())?;
        await! {
            conn
                .request_vote(tarpc::context::current(), rpc::RequestVoteReq {})
                .err_into()
        }
    }
}
