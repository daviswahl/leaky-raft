#![feature(
    futures_api,
    arbitrary_self_types,
    await_macro,
    async_await,
    proc_macro_hygiene
)]

#[macro_use]
extern crate leaky_raft;

use futures::{compat::TokioDefaultSpawner, prelude::*};
use leaky_raft::util::spawn_compat;

use env_logger;
use log::{error, info};
use tarpc::server::Handler;
use tarpc_bincode_transport as bincode_transport;

use leaky_raft::{rpc, server, Error, Result};
// Basically everything ./main.rs is temporary.

static ADDR: &'static str = "0.0.0.0";

async fn spawn_server(port: u32, clients: Vec<u32>) -> Result<()> {
    let port = mk_addr_string(port);
    info!("Spawning server at: {}", port);
    let transport = bincode_transport::listen(&port.parse()?)?;

    let (tx, rx) = tokio::sync::mpsc::channel(1_000);

    // TODO: Need to be able to shut this down.
    let server = tarpc::server::new(tarpc::server::Config::default())
        .incoming(transport)
        .respond_with(rpc::gen::serve(rpc::new_server(tx)));

    spawn_compat(server);

    let clients = clients.into_iter().map(mk_addr_string).collect();
    let server = server::new(rx, clients, ())
        .start()
        .map(|complete| match complete {
            Ok(state) => info!("stream exhausted: {}", state.cycles),
            Err(e) => error!("{}", e),
        });

    spawn_compat(server);
    Ok(())
}

fn mk_addr_string(port: u32) -> String {
    format!("{}:{}", ADDR, port)
}

async fn run() -> Result<()> {
    collect_await!(
        spawn_server(12000, vec![12001, 12002]),
        spawn_server(12001, vec![12000, 12002]),
        spawn_server(12002, vec![12000, 12001]),
    )?;

    Ok(())
}

fn main() {
    env_logger::init();
    tarpc::init(TokioDefaultSpawner);
    tokio::run(run().map_err(|e| error!("Oh no: {}", e)).boxed().compat());
}
