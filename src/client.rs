use crate::{rpc, RaftError, Result};
use futures::TryFutureExt;
use tarpc_bincode_transport as bincode_transport;

pub struct Client {
    addr: String,
    connection: Option<crate::rpc::gen::Client>,
}

pub fn new(addr: String) -> Client {
    Client {
        addr,
        connection: None,
    }
}

impl Client {
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

    pub async fn request_vote(&mut self) -> Result<rpc::Response> {
        let mut conn = await!(self.connect())?;
        await! {
            conn
                .request_vote(tarpc::context::current(), rpc::Request::RequestVote(rpc::RequestVoteReq {}))
                .err_into::<RaftError>()
        }
    }
}
