use futures::{
    future::{self, Ready},
    prelude::*,
};

use tarpc::{
    client, context,
    server::{self, Handler},
};

use crate::rpc::Service;

// This is the type that implements the generated Service trait. It is the business logic
// and is used to start the server.
#[derive(Clone)]
pub struct HelloServer;

impl Service for HelloServer {
    // Each defined rpc generates two items in the trait, a fn that serves the RPC, and
    // an associated type representing the future output by the fn.

    type HelloFut = Ready<String>;

    fn hello(self, _: context::Context, name: String) -> Self::HelloFut {
        future::ready(format!("Hello, {}!", name))
    }
}